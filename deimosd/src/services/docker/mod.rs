use std::{path::PathBuf, sync::Arc};

use bollard::Docker;
use container::ManagedContainer;
use dashmap::DashMap;
use tokio_util::sync::CancellationToken;

pub mod container;

/// Service managing the creation and removal of Docker containers
pub struct DockerService {
    pub config: DockerConfig,
    docker: Docker,
    /// Map of container names to their state
    containers: DashMap<String, ManagedContainer>,
}

/// Configuration for the local Docker container management service
#[derive(Debug, serde::Deserialize)]
pub struct DockerConfig {
    pub containerdir: PathBuf,
    pub conn: Option<DockerConnectionConfig>,
}

/// Configuration governing how the server will connect to the Docker API
#[derive(Debug, serde::Deserialize)]
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
    pub const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

    /// Load the docker service from a config file specifying how to connect to the local Docker
    /// engine and where to load container configurations from
    pub async fn new(config: DockerConfig) -> Result<Self, DockerServiceInitError> {
        let docker = match config.conn {
            None => {
                tracing::trace!("No docker config given, using platform defaults to connect");
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

        let mut container_entries = tokio::fs::read_dir(&config.containerdir).await?;
        while let Some(entry) = container_entries.next_entry().await? {
            let container = match entry.file_type().await {
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
                    if containers.insert(name.clone(), c).is_some() {
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

        Ok(Self {
            config,
            docker,
            containers,
        })
    }

    pub async fn run(self: Arc<Self>, cancel: CancellationToken) {}

    /// Get a handle to the connected Docker client
    pub fn client(&self) -> &Docker {
        &self.docker
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DockerServiceInitError {
    #[error("Docker API error: {0}")]
    Bollard(#[from] bollard::errors::Error),
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Duplicate configurations detected for docker container with name {0}")]
    DuplicateConfiguration(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DockerContainerInitError {
    #[error("Docker API error: {0}")]
    Bollard(#[from] bollard::errors::Error),
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
}
