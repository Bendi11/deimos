use std::{sync::Arc, time::Duration};

use auth::{DeimosToken, PersistentToken, PersistentTokenKind};
use chrono::Utc;
use deimosproto::client::DeimosServiceClient;
use futures::StreamExt;
use http::Uri;
use layer::{auth::{AuthorizationLayer, AuthorizationService}, cancel::{CancelLayer, CancelService}, conn::{ConnectionTracker, ConnectionTrackerLayer}};
use tokio::sync::{Mutex, Notify};
use tonic::transport::{Channel, ClientTlsConfig};

use super::NotifyMutation;

pub mod auth;
mod layer;


/// A client for the authorized pod control API
pub type ApiClient = DeimosServiceClient<
    AuthorizationService<
        CancelService<
            ConnectionTracker<Channel>
        >
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
    /// Settings to be 
    pub settings: NotifyMutation<ContextSettings>,
    /// Unprotected token that is not saved on application exit, must remain synchronized with the
    /// `token` in [persistent](ContextClients::persistent)
    pub token: NotifyMutation<Option<DeimosToken>>,
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
    pub settings: ContextSettings,
    #[serde(default)]
    pub token: Option<PersistentToken>,
}

/// Settings that may be adjusted by the user
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContextSettings {
    #[serde(with = "http_serde::uri")]
    pub server_uri: Uri,
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
    pub token_protect: PersistentTokenKind,
}

impl ContextClients {
    pub async fn new(persistent: ContextPersistent) -> Self {
        let conn = NotifyMutation::new(ContextConnectionState::Unknown);
        let cancel = Arc::new(Notify::new());
        let clients = Mutex::new(None);

        let token = persistent.token.and_then(|tok| match tok.unprotect() {
            Ok(tok) => Some(tok),
            Err(e) => {
                tracing::error!("Failed to unprotect persistent token: {}", e);
                None
            }
        });
        

        let settings = NotifyMutation::new(persistent.settings);
        let token = NotifyMutation::new(token);

        let this = Self {
            conn,
            settings,
            token,
            cancel,
            clients,
        };

        this.connect_api().await;

        this
    }
    
    pub async fn request_token(&self, user: String) {
        let Some(mut auth) = self.authapi().await else { return };
        
        let request = deimosproto::TokenRequest {
            user,
            datetime: Utc::now().timestamp(),
        };

        let mut stream = match auth.request_token(request).await {
            Ok(stream) => stream.into_inner(),
            Err(e) => {
                tracing::warn!("Failed to request token from server: {}", e);
                return;
            }
        };
        
        let own_token = self.token.clone();

        tokio::task::spawn(async move {
            match stream.next().await {
                Some(Ok(token)) => match DeimosToken::from_proto(token) {
                    Ok(token) => {
                        tracing::info!("Got new token from server {:?}", token);
                        own_token.set(Some(token));
                    },
                    Err(e) => {
                        tracing::error!("Failed to decode received token: {}", e)
                    },
                }
                Some(Err(e)) => {
                    tracing::warn!("Failed to receive token from server: {}", e)
                },
                None => {
                    tracing::warn!("Token request stream closed before token was received");
                }
            }
        });
    }
    
    /// Get the authorized pods API client if one is available
    pub async fn podapi(&self) -> Option<tokio::sync::MappedMutexGuard<ApiClient>> {
        tokio::sync::MutexGuard::try_map(
            self.clients.lock().await,
            |opt| opt.as_mut().map(|c| &mut c.pods)
        ).ok()
    }

    async fn authapi(&self) -> Option<tokio::sync::MappedMutexGuard<AuthClient>> {
        tokio::sync::MutexGuard::try_map(
            self.clients.lock().await,
            |opt| opt.as_mut().map(|c| &mut c.auth)
        ).ok()
    }

    /// Reload the API connection using the given context settings
    pub async fn reload(&self, settings: ContextSettings) {
        self.settings.set(settings.clone());
        self.connect_api().await;
    }

    /// Create a new gRPC client with the given connection settings, used to refresh the connection
    /// as settings are updated
    async fn connect_api(&self) {
        let channel = {
            let settings = self.settings.read();
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
        let pods = DeimosServiceClient::new(
            tower::ServiceBuilder::new()
                .layer(AuthorizationLayer::new(self.token.clone()))
                .layer(CancelLayer::new(self.cancel.clone()))
                .layer(ConnectionTrackerLayer::new(self.conn.clone()))
                .service(channel.clone())
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
    
    /// Get a copy of the current state that can be serialized to a save file
    pub fn persistent(&self) -> ContextPersistent {
        let settings = self.settings.read().clone();

        let token = self.token.read().clone().and_then(|tok| match PersistentToken::protect(settings.token_protect, tok) {
            Ok(tok) => Some(tok),
            Err(e) => {
                tracing::error!("Failed to protect persistent token: {}", e);
                None
            }
        });

        ContextPersistent {
            settings,
            token,
        }
    }
}

impl Default for ContextPersistent {
    fn default() -> Self {
        Self {
            settings: ContextSettings::default(),
            token: None,
        }
    }
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            server_uri: Uri::default(),
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(60),
            token_protect: PersistentTokenKind::Plaintext,
        }
    }
}
