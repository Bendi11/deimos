use std::{sync::Arc, task::Poll};

use bollard::container::LogsOptions;
use bytes::Bytes;
use futures::{stream::BoxStream, Stream, StreamExt};

use crate::pod::{id::DeimosId, Pod, PodManager, PodStateKnown};

/// A streamer forwarding a Docker container's logs
#[pin_project::pin_project]
pub struct PodLogStream {
    #[pin]
    stream: BoxStream<'static, Result<bollard::container::LogOutput, bollard::errors::Error>>,
    id: DeimosId,
}

impl PodManager {
    /// Subscribe to logs from the given pod
    pub async fn subscribe_logs(&self, pod: Arc<Pod>) -> Result<PodLogStream, PodSubscribeLogsError> {
        let lock = pod.state().read().await;
        match *lock {
            PodStateKnown::Enabled(ref run) => Ok(
                PodLogStream::new(
                    pod.id(),
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
                )
            ),
            _ => Err(PodSubscribeLogsError::NotEnabled),
        }
    }
}

impl PodLogStream {
    /// Create a new log streamer from the given existing stream
    fn new(
        id: DeimosId,
        stream: impl Stream<Item = Result<bollard::container::LogOutput, bollard::errors::Error>> + Send + 'static
    ) -> Self {
        Self {
            stream: stream.boxed(),
            id,
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
                            tracing::warn!("Log stream for {} closing due to failure: {}", stream.id, e);
                            None
                        }
                    },
                    None => {
                        tracing::trace!("Log stream for {} stopped", stream.id);
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
