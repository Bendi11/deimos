use std::{
    io::Write, path::{Path, PathBuf}, str::FromStr, sync::Arc
};

use chrono::{DateTime, Utc};
use deimosproto::DeimosServiceClient;
use iced::widget::image;
use mime::Mime;
use tokio::sync::Mutex;
use tonic::transport::Channel;

use super::Context;

/// Data received from a server about a single container, cached locally.
/// Contains iced handles for resources used to display the container.
#[derive(Debug, Clone)]
pub struct CachedContainer {
    pub data: CachedContainerData,
    pub banner: Option<image::Handle>,
    pub icon: Option<image::Handle>,
}

/// Data to be serialized in a local cache file for a container
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedContainerData {
    pub id: String,
    pub name: String,
    pub up: CachedContainerUpStateFull,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CachedContainerUpState {
    Dead,
    Paused,
    Running,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CachedContainerUpStateFull {
    Known(CachedContainerUpState),
    UpdateRequested {
        old: CachedContainerUpState,
        req: CachedContainerUpState,
    }
}

impl Context {
    /// Save all cached container state to the local cache directory
    pub fn save_cached_containers(&self) {
        for container in self.containers.values() {
            if let Err(e) = container.save(&self.cache_dir) {
                tracing::error!("Failed to save container {}: {}", container.data.id, e);
            }
        }
    }

    /// Attempt to load all containers from the given local cache directory
    pub(super) async fn load_cached_containers(cache_dir: PathBuf) -> Vec<CachedContainer> {
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
                    "Failed to load cached containers from {}: {}",
                    cache_dir.display(),
                    e
                );
                return vec![];
            }
        };

        let mut containers = Vec::new();

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
                    let meta = match CachedContainerData::load(&path).await {
                        Ok(container) => container,
                        Err(e) => {
                            tracing::error!("Failed to load cached container {}: {} - it will be deleted and re-synchronized", path.display(), e);
                            if let Err(e) = tokio::fs::remove_dir(path.clone()).await {
                                tracing::error!(
                                    "Failed to delete erroneous cached container directory {}: {}",
                                    path.display(),
                                    e
                                );
                            }
                            continue;
                        }
                    };

                    
                    let full = CachedContainer::load(meta, &path).await;
                    containers.push(full)
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

        containers
    }
}

impl CachedContainerData {
    /// Load only the cached metadata for a cached container, without loading large images yet
    async fn load(directory: &Path) -> Result<Self, CachedContainerLoadError> {
        let meta_path = directory.join(CachedContainer::METADATA_FILE);
        let data_str = tokio::fs::read_to_string(&meta_path).await.map_err(|err| {
            CachedContainerLoadError::IO {
                path: meta_path,
                err,
            }
        })?;
        serde_json::from_str::<CachedContainerData>(&data_str).map_err(Into::into)
    }
    
    /// Write cached container metadata to a local cache directory
    fn save(&self, directory: &Path) -> Result<(), CachedContainerSaveError> {
        let meta_path = directory.join(CachedContainer::METADATA_FILE);

        let mut file = std::fs::File::create(&meta_path).map_err(|err| CachedContainerSaveError::IO {
            path: meta_path.clone(),
            err,
        })?;
        
        let bytes = serde_json::to_vec(self)?;
        file.write_all(&bytes).map_err(|err| CachedContainerSaveError::IO { path: meta_path, err })?;

        Ok(())
    }
}

impl CachedContainer {
    const METADATA_FILE: &str = "meta.json";
    const BANNER_FILENAME: &str = "banner";
    const ICON_FILENAME: &str = "icon";

    /// Check if the image with the given MIME type received from the server is supported by the
    /// frontend
    fn supported_image_mime(kind: Mime) -> bool {
        kind == mime::IMAGE_JPEG || kind == mime::IMAGE_PNG || kind == mime::IMAGE_BMP
    }

    /// Load a cached container from a local cache directory
    async fn load(data: CachedContainerData, directory: &Path) -> Self {
        tracing::trace!("Loading cached container from {}", directory.display());

        let banner = Self::load_image(directory.join(Self::BANNER_FILENAME)).await;
        let icon = Self::load_image(directory.join(Self::ICON_FILENAME)).await;

        Self { data, banner, icon }
    }

    /// Save all state to the filesystem, creating cache directories as required
    fn save(&self, cache_dir: &Path) -> Result<(), CachedContainerSaveError> {
        let dir = self.directory(cache_dir);
        if let Err(e) = std::fs::create_dir(&dir) {
            tracing::warn!("Failed to create directory '{}' for container {}: {}", dir.display(), self.data.id, e);
        }

        tracing::trace!("Saving container {} to {}", self.data.id, dir.display());
        self.data.save(&dir)?;

        Ok(())
    }

    /// Load an image, ignoring errors if it was not found and reporting them as warnings otherwise
    async fn load_image(from: PathBuf) -> Option<image::Handle> {
        match tokio::fs::read(&from).await {
            Ok(bytes) => Some(image::Handle::from_bytes(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                tracing::warn!("Failed to load image file '{}': {}. Should get replaced next container synchronization", from.display(), e);
                None
            }
        }
    }

    /// Get the directory that cache files for this container should be placed into
    fn directory(&self, cache_dir: &Path) -> PathBuf {
        cache_dir.join(&self.data.id)
    }
}

impl From<deimosproto::ContainerUpState> for CachedContainerUpState {
    fn from(value: deimosproto::ContainerUpState) -> Self {
        match value {
            deimosproto::ContainerUpState::Dead => Self::Dead,
            deimosproto::ContainerUpState::Paused => Self::Paused,
            deimosproto::ContainerUpState::Running => Self::Running,
        }
    }
}

impl From<CachedContainerUpState> for deimosproto::ContainerUpState {
    fn from(val: CachedContainerUpState) -> Self {
        match val {
            CachedContainerUpState::Dead => deimosproto::ContainerUpState::Dead,
            CachedContainerUpState::Paused => deimosproto::ContainerUpState::Paused,
            CachedContainerUpState::Running => deimosproto::ContainerUpState::Running
        }
    }
}

impl From<CachedContainerUpState> for CachedContainerUpStateFull {
    fn from(value: CachedContainerUpState) -> Self {
        Self::Known(value)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CachedContainerLoadError {
    #[error("I/O operation on file {}: {}", path.display(), err)]
    IO {
        path: PathBuf,
        #[source]
        err: std::io::Error,
    },
    #[error("Failed to parse cached container state: {0}")]
    Decode(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CachedContainerSaveError {
    #[error("I/O operation on file {}: {}", path.display(), err)]
    IO {
        path: PathBuf,
        #[source]
        err: std::io::Error,
    },
    #[error("Failed to serialize container state: {0}")]
    Encode(#[from] serde_json::Error),
}
