
use std::{future::Future, net::IpAddr, sync::Arc};

use futures::Stream;
use pin_project::pin_project;

use super::{token::ApiTokenPendingFuture, ApiAuthorization, ApiToken, ApiTokenPending};

/// A stream used in the authorization API that will send either a denied message or the approved
/// token to a client that has requested a token.
#[pin_project]
pub struct PendingTokenStream(#[pin] ApiTokenPendingFuture);

impl ApiAuthorization {
    /// Approve the given pending token request
    pub async fn approve(&self, request: ApiTokenPending) -> Result<ApiToken, ApiTokenIssueError> {
        let token = request.upgrade().await;
        let base64 = token.key().to_base64();
        match self.tokens.get(&base64) {
            Some(exist) => {
                tracing::error!(
                    "Generated API token identical to existing: new for '{}' collides with token issued for '{}'",
                    token.user(),
                    exist.user()
                );

                Err(ApiTokenIssueError::KeyCollision { exist: exist.user().clone(), requested: token.user().clone() })
            },
            None => {
                self.tokens.insert(base64, token.clone());
                Ok(token)
            }
        }
    }
    
    /// Create a new pending token request for the given username
    pub async fn create_request(&self, requester: IpAddr, user: Arc<str>) -> PendingTokenStream {
        let (pending, rx) = ApiTokenPending::create(user.clone(), requester);

        match self.valid_username(&user) {
            Ok(_) => {
                self.pending.insert(
                    user.clone(),
                    pending
                );
            }
            Err(e) => {
                pending.deny(e).await;
            }
        }

        PendingTokenStream(rx)
    }
    
    /// Ensure the given username only contains displayable ASCII characters and that it does not
    /// collide with existing tokens
    fn valid_username(&self, user: &str) -> Result<(), ApiTokenIssueError> {
        match user.chars().all(|c| c.is_ascii_alphanumeric() || c.is_ascii_punctuation()) {
            true => match self.tokens.iter().all(|entry| **entry.value().user() != *user) {
                true => match !self.pending.contains_key(user) {
                    true => Ok(()),
                    false => Err(ApiTokenIssueError::UsernameInUse(user.to_owned())),
                },
                false => Err(ApiTokenIssueError::UsernameInUse(user.to_owned())),
            },
            false => Err(ApiTokenIssueError::InvalidUsername),
        }
    }
}

impl Stream for PendingTokenStream {
    type Item = Result<deimosproto::Token, tonic::Status>;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        let recv = self.project().0;
        let recv = futures::ready!(recv.poll(cx));
        std::task::Poll::Ready(Some(
            recv
                .map(ApiToken::proto)
                .map_err(tonic::Status::permission_denied)
        ))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiTokenIssueError {
    #[error("A token with username '{}' already exists", .0)]
    UsernameInUse(String),
    #[error("Username is not ASCII or contains whitespace")]
    InvalidUsername,
    #[error("Generated duplicate keys - requested {} collides with existing {}", exist, requested)]
    KeyCollision {
        exist: Arc<str>,
        requested: Arc<str>,
    },
}
