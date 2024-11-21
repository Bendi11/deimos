use std::{net::IpAddr, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use deimosproto::auth::DeimosTokenKey;
use rand::{CryptoRng, Rng};
use tokio::sync::{mpsc, oneshot};
use tonic::{async_trait, service::Interceptor};


mod issue;
pub use issue::{ApiTokenIssueError, PendingTokenStream};

use crate::server::Deimos;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize,)]
pub struct ApiToken {
    /// Username as given in the token request
    user: Arc<str>,
    /// Date and time that the token was issued by the server
    issued: DateTime<Utc>,
    /// Randomly generated token assigned by the server
    key: DeimosTokenKey,
}

/// A pending token request from a client
#[derive(Debug,)]
pub struct ApiTokenPending {
    user: Arc<str>,
    requested_at: DateTime<Utc>,
    requester: IpAddr,
    resolve: mpsc::Sender<Result<ApiToken, ApiTokenIssueError>>,
}

type PendingTokensCollection = Arc<DashMap<Arc<str>, ApiTokenPending>>;

/// Authorization state for the gRPC API, tracking all issued tokens
#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ApiAuthorization {
    /// User-provided configuration
    config: ApiAuthorizationConfig,
    /// A map of base64 token keys to their state
    tokens: Arc<DashMap<String, ApiToken>>,
    /// Set of token requests sorted by the time they were requested in order to remove the oldest
    /// requests by a maintainer thread
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

#[async_trait]
impl deimosproto::internal_server::Internal for Deimos {
    async fn get_pending(self: Arc<Self>, _req: tonic::Request<deimosproto::GetPendingRequest>)
        -> Result<tonic::Response<deimosproto::GetPendingResponse>, tonic::Status> {
        let pending = self
            .api
            .auth
            .pending
            .iter()
            .map(|pending| deimosproto::PendingTokenRequest {
                username: pending.user.to_string(),
                requested_dt: pending.requested_at.timestamp(),
                requester_address: pending.requester.to_string(),
            })
            .collect();

        Ok(
            tonic::Response::new(deimosproto::GetPendingResponse { pending })
        )
    }

    async fn approve(self: Arc<Self>, req: tonic::Request<deimosproto::ApproveRequest>)
        -> Result<tonic::Response<deimosproto::ApproveResponse>, tonic::Status> {
        let user = req.into_inner().username;

        let pending = self.api.auth.pending.remove(&*user);

        match pending {
            Some((user, pend)) => {
                tracing::info!("Approved token request for '{}'", user);

                self
                    .api
                    .auth
                    .approve(pend)
                    .await
                    .map(|_| tonic::Response::new(deimosproto::ApproveResponse {}))
                    .map_err(|e| tonic::Status::internal(e.to_string()))
            },
            None => {
                Err(
                    tonic::Status::not_found(format!("Request with username {} not found", user))
                )
            }
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

impl ApiToken {
    /// Generate a new token from the given source of randomness
    pub fn rand<R: Rng + CryptoRng>(mut rng: R, user: Arc<str>) -> Self {
        let issued = Utc::now();
        let mut key = vec![0u8 ; 64];
        rng.fill_bytes(&mut key);

        let key = DeimosTokenKey::from_bytes(key);

        Self {
            user,
            issued,
            key,
        }
    }
    
    /// Get a protocol buffer representation of the given token
    pub fn proto(&self) -> deimosproto::Token {
        deimosproto::Token {
            name: self.user.to_string(),
            issued: self.issued.timestamp(),
            key: self.key.as_bytes().to_owned(),
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

impl std::cmp::PartialEq for ApiTokenPending {
    fn eq(&self, other: &Self) -> bool {
        self.requested_at.eq(&other.requested_at)
    }
}

impl std::cmp::Eq for ApiTokenPending {}

impl std::cmp::PartialOrd for ApiTokenPending {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.requested_at.partial_cmp(&other.requested_at)
    }
}

impl std::cmp::Ord for ApiTokenPending {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.requested_at.cmp(&other.requested_at)
    }
}