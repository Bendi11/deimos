use std::{sync::Arc, time::Duration};

use auth::{DeimosToken, PersistentToken, PersistentTokenKind, TokenStatus};
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
    pub settings: NotifyMutation<ContextSettings>,
    pub token_protect: NotifyMutation<PersistentTokenKind>,
    pub token: NotifyMutation<TokenStatus>,
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
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContextPersistent {
    pub settings: ContextSettings,

    pub token_protect: PersistentTokenKind,
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
}

impl ContextClients {
    /// Create a new client collection and attempt an API connection using the given settings
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
        
        let token_protect = NotifyMutation::new(persistent.token_protect);
        let token = TokenStatus::from_token(token);
        let settings = NotifyMutation::new(persistent.settings);
        let token = NotifyMutation::new(token);

        let this = Self {
            conn,
            settings,
            token_protect,
            token,
            cancel,
            clients,
        };

        this.connect_api().await;

        this
    }
    
    /// Request a new token with the given username from the server
    pub async fn request_token(&self, user: String) {
        self.cancel.notify_waiters();
        let Some(mut auth) = self.authapi().await else { return };

        let cancel = Arc::new(Notify::new());
        self.token.set(TokenStatus::Requested { user: user.clone(), cancel: cancel.clone() });
        
        let request = deimosproto::TokenRequest {
            user,
            datetime: Utc::now().timestamp(),
        };

        let mut stream = match auth.request_token(request).await {
            Ok(stream) => stream.into_inner(),
            Err(e) => {
                tracing::warn!("Failed to request token from server: {}", e);
                self.token.set(
                    TokenStatus::Denied { reason: e.message().to_owned() }
                );
                return;
            }
        };

        let own_token = self.token.clone();

        tokio::task::spawn(async move {
            let next = tokio::select! {
                _ = cancel.notified() => {
                    own_token.set(TokenStatus::None);
                    return
                },
                result = stream.next() => result,
            };

            let reason = match next {
                Some(Ok(token)) => match DeimosToken::from_proto(token) {
                    Ok(token) => {
                        own_token.set(TokenStatus::Token(token));
                        return
                    },
                    Err(e) => format!("Failed to decode received token: {}", e),
                }
                Some(Err(e)) => format!("Failed to receive token from server: {}", e.message()),
                None => String::from("Token request stream closed before token was received"),
            };

            own_token.set(TokenStatus::Denied { reason });
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
        let token_protect = *self.token_protect.read();
        let settings = self.settings.read().clone();

        let token = self
            .token
            .read()
            .token()
            .cloned()
            .and_then(|tok| match PersistentToken::protect(*self.token_protect.read(), tok) {
            Ok(tok) => Some(tok),
            Err(e) => {
                tracing::error!("Failed to protect persistent token: {}", e);
                None
            }
        });

        ContextPersistent {
            settings,
            token_protect,
            token,
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
