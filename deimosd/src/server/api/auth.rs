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
        let name: Arc<str> = Arc::from(name);
        let token = ApiToken::rand(OsRng, name.clone());
        self.tokens.insert(token.key.to_base64(), token.clone());
        token
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

#[pin_project]
pub struct AuthTokenStream {
    #[pin]
    recv: tokio::sync::oneshot::Receiver<ApiToken>,
}

#[async_trait]
impl deimosproto::authserver::DeimosAuthorization for Deimos {
    type RequestTokenStream = futures::stream::Once<tokio::sync::oneshot::Receiver<tonic::Response<>;

    fn request_token(self: Arc<Self>, request: tonic::Request<deimosproto::TokenRequest>) -> Result<tonic::Response<Self::RequestTokenStream>, tonic::Status> {

    }
}

impl Stream for AuthTokenStream {
    type Item = Result<tonic::Response<deimosproto::Token>, tonic::Status>;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        let project = self.project();
        let tok = futures::ready!(project.recv.poll(cx));

        Poll::Ready(match tok {
            Ok(tok) => Ok(tonic::Response::new(tok.proto())),
            Err(e) => Err(tonic::Status::permission_denied("Token request denied")),
        })
    }
}

impl ApiToken {
    /// Generate a new token from the given source of randomness
    pub fn rand<R: Rng + CryptoRng>(mut rng: R, user: Arc<str>) -> Self {
        let issued = Utc::now();
        let mut key = std::iter::repeat_n(0u8, 64).collect::<Vec<_>>();
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


