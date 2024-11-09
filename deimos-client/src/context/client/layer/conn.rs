use std::{future::Future, task::Poll};

use pin_project::pin_project;
use tonic::Code;
use tower::{Layer, Service};

use crate::context::{client::ContextConnectionState, NotifyMutation};

/// A layer that will wrap a service with a [ConnectionTracker]
pub struct ConnectionTrackerLayer {
    conn: NotifyMutation<ContextConnectionState>,
}

/// A [Service] that tracks responses from each request and sets the given connection state
#[derive(Debug, Clone,)]
pub struct ConnectionTracker<S> {
    inner: S,
    conn: NotifyMutation<ContextConnectionState>,
}

#[pin_project]
pub struct ConnectionTrackerFuture<F> {
    #[pin]
    inner: F,
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
    type Future = ConnectionTrackerFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        let inner = self.inner.call(request);

        ConnectionTrackerFuture {
            inner,
            conn: self.conn.clone(),
        }
    }
}

impl<F, T, E> Future for ConnectionTrackerFuture<F>
where 
    F: Future<Output = Result<http::Response<T>, E>>,
    E: std::fmt::Display {

    type Output = F::Output;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let project = self.project();
        let response = futures::ready!(project.inner.poll(cx));
        match response {
            Ok(response) => {
                let connstat = if let Some(status) = tonic::Status::from_header_map(response.headers()) {
                    match status.code() {
                        Code::Ok => ContextConnectionState::Connected,
                        Code::Unauthenticated => ContextConnectionState::NoToken,
                        _ => ContextConnectionState::Error,
                    }
                } else {
                    ContextConnectionState::Connected
                };

                project.conn.set(connstat);  

                Poll::Ready(Ok(response))
            },
            Err(e) => {
                project.conn.set(ContextConnectionState::Error);
                Poll::Ready(Err(e))
            }
        }
    }
}
