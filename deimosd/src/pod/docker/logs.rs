use std::{sync::Arc, task::Poll};

use bollard::container::LogsOptions;
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
                        .logs(
                            &run.docker_id, 
                            Some(
                                LogsOptions::<&'static str> {
                                    stdout: true,
                                    stderr: true,
                                    follow: true,
                                    ..Default::default()
                                }
                            )
                        )
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
        let poll = stream
            .stream
            .poll_next(cx);

        match poll {
            Poll::Ready(value) => Poll::Ready(
                match value {
                    Some(buf) => match buf {
                        Ok(buf) => Some(buf.into_bytes()),
                        Err(e) => {
                            tracing::warn!("Log stream closing due to failure: {e}");
                            None
                        }
                    },
                    None => {
                        tracing::trace!("Stream for container stopped");
                        None
                    }
                }
            ),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PodSubscribeLogsError {
    #[error("Container is not enabled")]
    NotEnabled,
}
