use std::{str::FromStr, sync::Arc, time::Duration};

use auth::PersistentToken;
use deimosproto::{auth::DeimosTokenKey, client::DeimosServiceClient};
use http::Uri;
use layer::{cancel::{CancelLayer, CancelService}, conn::{ConnectionTracker, ConnectionTrackerLayer}};
use tokio::sync::{Mutex, Notify};
use tonic::{metadata::MetadataValue, service::Interceptor, transport::{Channel, ClientTlsConfig}};

use super::NotifyMutation;

mod layer;
mod auth;

#[derive(Clone)]
pub struct AuthenticationInterceptor(Option<DeimosTokenKey>);

/// A client for the authorized pod control API
pub type ApiClient = DeimosServiceClient<
    tonic::service::interceptor::InterceptedService<
        CancelService<ConnectionTracker<Channel>>,
        AuthenticationInterceptor
    >
>;

/// A client for the restricted authorization API, requests are multiplexed over the same channel
/// as standard API requests
pub type AuthClient = deimosproto::authclient::DeimosAuthorizationClient<CancelService<Channel>>;

/// All state required for accessing the authorized pod control APIs
#[derive(Debug)]
pub struct ContextClients {
    /// Current connection state, updated by middleware in the client stack
    pub conn: NotifyMutation<ContextConnectionState>,
    /// Persistent data maintained in save files for the clients
    pub persistent: ContextPersistent,
    /// Notifier semaphore used to stop ongoing API requests when reloading settings or token
    cancel: Arc<Notify>,
    /// Collection of all service clients - these are reset whenever the API has to be reconnected
    /// due to token or settings change
    clients: Mutex<Option<ClientCollection>>,
}

#[derive(Debug)]
struct ClientCollection {
    pods: ApiClient,
    auth: AuthClient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextConnectionState {
    Unknown,
    NoToken,
    Connected,
    Error,
}

/// Persistent state kept for the [Context]'s connection and authorization data
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContextPersistent {
    pub settings: NotifyMutation<ContextSettings>,
    #[serde(default)]
    pub token: NotifyMutation<Option<PersistentToken>>,
}

/// Settings that may be adjusted by the user
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContextSettings {
    #[serde(with = "http_serde::uri")]
    pub server_uri: Uri,
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
}

impl ContextClients {
    pub async fn new(persistent: ContextPersistent) -> Self {
        let conn = NotifyMutation::new(ContextConnectionState::Unknown);
        let cancel = Arc::new(Notify::new());
        let clients = Mutex::new(None);

        let this = Self {
            persistent,
            conn,
            cancel,
            clients,
        };

        this.connect_api().await;

        this
    }
    
    /// Get the authorized pods API client if one is available
    pub async fn podapi(&self) -> Option<tokio::sync::MappedMutexGuard<ApiClient>> {
        tokio::sync::MutexGuard::try_map(
            self.clients.lock().await,
            |opt| opt.as_mut().map(|c| &mut c.pods)
        ).ok()
    }

    /// Reload the API connection using the given context settings
    pub async fn reload(&self, settings: ContextSettings) {
        self.persistent.settings.set(settings.clone());
        self.connect_api().await;
    }

    /// Create a new gRPC client with the given connection settings, used to refresh the connection
    /// as settings are updated
    async fn connect_api(&self) {
        let token = self.persistent.token.read().clone();
        let token = match token {
            Some(ref token) => match token.unprotect() {
                Ok(unprotect) => Some(unprotect),
                Err(e) => {
                    tracing::error!("Failed to unprotect token: {}", e);
                    return
                }
            },
            None => None,
        };
        
        let channel = {
            let settings = self.persistent.settings.read();
            Channel::builder(settings.server_uri.clone())
                .connect_timeout(settings.connect_timeout)
                .timeout(settings.request_timeout)
                .tls_config(ClientTlsConfig::new().with_webpki_roots())
                .ok()
        };

        let Some(channel) = channel else { return };

        self.cancel.notify_waiters();
        let mut lock = self.clients.lock().await;
        
        let channel = channel.connect_lazy();
        let pods = DeimosServiceClient::with_interceptor(
            tower::ServiceBuilder::new()
                .layer(CancelLayer::new(self.cancel.clone()))
                .layer(ConnectionTrackerLayer::new(self.conn.clone()))
                .service(channel.clone()),
            AuthenticationInterceptor(token.map(|tok| tok.key))
        );

        let auth = AuthClient::new(
            tower::ServiceBuilder::new()
                .layer(CancelLayer::new(self.cancel.clone()))
                .service(channel),
        );
        
        *lock = Some(
            ClientCollection {
                pods,
                auth,
            }
        );
    }
}

impl Interceptor for AuthenticationInterceptor {
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        if let Some(ref token) = self.0 {
            match MetadataValue::from_str(&token.to_base64()) {
                Ok(val) => {
                    request
                        .metadata_mut()
                        .insert(
                            "authorization",
                            val,
                        );
                },
                Err(e) => {
                    tracing::error!("Failed to create HTTP header for authorization token: {}", e);
                }
            }
            
        }

        Ok(request)
    }
}

impl Default for ContextPersistent {
    fn default() -> Self {
        Self {
            settings: NotifyMutation::new(ContextSettings::default()),
            token: NotifyMutation::new(None),
        }
    }
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            server_uri: Uri::default(),
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(60),
        }
    }
}
