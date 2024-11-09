use deimosproto::auth::DeimosTokenKey;
use http::{HeaderValue, Request};
use tower::{Layer, Service};

use crate::context::{client::auth::DeimosToken, NotifyMutation};

pub struct AuthorizationLayer {
    token: NotifyMutation<Option<DeimosToken>>,
}

#[derive(Debug, Clone)]
pub struct AuthorizationService<S> {
    inner: S,
    token: NotifyMutation<Option<DeimosToken>>,
}

impl AuthorizationLayer {
    pub const fn new(token: NotifyMutation<Option<DeimosToken>>) -> Self {
        Self {
            token
        }
    }
}

impl<S> Layer<S> for AuthorizationLayer {
    type Service = AuthorizationService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthorizationService {
            inner,
            token: self.token.clone(),
        }
    }
}


impl<S, B, R> Service<Request<B>> for AuthorizationService<S>
where 
    S: Service<Request<B>, Response = R>
{
    type Response = R;
    type Future = S::Future;
    type Error = S::Error;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    
    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        if let Some(ref token) = *self.token.read() {
            match HeaderValue::from_str(token.base64()) {
                Ok(auth) => {
                    req.headers_mut().insert(DeimosTokenKey::HTTP_HEADER_NAME, auth);
                },
                Err(e) => {
                    tracing::error!("Failed to create authorization token header value: {}", e);
                }
            }
        }
        self.inner.call(req)
    }
}
