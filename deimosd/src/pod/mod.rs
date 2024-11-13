use std::{
    collections::HashMap, path::{Path, PathBuf}, sync::Arc, task::Poll, time::Duration
};

use bollard::{secret::EventMessageTypeEnum, system::EventsOptions, Docker};
use dashmap::DashMap;
use futures::{
    stream::{BoxStream, SelectAll}, Stream, StreamExt
};
use id::{DeimosId, DockerId};

use crate::server::upnp::Upnp;

pub mod docker;
pub mod id;
pub mod config;
pub mod state;

pub use state::{Pod,  PodState, PodStateKnown};
pub use config::{DockerConnectionConfig, DockerConnectionType, PodManagerConfig};

/// Manager responsible for orchestrating Docker containers and watching for external events and
/// failures
pub struct PodManager {
    config: PodManagerConfig,
    docker: Docker,
    upnp: Upnp,
    pods: HashMap<DeimosId, Arc<Pod>>,
    reverse_lookup: Arc<DashMap<DockerId, Arc<Pod>>>,
}

pub type PodStateStreamMapper = dyn FnMut(PodState) -> (DeimosId, PodState) + Send + Sync;
pub type PodStateStream = SelectAll<
    futures::stream::Map<
        tokio_stream::wrappers::WatchStream<PodState>,
        Box<PodStateStreamMapper>,
    >,
>;

pub struct DockerEventStream {
    inner: BoxStream<'static, Result<bollard::secret::EventMessage, bollard::errors::Error>>,
    docker: Docker,
    reverse: Arc<DashMap<DockerId, Arc<Pod>>>,
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

    fn new(docker: Docker, reverse: Arc<DashMap<DockerId, Arc<Pod>>>) -> Self {
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

impl PodManager {
    /// Load a config TOML file from the given path, and use the options specified inside to
    /// create a connection to the local Docker server, then load all pods from the directory
    /// given.
    pub async fn new(config: PodManagerConfig, upnp: Upnp) -> Result<Self, PodManagerInitError> {
        let docker = match config.docker {
            None => Docker::connect_with_local_defaults().map(|docker| {
                docker.with_timeout(Duration::from_secs(
                    DockerConnectionConfig::default_timeout(),
                ))
            }),
            Some(ref conn) => match conn.kind {
                DockerConnectionType::Http => Docker::connect_with_http(
                    &conn.addr,
                    conn.timeout,
                    bollard::API_DEFAULT_VERSION,
                ),
                DockerConnectionType::Local => Docker::connect_with_local(
                    &conn.addr,
                    conn.timeout,
                    bollard::API_DEFAULT_VERSION,
                ),
            },
        }?;

        let docker = docker.negotiate_version().await?;
        tracing::info!("Connected to Docker daemon {}", docker.client_version());

        let pods = Self::load_containers(&config.containerdir).await?;
        if pods.is_empty() {
            tracing::warn!("Starting pod manager with no pods configured");
        }

        let reverse_lookup = Arc::new(DashMap::with_capacity(pods.len()));

        Ok(Self {
            config,
            docker,
            upnp,
            pods,
            reverse_lookup,
        })
    }

    /// Get a stream of state changes made to containers, with their associated ID
    pub fn stream(&self) -> PodStateStream {
        let iter = self.pods.values().cloned().map(|pod| {
            let id = pod.id();
            tokio_stream::wrappers::WatchStream::new(pod.state.tx.subscribe()).map(Box::<PodStateStreamMapper>::from(
                Box::new(move |state| (id.clone(), state)),
            ))
        });

        futures::stream::select_all(iter)
    }

    /// Get a reference to the pod with the given ID
    pub fn get(&self, id: &str) -> Option<Arc<Pod>> {
        self.pods.get(id).cloned()
    }
    
    /// Process all Docker container events in a loop to monitor uncommanded pod state changes
    pub fn eventloop(&self) -> impl Stream<Item = (Arc<Pod>, String)> {
        DockerEventStream::new(self.docker.clone(), self.reverse_lookup.clone())
    }
    
    /// Handle an event received from the [eventloop](Self::eventloop) stream
    pub async fn handle_event(&self, pod: Arc<Pod>, action: String) {
        tracing::trace!("Pod {} got event '{}'", pod.id(), action);
        let state = pod.state_wait().await;

        match action.as_str() {
            "die" => match state {
                PodStateKnown::Disabled => {

                },
                PodStateKnown::Paused(..) => {
                    tracing::info!("Paused container {} died unexpectedly", pod.id());
                    let _ = self.disable(pod).await;
                },
                PodStateKnown::Enabled(..) => {
                    tracing::warn!("Running container {} died unexpectedly", pod.id());
                    let _ = self.disable(pod).await;
                }
            },
            _ => {},
        }
    }

    /// Load all containers from directory entries in the given containers directory,
    /// logging errors and ignoring on failure
    async fn load_containers(
        dir: &Path,
    ) -> Result<HashMap<DeimosId, Arc<Pod>>, PodManagerInitError> {
        let mut pods = HashMap::new();

        let mut iter =
            tokio::fs::read_dir(dir)
                .await
                .map_err(|err| PodManagerInitError::PodRead {
                    path: dir.to_owned(),
                    err,
                })?;

        loop {
            let entry = match iter.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(e) => {
                    tracing::error!(
                        "Failed to read directory entry from pod directory {}: {}",
                        dir.display(),
                        e
                    );
                    continue;
                }
            };

            let path = entry.path();

            match entry.file_type().await {
                Ok(ft) if ft.is_dir() => match Pod::load(&entry.path()).await {
                    Ok(pod) => {
                        pods.insert(pod.id(), Arc::new(pod));
                    }
                    Err(e) => {
                        tracing::error!("Failed to load container from {}: {}", path.display(), e);
                    }
                },
                Ok(..) => {
                    tracing::warn!(
                        "Ignoring non-directory entry {} in pod directory",
                        path.display()
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to get file type of entry {} in pod directory: {}",
                        path.display(),
                        e
                    );
                }
            }
        }

        Ok(pods)
    }

    /// Get an immutable iterator over references to the managed pods
    pub fn iter(&self) -> impl Iterator<Item = (&DeimosId, &Arc<Pod>)> {
        self.pods.iter()
    }
}

impl<'a> IntoIterator for &'a PodManager {
    type Item = (&'a DeimosId, &'a Arc<Pod>);
    type IntoIter = std::collections::hash_map::Iter<'a, DeimosId, Arc<Pod>>;

    fn into_iter(self) -> Self::IntoIter {
        self.pods.iter()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PodManagerInitError {
    #[error("Failed to create Docker client: {0}")]
    Docker(#[from] bollard::errors::Error),
    #[error("Failed to read entries from pod directory {}: {}", path.display(), err)]
    PodRead { path: PathBuf, err: std::io::Error },
}
