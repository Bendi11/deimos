
use std::{future::Future, net::IpAddr, sync::Arc};

use futures::Stream;
use pin_project::pin_project;
use rand::rngs::OsRng;
use tokio::sync::oneshot;

use super::{ApiAuthorization, ApiToken, ApiTokenPending};

#[pin_project]
pub struct PendingTokenStream {
    #[pin]
    rx: tokio::sync::oneshot::Receiver<Result<ApiToken, ApiTokenIssueError>>,
    tokens: ApiAuthorization,
    username: Arc<str>,
}

impl ApiAuthorization {
    pub fn issue(&self, name: String) -> Result<ApiToken, ApiTokenIssueError> {
        if self.tokens.iter().any(|entry| *entry.value().user == name) {
            return Err(ApiTokenIssueError::UsernameInUse(name))
        }

        for _ in 0..16 {
            let token = ApiToken::rand(OsRng, name.clone());
            let base64 = token.key.to_base64();
            match self.tokens.get(&base64) {
                Some(exist) => {
                    tracing::warn!("Generated API token identical to existing: new for '{}' collides with token issued for '{}'", token.user, exist.user);
                },
                None => {
                    self.tokens.insert(token.key.to_base64(), token.clone());
                    return Ok(token)
                }
            }
        }

        panic!("Generated duplicate tokens over 16 times - something is wrong with PRNG");
    }
    
    /// Create a new pending token request for the given username
    pub fn create_request(&self, requester: IpAddr, user: Arc<str>) -> PendingTokenStream {
        let (resolve, stream) = PendingTokenStream::new(self.clone(), user.clone());
        match self.valid_username(&user) {
            Ok(_) => {
                let mut pending = self.pending.lock().unwrap();
                pending.insert(
                    ApiTokenPending {
                        user,
                        requested_at: chrono::Utc::now(),
                        requester,
                        resolve,
                    }
                );
            }
            Err(e) => {
                let _ = resolve.send(Err(e));
            }
        }

        stream
    }
    
    /// Ensure the given username only contains displayable ASCII characters and that it does not
    /// collide with existing tokens
    fn valid_username(&self, user: &str) -> Result<(), ApiTokenIssueError> {
        match user.chars().all(|c| c.is_ascii_alphanumeric() || c.is_ascii_punctuation()) {
            true => match self.tokens.iter().all(|entry| *entry.value().user != *user) {
                true => match self.pending.lock().unwrap().iter().all(|pending| *pending.user != *user) {
                    true => Ok(()),
                    false => Err(ApiTokenIssueError::UsernameInUse(user.to_owned())),
                },
                false => Err(ApiTokenIssueError::UsernameInUse(user.to_owned())),
            },
            false => Err(ApiTokenIssueError::InvalidUsername),
        }
    }
}

impl PendingTokenStream {
    /// Create a new token stream and the sender used to resolve the token request
    pub fn new(tokens: ApiAuthorization, username: Arc<str>) -> (oneshot::Sender<Result<ApiToken, ApiTokenIssueError>>, Self) {
        let (tx, rx) = oneshot::channel();
        (
            tx,
            Self {
                rx,
                tokens,
                username
            }
        )
    }
}

impl Stream for PendingTokenStream {
    type Item = Result<deimosproto::Token, tonic::Status>;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        let recv = self.project().rx;
        let recv = futures::ready!(recv.poll(cx));
        std::task::Poll::Ready(
            Some(
                recv
                    .as_ref()
                    .map_err(|_| tonic::Status::cancelled("Token request cancelled"))
                    .and_then(|res| match res {
                        Ok(token) => Ok(ApiToken::proto(token)),
                        Err(e) => Err(tonic::Status::permission_denied(e.to_string()))
                    })
            )
        )
    }
}


#[derive(Debug, thiserror::Error)]
pub enum ApiTokenIssueError {
    #[error("A token with username '{}' already exists", .0)]
    UsernameInUse(String),
    #[error("Username is not ASCII or contains whitespace")]
    InvalidUsername,
}
