use std::{future::Future, sync::Arc, task::Poll};

use futures::future::BoxFuture;
use pin_project::pin_project;
use tokio::sync::Notify;
use tower::{Layer, Service};

/// Layer that will wrap a service in a [CancelService]
pub struct CancelLayer {
    cancel: Arc<Notify>,
}

/// A service wrapper that cancels API requests if they are notified by a contained [Notify]
#[derive(Debug, Clone)]
pub struct CancelService<S> {
    inner: S,
    cancel: Arc<Notify>,
}

#[pin_project]
pub struct CancelFuture<F> {
    #[pin]
    inner: F,
    #[pin]
    cancel: BoxFuture<'static, ()>,
}

impl CancelLayer {
    /// Create a new layer that will cancel API requests when the given notifier is notified
    pub const fn new(cancel: Arc<Notify>) -> Self {
        Self {
            cancel,
        }
    }
}

impl<S> Layer<S> for CancelLayer {
    type Service = CancelService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CancelService {
            inner,
            cancel: self.cancel.clone(),
        }
    }
}

impl<S, R> Service<R> for CancelService<S>
where 
    S: Service<R>,
    S::Error: std::fmt::Display,
{
    type Response = S::Response;
    type Error = CancelServiceError<S::Error>;
    type Future = CancelFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        Service::poll_ready(&mut self.inner, cx).map_err(CancelServiceError::Internal)
    }

    fn call(&mut self, req: R) -> Self::Future {
        let notify = self.cancel.clone();

        CancelFuture {
            inner: self.inner.call(req),
            cancel: Box::pin(async move { notify.notified().await }),
        }
    }
}

impl<F, O, E> Future for CancelFuture<F>
where 
    F: Future<Output = Result<O, E>>,
    E: std::fmt::Display
{
    type Output = Result<O, CancelServiceError<E>>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let project = self.project();

        if let Poll::Ready(response) = project.inner.poll(cx) {
            return Poll::Ready(response.map_err(CancelServiceError::Internal))
        }

        if project.cancel.poll(cx).is_ready() {
            tracing::trace!("Request cancelled!");
            return Poll::Ready(Err(CancelServiceError::Cancelled))
        }

        Poll::Pending
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CancelServiceError<E: std::fmt::Display> {
    #[error("API call cancelled by remote")]
    Cancelled,
    #[error("{}", .0)]
    Internal(E),
}
