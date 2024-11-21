use std::{sync::Arc, time::Duration};

use dashmap::DashMap;
use deimosproto::auth::DeimosTokenKey;
use token::{ApiToken, ApiTokenPending};
use tonic::service::Interceptor;

mod grpc;
mod issue;
mod token;
pub use issue::{ApiTokenIssueError, PendingTokenStream};


type PendingTokensCollection = Arc<DashMap<Arc<str>, ApiTokenPending>>;

/// Authorization state for the gRPC API, tracking all issued tokens
#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ApiAuthorization {
    /// User-provided configuration
    config: ApiAuthorizationConfig,
    /// A map of base64 token keys to their state
    tokens: Arc<DashMap<String, ApiToken>>,
    /// Map of all token requests
    #[serde(skip)]
    pending: PendingTokensCollection,
}

/// Persistent state loaded from and saved to save files, not meant to be editable by users
#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct ApiAuthorizationPersistent {
    tokens: Arc<DashMap<String, ApiToken>>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ApiAuthorizationConfig {
    /// How long to wait until a request is timed out
    #[serde(default="ApiAuthorizationConfig::default_token_timeout")]
    pub request_timeout: Duration,
}

impl ApiAuthorization {
    /// Get persistent state for the token store
    pub fn persistent(&self) -> ApiAuthorizationPersistent {
        ApiAuthorizationPersistent {
            tokens: self.tokens.clone(),
        }
    }
    
    /// Load API authorization state from the given persistent data and user-provided configuration
    pub fn load(persistent: ApiAuthorizationPersistent, config: ApiAuthorizationConfig) -> Self {
        Self {
            config,
            tokens: persistent.tokens,
            pending: Default::default(),
        }
    }
}

impl Interceptor for ApiAuthorization {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        if true {
            match request.metadata().get(DeimosTokenKey::HTTP_HEADER_NAME) {
                Some(token) => match token.to_str().ok().and_then(|key| self.tokens.get(key)){
                    Some(_) => Ok(request),
                    None => Err(tonic::Status::unauthenticated("Invalid authorization token")),
                },
                None => Err(tonic::Status::unauthenticated(format!("No '{}' header located", DeimosTokenKey::HTTP_HEADER_NAME))),
            }
        } else {
            Ok(request)
        }
    }
}


impl ApiAuthorizationConfig {
    pub const fn default_token_timeout() -> Duration {
        Duration::from_secs(60 * 30)
    }
}

impl Default for ApiAuthorizationConfig {
    fn default() -> Self {
        Self {
            request_timeout: Self::default_token_timeout(),
        }
    }
}
