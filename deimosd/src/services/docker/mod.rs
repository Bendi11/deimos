use std::{path::PathBuf, sync::Arc, time::Duration};

use bollard::{container::{RemoveContainerOptions, StopContainerOptions}, Docker};
use container::ManagedContainer;
use dashmap::DashMap;
use tokio_util::sync::CancellationToken;

pub mod container;

/// Service managing the creation and removal of Docker containers
pub struct DockerService {
    pub config: DockerConfig,
    docker: Docker,
    /// Map of container names to their state
    containers: DashMap<String, Arc<ManagedContainer>>,
}

/// Configuration for the local Docker container management service
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerConfig {
    pub containerdir: PathBuf,
    pub conn: Option<DockerConnectionConfig>,
}

/// Configuration governing how the server will connect to the Docker API
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerConnectionConfig {
    pub kind: DockerConnectionType,
    pub addr: String,
    pub timeout: Option<u64>,
}

#[derive(Debug, serde::Deserialize)]
pub enum DockerConnectionType {
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "local")]
    Local,
}

impl DockerService {
    /// Default timeout to use for Docker API when no alternative is specified in the config
    pub const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

    /// Load the docker service from a config file specifying how to connect to the local Docker
    /// engine and where to load container configurations from
    pub async fn new(config: DockerConfig) -> Result<Self, DockerServiceInitError> {
        let docker = match config.conn {
            None => {
                tracing::info!("No docker config given, using platform defaults to connect");
                Docker::connect_with_local_defaults()
            }
            Some(ref cfg) => {
                let timeout = cfg.timeout.unwrap_or(Self::DEFAULT_TIMEOUT_SECONDS);
                match cfg.kind {
                    DockerConnectionType::Http => {
                        Docker::connect_with_http(&cfg.addr, timeout, bollard::API_DEFAULT_VERSION)
                    }
                    DockerConnectionType::Local => Docker::connect_with_socket(
                        &cfg.addr,
                        timeout,
                        bollard::API_DEFAULT_VERSION,
                    ),
                }
            }
        }?;

        let containers = DashMap::new();
        
        let dir = config.containerdir.clone();
        let container_entries = std::fs::read_dir(&dir)
            .map_err(|err| DockerServiceInitError::ContainersDirError { dir: dir.clone(), err })?;

        for entry in container_entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    tracing::warn!(
                        "I/O error when reading entries from container config directory {}: {}",
                        dir.display(),
                        e
                    );
                    continue
                }
            };

            let container = match entry.file_type() {
                Ok(fty) if fty.is_dir() => {
                    ManagedContainer::load_from_dir(entry.path(), &docker).await
                }
                Ok(fty) if fty.is_symlink() => {
                    let meta = match tokio::fs::symlink_metadata(entry.path()).await {
                        Ok(meta) => meta,
                        Err(e) => {
                            tracing::warn!(
                                "Failed to get symlink metadata for symlink {}, skipping due to {}",
                                entry.path().display(),
                                e
                            );
                            continue;
                        }
                    };

                    if meta.is_dir() {
                        ManagedContainer::load_from_dir(entry.path(), &docker).await
                    } else {
                        continue;
                    }
                }
                Ok(_) => {
                    tracing::warn!(
                        "Unknown file in container config directory: {}",
                        entry.path().display()
                    );
                    continue;
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to get filetype of container config directory entry {}: {}",
                        entry.path().display(),
                        e
                    );
                    continue;
                }
            };

            match container {
                Ok(c) => {
                    let name = c.container_name().to_owned();
                    if containers.insert(name.clone(), Arc::new(c)).is_some() {
                        return Err(DockerServiceInitError::DuplicateConfiguration(name));
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to load container configuration from {}: {}",
                        entry.path().display(),
                        e
                    );
                }
            }
        }

        if containers.is_empty() {
            tracing::warn!("Deimos server starting with no docker containers configured");
        }

        Ok(Self {
            config,
            docker,
            containers,
        })
    }
    
    /// Create a container from the configuration loaded for the given managed container
    pub async fn create_container(&self, name: Arc<str>) -> Result<(), DockerCreateContainerError> {
        let container = self
            .containers
            .get(name.as_ref())
            .ok_or_else(|| DockerCreateContainerError::NoSuchContainer(String::from(name.as_ref())))?;

        container.create(&self.docker).await.map_err(Into::into)
    }
    
    /// Run all necessary tasks for the Docker container manager, cancel-safe with the given
    /// cancellation token
    pub async fn run(self: Arc<Self>, cancel: CancellationToken) {
        tokio::select! {
            _ = cancel.cancelled() => {},
            _ = self.clone().run_internal() => {},
        };
        
        tracing::info!("Removing all Docker containers");
        self.stop_all().await;
    }

    async fn run_internal(self: Arc<Self>) {
        if let Err(e) = self.create_container(Arc::from("arma3")).await {
            tracing::error!("Failed to create container: {e}");
        }

        tokio::time::sleep(Duration::from_secs(30)).await;

        self.stop_all().await;
    }
    
    /// Attempt to stop all running containers, e.g. for graceful server shutdown
    pub async fn stop_all(self: Arc<Self>) {
        let containers = self.containers.iter().map(|entry| entry.value().clone()).collect::<Vec<_>>();
        let tasks = containers
            .into_iter()
            .map(
                |container| {
                    let docker = self.docker.clone();
                    tokio::task::spawn(async move {
                        let state = container.state.lock().await;
                        if let Some(state) = state.as_ref() {
                            if let Err(e) = docker.stop_container(&state.docker_id, None).await {
                                tracing::error!(
                                    "Failed to stop container {} for {}: {}",
                                    state.docker_id,
                                    container.container_name(),
                                    e
                                );

                                return
                            }

                            if let Err(e) = docker.remove_container(
                                &state.docker_id,
                                Some(RemoveContainerOptions {
                                    force: false,
                                    ..Default::default()
                                })
                            ).await {
                                tracing::error!(
                                    "Failed to remove container {} for {}: {}",
                                    state.docker_id,
                                    container.container_name(),
                                    e
                                );

                                return
                            }

                            tracing::info!("Stopped and removed container {} for {}", state.docker_id, container.container_name());
                        }
                    })
                }
            );

        for future in tasks {
            if let Err(e) = future.await {
                tracing::error!("Failed to spawn task to stop Docker container: {e}");
            }
        }
    }

    /// Get a handle to the connected Docker client
    pub fn client(&self) -> &Docker {
        &self.docker
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DockerCreateContainerError {
    #[error("No such container '{0}'")]
    NoSuchContainer(String),
    #[error("{0}")]
    Other(#[from] container::ManagedContainerStartError),
}

#[derive(Debug, thiserror::Error)]
pub enum DockerServiceInitError {
    #[error("Docker API error: {0}")]
    Bollard(#[from] bollard::errors::Error),
    #[error("Failed to load container configs from directory {dir}: {err}")]
    ContainersDirError {
        dir: PathBuf,
        err: std::io::Error
    },
    #[error("Duplicate configurations detected for docker container with name {0}")]
    DuplicateConfiguration(String),
}
