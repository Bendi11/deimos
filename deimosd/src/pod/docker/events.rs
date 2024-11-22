use std::{collections::HashMap, sync::Arc, task::Poll};

use bollard::{secret::EventMessageTypeEnum, system::EventsOptions, Docker};
use futures::{stream::BoxStream, Stream, StreamExt};

use crate::pod::{Pod, ReversePodLookup};

/// A stream that maps events received from the local Docker server to their corresponding pods.
/// Also handles resubscribing to the docker stream in case it is dropped for some reason.
pub struct DockerEventStream {
    inner: BoxStream<'static, Result<bollard::secret::EventMessage, bollard::errors::Error>>,
    docker: Docker,
    reverse: ReversePodLookup,
}

impl DockerEventStream {
    fn subscribe(docker: &Docker) -> BoxStream<'static, Result<bollard::secret::EventMessage, bollard::errors::Error>> {
        let mut filters = HashMap::with_capacity(1);
        filters.insert("type", vec!["container"]);

        tracing::trace!("Subscribing to Docker event stream with filters {:?}", filters);
        docker.events(
            Some(
                EventsOptions::<&'static str> {
                    filters,
                    ..Default::default()
                }
            )
        ).boxed()
    }
    
    /// Create a new stream that will subscribe to container events from the given Docker instance,
    /// and map them to local pods using the provided reverse lookup table
    pub fn new(docker: Docker, reverse: ReversePodLookup) -> Self {
        Self {
            inner: Self::subscribe(&docker),
            docker,
            reverse,
        }
    }
}

impl Stream for DockerEventStream {
    type Item = (Arc<Pod>, String);

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let next = futures::ready!(self.inner.as_mut().poll_next(cx));
            match next {
                Some(event) => match event {
                    Ok(ev) => match ev.typ {
                        Some(EventMessageTypeEnum::CONTAINER) => {
                            let Some(actor) = ev.actor else {
                                tracing::warn!("Received container event with no actor");
                                continue;
                            };

                            let Some(id) = actor.id else {
                                tracing::warn!("Received container event with no actor ID");
                                continue;
                            };

                            let Some(action) = ev.action else {
                                tracing::warn!("Received container event with no action");
                                continue;
                            };

                            if let Some(pod) = self.reverse.get(id.as_str()) {
                                break Poll::Ready(Some((pod.clone(), action)))
                            }
                        },
                        _ => {
                            tracing::warn!("Got unwanted Docker event {:?}", ev.typ);
                        }
                    },
                    Err(e) => {
                        tracing::error!("Docker event stream closed unexpectedly: {}", e);
                    }
                },
                None => {
                    self.inner = Self::subscribe(&self.docker);
                }
            }
        }
    }
}
