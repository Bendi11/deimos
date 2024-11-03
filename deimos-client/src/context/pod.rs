use std::{
    io::Write,
    path::{Path, PathBuf}, sync::Arc,
};

use im::HashMap;

use super::{Context, NotifyMutation};

/// Data received from a server about a single container, cached locally.
/// Contains iced handles for resources used to display the container.
#[derive(Debug, Clone)]
pub struct CachedPod {
    pub data: CachedPodData,
}

/// Data to be serialized in a local cache file for a container
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedPodData {
    pub id: String,
    pub name: String,
    pub up: NotifyMutation<CachedPodState>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum CachedPodState {
    Disabled,
    Transit,
    Paused,
    Enabled,
}

impl Context {
    /// Save all cached pod state to the local cache directory
    pub fn save_cached_pods(&self) {
        for container in self.pods.read().values() {
            if let Err(e) = container.save(&self.cache_dir) {
                tracing::error!("Failed to save container {}: {}", container.data.id, e);
            }
        }
    }

    /// Attempt to load all pods from the given local cache directory
    pub(super) async fn load_cached_pods(cache_dir: PathBuf) -> HashMap<String, Arc<CachedPod>> {
        if !cache_dir.exists() {
            if let Err(e) = tokio::fs::create_dir(&cache_dir).await {
                tracing::error!(
                    "Failed to create cache directory '{}': {}",
                    cache_dir.display(),
                    e
                );
            }
        }

        let mut iter = match tokio::fs::read_dir(&cache_dir).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(
                    "Failed to read cached pods directory {}: {}",
                    cache_dir.display(),
                    e
                );
                return HashMap::default();
            }
        };

        let mut pods = HashMap::default();

        loop {
            let entry = match iter.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!(
                        "Failed to get entry from directory {}: {}",
                        cache_dir.display(),
                        e
                    );
                    continue;
                }
            };

            match entry.file_type().await {
                Ok(ft) if ft.is_dir() => {
                    let path = entry.path();
                    let meta = match CachedPodData::load(&path).await {
                        Ok(container) => container,
                        Err(e) => {
                            tracing::error!("Failed to load cached pod {}: {} - it will be deleted and re-synchronized", path.display(), e);
                            if let Err(e) = tokio::fs::remove_dir_all(path.clone()).await {
                                tracing::error!(
                                    "Failed to delete erroneous cached pod directory {}: {}",
                                    path.display(),
                                    e
                                );
                            }
                            continue;
                        }
                    };

                    let full = CachedPod::load(meta, &path).await;
                    pods.insert(full.data.id.clone(), Arc::new(full));
                }
                Ok(_) => (),
                Err(e) => {
                    tracing::warn!(
                        "Failed to get file type for local cache directory entry {}: {}",
                        entry.path().display(),
                        e
                    );
                }
            }
        }

        pods
    }
}

impl CachedPodData {
    /// Load only the cached metadata for a cached container, without loading large images yet
    async fn load(directory: &Path) -> Result<Self, CachedPodLoadError> {
        let meta_path = directory.join(CachedPod::METADATA_FILE);
        let data_str =
            tokio::fs::read_to_string(&meta_path)
                .await
                .map_err(|err| CachedPodLoadError::IO {
                    path: meta_path,
                    err,
                })?;
        serde_json::from_str::<CachedPodData>(&data_str).map_err(Into::into)
    }

    /// Write cached container metadata to a local cache directory
    fn save(&self, directory: &Path) -> Result<(), CachedPodSaveError> {
        let meta_path = directory.join(CachedPod::METADATA_FILE);

        let mut file = std::fs::File::create(&meta_path).map_err(|err| CachedPodSaveError::IO {
            path: meta_path.clone(),
            err,
        })?;

        let bytes = serde_json::to_vec(self)?;
        file.write_all(&bytes)
            .map_err(|err| CachedPodSaveError::IO {
                path: meta_path,
                err,
            })?;

        Ok(())
    }
}

impl CachedPod {
    const METADATA_FILE: &str = "meta.json";

    /// Load a cached container from a local cache directory
    async fn load(data: CachedPodData, directory: &Path) -> Self {
        tracing::trace!("Loading cached container from {}", directory.display());

        Self { data }
    }

    /// Save all state to the filesystem, creating cache directories as required
    fn save(&self, cache_dir: &Path) -> Result<(), CachedPodSaveError> {
        let dir = self.directory(cache_dir);
        if let Err(e) = std::fs::create_dir(&dir) {
            tracing::warn!(
                "Failed to create directory '{}' for pod {}: {}",
                dir.display(),
                self.data.id,
                e
            );
        }

        tracing::trace!("Saving pod {} to {}", self.data.id, dir.display());
        self.data.save(&dir)?;

        Ok(())
    }

    /// Get the directory that cache files for this container should be placed into
    fn directory(&self, cache_dir: &Path) -> PathBuf {
        cache_dir.join(&self.data.id)
    }
}

impl From<deimosproto::PodState> for CachedPodState {
    fn from(value: deimosproto::PodState) -> Self {
        match value {
            deimosproto::PodState::Disabled => Self::Disabled,
            deimosproto::PodState::Transit => Self::Transit,
            deimosproto::PodState::Paused => Self::Paused,
            deimosproto::PodState::Enabled => Self::Enabled,
        }
    }
}

impl From<CachedPodState> for deimosproto::PodState {
    fn from(val: CachedPodState) -> Self {
        match val {
            CachedPodState::Transit => deimosproto::PodState::Transit,
            CachedPodState::Disabled => deimosproto::PodState::Disabled,
            CachedPodState::Paused => deimosproto::PodState::Paused,
            CachedPodState::Enabled => deimosproto::PodState::Enabled,
        }
    }
}


#[derive(Debug, thiserror::Error)]
pub enum CachedPodLoadError {
    #[error("I/O operation on file {}: {}", path.display(), err)]
    IO {
        path: PathBuf,
        #[source]
        err: std::io::Error,
    },
    #[error("Failed to parse cached pod state: {0}")]
    Decode(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CachedPodSaveError {
    #[error("I/O operation on file {}: {}", path.display(), err)]
    IO {
        path: PathBuf,
        #[source]
        err: std::io::Error,
    },
    #[error("Failed to serialize pod state: {0}")]
    Encode(#[from] serde_json::Error),
}
