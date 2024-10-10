use std::{path::PathBuf, sync::Arc};

use bollard::Docker;
use dashmap::DashMap;

use super::container::ManagedContainer;



/// Service managing the creation and removal of Docker containers
pub struct DockerState {
    pub config: DockerConfig,
    pub docker: Docker,
    /// Map of managed container IDs to their config and state
    pub containers: DashMap<String, Arc<ManagedContainer>>,
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

impl DockerState {
    /// Default timeout to use for Docker API when no alternative is specified in the config
    pub const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

    /// Load the docker service from a config file specifying how to connect to the local Docker
    /// engine and where to load container configurations from
    pub async fn new(config: DockerConfig) -> Result<Self, DockerInitError> {
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
            .map_err(|err| DockerInitError::ContainersDirError { dir: dir.clone(), err })?;

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
                },
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
                        return Err(DockerInitError::DuplicateConfiguration(name));
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
}


#[derive(Debug, thiserror::Error)]
pub enum DockerInitError {
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
