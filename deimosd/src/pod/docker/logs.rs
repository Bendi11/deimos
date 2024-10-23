use std::sync::Arc;

use bytes::Bytes;
use futures::{stream::BoxStream, Stream, StreamExt};

use crate::pod::{manager::PodManager, Pod, PodStateKnown};

/// A streamer forwarding a Docker container's logs
#[pin_project::pin_project]
pub struct PodLogStream {
    #[pin]
    stream: BoxStream<'static, Result<bollard::container::LogOutput, bollard::errors::Error>>
}

impl PodManager {
    pub async fn subscribe_logs(&self, pod: Arc<Pod>) -> Result<PodLogStream, PodSubscribeLogsError> {
        let lock = pod.state_lock().await;
        match *lock.state() {
            PodStateKnown::Enabled(ref run) => Ok(
                PodLogStream::new(
                        self
                            .docker
                            .logs(&run.docker_id, Option::<bollard::container::LogsOptions<&'static str>>::None)
                            .boxed()
                )
            ),
            _ => Err(PodSubscribeLogsError::NotEnabled),
        }
    }
}

impl PodLogStream {
    /// Create a new log streamer from the given existing stream
    fn new(stream: impl Stream<Item = Result<bollard::container::LogOutput, bollard::errors::Error>> + Send + 'static) -> Self {
        Self {
            stream: stream.boxed(),
        }
    }
}

impl Stream for PodLogStream {
    type Item = Bytes;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        let stream = self.project();
        stream
            .stream
            .poll_next(cx)
            .map(
                |o| o.and_then(
                    |result| match result {
                        Ok(buf) => Some(buf.into_bytes()),
                        Err(e) => {
                            tracing::warn!("Log stream failed: {e}");
                            None
                        }
                    }
                )
            )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PodSubscribeLogsError {
    #[error("Container is not enabled")]
    NotEnabled,
}
