use futures::future::BoxFuture;
use tonic::Code;
use tower::{Layer, Service};

use crate::context::{ContextConnectionState, NotifyMutation};


/// A [Service] that tracks responses from each request and sets the given connection state
#[derive(Debug, Clone,)]
pub struct ConnectionTracker<S> {
    inner: S,
    conn: NotifyMutation<ContextConnectionState>,
}

pub struct ConnectionTrackerLayer {
    conn: NotifyMutation<ContextConnectionState>,
}

impl ConnectionTrackerLayer {
    /// Create a new layer that will set the given connection flag with the results of a wrapper
    /// service
    pub const fn new(conn: NotifyMutation<ContextConnectionState>) -> Self {
        Self {
            conn,
        }
    }
}

impl<S> Layer<S> for ConnectionTrackerLayer {
    type Service = ConnectionTracker<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ConnectionTracker {
            inner,
            conn: self.conn.clone(),
        }
    }
}

impl<S, B> Service<http::Request<B>> for ConnectionTracker<S>
where 
    S: Service<http::Request<B>, Response = http::Response<B>> + Clone + Send + 'static,
    S::Error: std::fmt::Display + Send,
    S::Future: Send + 'static,
    B: tonic::transport::Body + Send + 'static
{
    type Response = http::Response<B>;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let conn = self.conn.clone();

        Box::pin(
            async move {
                let response = match tower::Service::call(&mut inner, request).await {
                    Ok(response) => response,
                    Err(e) => {
                        tracing::warn!("Got error in tonic channel when sending request: {}", e);
                        conn.set(ContextConnectionState::Error).await;
                        return Err(e)
                    }
                };

                if let Some(status) = tonic::Status::from_header_map(response.headers()) {
                    let connstat = match status.code() {
                        Code::Ok => ContextConnectionState::Connected,
                        Code::Unauthenticated => ContextConnectionState::NoToken,
                        _ => ContextConnectionState::Error,
                    };

                    conn.set(connstat).await;
                }

                Ok(response)
            }
        )
    }
}
