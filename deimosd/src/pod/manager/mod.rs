use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc, time::Duration};

use bollard::Docker;

use super::{id::DeimosId, Pod};

mod config;

pub use config::{PodManagerConfig, DockerConnectionConfig, DockerConnectionType};

/// Manager responsible for orchestrating Docker containers and watching for external events and
/// failures
pub struct PodManager {
    config: PodManagerConfig,
    pub(super) docker: Docker,
    pods: HashMap<DeimosId, Arc<Pod>>,
}

impl PodManager {
    /// Load a config TOML file from the given path, and use the options specified inside to
    /// create a connection to the local Docker server, then load all pods from the directory
    /// given.
    pub async fn init(config: PodManagerConfig) -> Result<Self, PodManagerInitError> {
        let docker = match config.docker {
            None => Docker::connect_with_local_defaults()
                .map(|docker| docker.with_timeout(Duration::from_secs(DockerConnectionConfig::default_timeout()))),
            Some(ref conn) => match conn.kind {
                DockerConnectionType::Http => Docker::connect_with_http(&conn.addr, conn.timeout, bollard::API_DEFAULT_VERSION),
                DockerConnectionType::Local => Docker::connect_with_local(&conn.addr, conn.timeout, bollard::API_DEFAULT_VERSION),
            }
        }?;

        let pods = Self::load_containers(&config.containerdir).await?;
        if pods.is_empty() {
            tracing::warn!("Starting pod manager with no pods configured");
        }

        Ok(
            Self {
                config,
                docker,
                pods,
            }
        )
    }
    
    /// Get a reference to the pod with the given ID
    pub fn get(&self, id: &str) -> Option<Arc<Pod>> {
        self.pods.get(id).cloned()
    }
    
    /// Load all containers from directory entries in the given containers directory,
    /// logging errors and ignoring on failure
    async fn load_containers(dir: &Path) -> Result<HashMap<DeimosId, Arc<Pod>>, PodManagerInitError> {
        let mut pods = HashMap::new();

        let mut iter = tokio::fs::read_dir(dir)
            .await
            .map_err(|err| PodManagerInitError::PodRead { path: dir.to_owned(), err })?;

        loop {
            let entry = match iter.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(e) => {
                    tracing::error!("Failed to read directory entry from pod directory {}: {}", dir.display(), e);
                    continue
                }
            };
            
            let path = entry.path();

            match entry.file_type().await {
                Ok(ft) if ft.is_dir() => match Pod::load(&entry.path()).await {
                    Ok(pod) => {
                        pods.insert(pod.id(), Arc::new(pod));
                    },
                    Err(e) => {
                        tracing::error!("Failed to load container from {}: {}", path.display(), e);
                    },
                },
                Ok(..) => {
                    tracing::warn!("Ignoring non-directory entry {} in pod directory", path.display());
                },
                Err(e) => {
                    tracing::error!("Failed to get file type of entry {} in pod directory: {}", path.display(), e);
                }
            }
        }

        Ok(pods)
    }
    
    /// Get an immutable iterator over references to the managed pods
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (&'a DeimosId, &'a Arc<Pod>)> {
        self.pods.iter()
    }
}

impl<'a> IntoIterator for &'a PodManager {
    type Item = (&'a DeimosId, &'a Arc<Pod>);
    type IntoIter = std::collections::hash_map::Iter<'a, DeimosId, Arc<Pod>>;

    fn into_iter(self) -> Self::IntoIter {
        (&self.pods).into_iter()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PodManagerInitError {
    #[error("Failed to create Docker client: {0}")]
    Docker(#[from] bollard::errors::Error),
    #[error("Failed to read entries from pod directory {}: {}", path.display(), err)]
    PodRead {
        path: PathBuf,
        err: std::io::Error,
    }
}
