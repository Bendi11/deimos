use std::{future::Future, net::IpAddr, sync::Arc};

use chrono::{DateTime, Utc};
use deimosproto::auth::DeimosTokenKey;
use pin_project::pin_project;
use rand::{rngs::OsRng, CryptoRng, Rng};
use tokio::sync::mpsc;


/// An API token that has been created with a random key and approved sometime in the past
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize,)]
pub struct ApiToken {
    /// Username as given in the token request
    user: Arc<str>,
    /// Date and time that the token was issued by the server
    issued: DateTime<Utc>,
    /// Randomly generated token assigned by the server
    key: DeimosTokenKey,
}

/// Type representing a pending token request from a client, with data about the client and a
/// channel used to send the result of a request when it has been approved or denied
#[derive(Debug,)]
pub struct ApiTokenPending {
    /// Username of the token
    user: Arc<str>,
    /// The date and time when the token was originally requested
    requested_at: DateTime<Utc>,
    /// The IP address of the client that requested this token
    requester: IpAddr,
    /// Channel that will be used to send the result of the request to the client
    resolve: mpsc::Sender<Result<ApiToken, String>>,
}

/// Future that will resolve with the result of a pending token request
#[pin_project]
#[derive(Debug)]
pub struct ApiTokenPendingFuture(#[pin] mpsc::Receiver<Result<ApiToken, String>>);

impl ApiToken {
    /// Generate a new token from the given source of randomness and the given username
    fn rand<R: Rng + CryptoRng>(mut rng: R, user: Arc<str>) -> Self {
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
    
    /// Get a protocol buffer representation of the token
    pub fn proto(self) -> deimosproto::Token {
        deimosproto::Token {
            name: self.user.to_string(),
            issued: self.issued.timestamp(),
            key: self.key.as_bytes().to_owned(),
        }
    }
    
    /// Get the randomly-generated key assigned to this token
    pub const fn key(&self) -> &DeimosTokenKey {
        &self.key
    }
    
    /// Get the username assigned to the token
    pub const fn user(&self) -> &Arc<str> {
        &self.user
    }

    /// Get the date and time that this token was generated at
    pub const fn issued(&self) -> DateTime<Utc> {
        self.issued
    }
}

impl ApiTokenPending {
    /// Upgrade this API token request to a full API token, notifying the waiting client that the
    /// request has been approved
    pub async fn upgrade(self) -> ApiToken {
        tracing::trace!("Upgrading token request for {}", self.requester);
        let token = ApiToken::rand(OsRng, self.user);
        let _ = self.resolve.send(Ok(token.clone())).await;
        token
    }
    
    /// Deny the given token request, notifying the client waiting on the request 
    pub async fn deny(self, reason: impl ToString) {
        let _ = self.resolve.send(Err(reason.to_string())).await;
    }
    
    /// Create a new pending token request with the given username and client address and return
    /// the pending request structure and a future that will produce a value when the request is
    /// resolved
    pub fn create(user: Arc<str>, requester: IpAddr) -> (Self, ApiTokenPendingFuture) {
        let requested_at = Utc::now();
        let (resolve, rx) = mpsc::channel(1);

        (
            Self {
                user,
                requested_at,
                requester,
                resolve,
            },
            ApiTokenPendingFuture(rx)
        )
    }
    
    /// Get a protobuf representation of this token request
    pub fn proto(&self) -> deimosproto::PendingTokenRequest {
        deimosproto::PendingTokenRequest {
            username: self.user.to_string(),
            requested_dt: self.requested_at.timestamp(),
            requester_address: self.requester.to_string(),
        }
    }
}

impl Future for ApiTokenPendingFuture {
    type Output = Result<ApiToken, String>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut project = self.project();
        let result = futures::ready!(project.0.poll_recv(cx));
        match result {
            Some(value) => std::task::Poll::Ready(value),
            None => std::task::Poll::Pending,
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
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for ApiTokenPending {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.requested_at.cmp(&other.requested_at)
    }
}
