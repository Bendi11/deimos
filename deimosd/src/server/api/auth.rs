use std::{future::Future, sync::Arc, task::Poll};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use deimosproto::auth::DeimosTokenKey;
use futures::Stream;
use pin_project::pin_project;
use rand::{rngs::OsRng, CryptoRng, Rng};
use tonic::{async_trait, service::Interceptor};

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

/// Authorization state for the gRPC API, tracking all issued tokens
#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ApiAuthorization {
    tokens: Arc<DashMap<String, ApiToken>>,
}

impl ApiAuthorization {
    pub fn issue(&self, name: String) -> ApiToken {
        for _ in 0..16 {
            let token = ApiToken::rand(OsRng, name.clone());
            let base64 = token.key.to_base64();
            match self.tokens.get(&base64) {
                Some(exist) => {
                    tracing::warn!("Generated API token identical to existing: new for '{}' collides with token issued for '{}'", token.user, exist.user);
                },
                None => {
                    self.tokens.insert(token.key.to_base64(), token.clone());
                    return token
                }
            }
        }
        
        panic!("Generated duplicate tokens over 16 times - something is wrong with PRNG");
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
                None => Err(tonic::Status::unauthenticated("No 'authorization-bin' header located")),
            }
        } else {
            Ok(request)
        }
    }
}

#[pin_project(project = AuthTokenStreamProj)]
pub enum AuthTokenStream {
    Waiting(#[pin] tokio::sync::oneshot::Receiver<ApiToken>),
    Empty
}

#[async_trait]
impl deimosproto::authserver::DeimosAuthorization for Deimos {
    type RequestTokenStream = AuthTokenStream;

    async fn request_token(self: Arc<Self>, request: tonic::Request<deimosproto::TokenRequest>) -> Result<tonic::Response<Self::RequestTokenStream>, tonic::Status> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let stream = AuthTokenStream::Waiting(rx);
        let token = self.api.persistent.tokens.issue(request.into_inner().user);
        let _ = tx.send(token);
        Ok(tonic::Response::new(stream))
    }
}

impl Stream for AuthTokenStream {
    type Item = Result<deimosproto::Token, tonic::Status>;

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        Poll::Ready(match self.as_mut().project() {
            AuthTokenStreamProj::Waiting(recv) => {
                let recv = futures::ready!(recv.poll(cx));
                *self = Self::Empty;

                Some(
                    recv
                        .as_ref()
                        .map(ApiToken::proto)
                        .map_err(|_| tonic::Status::permission_denied("Token request denied"))
                )
            },
            AuthTokenStreamProj::Empty => None,
        })
    }
}

impl ApiToken {
    /// Generate a new token from the given source of randomness
    pub fn rand<R: Rng + CryptoRng>(mut rng: R, user: String) -> Self {
        let user = user.into();
        let issued = Utc::now();
        let mut key = std::iter::repeat(0u8).take(64).collect::<Vec<_>>();
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
            key: self.key.to_bytes(),
        }
    }
}
