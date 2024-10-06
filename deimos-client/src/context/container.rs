use std::{path::{Path, PathBuf}, sync::Arc};

use chrono::{DateTime, Utc};
use iced::widget::image;

use super::Context;


/// Data received from a server about a single container, cached locally.
/// Contains iced handles for resources used to display the container.
#[derive(Debug)]
pub struct CachedContainer {
    pub data: CachedContainerData,
    pub banner: Option<image::Handle>,
    pub icon: Option<image::Handle>,
}

/// Data to be serialized in a local cache file for a container
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CachedContainerData {
    pub id: String,
    pub name: String,
    pub last_update: DateTime<Utc>,
}

impl Context {
    pub async fn synchronize_container(self: Arc<Self>) {

    }
    
    /// Attempt to load all containers from the given local cache directory
    async fn load_cached_containers(&self, dir: &Path) {
        let mut iter = match tokio::fs::read_dir(dir).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to load cached containers from {}: {}", dir.display(), e);
                return
            }
        };

        let mut containers = self.containers.write().await;
        
        loop {
            let entry = match iter.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!("Failed to get entry from directory {}: {}", dir.display(), e);
                    continue
                }
            };
            
            match entry.file_type().await {
                Ok(ft) if ft.is_dir() => {
                    let path = entry.path();
                    let cached = match CachedContainer::load(&path).await {
                        Ok(container) => container,
                        Err(e) => {
                            tracing::error!("Failed to load cached container {}: {} - it will be deleted and re-synchronized", path.display(), e);
                            if let Err(e) = tokio::fs::remove_dir(path.clone()).await {
                                tracing::error!("Failed to delete erroneous cached container directory {}: {}", path.display(), e);
                            }
                            continue
                        }
                    };

                    containers.insert(cached.data.id.clone(), Arc::new(cached));
                },
                Ok(_) => (),
                Err(e) => {
                    tracing::warn!("Failed to get file type for local cache directory entry {}: {}", entry.path().display(), e);
                }
            }
        }
    }
}

impl CachedContainer {
    const METADATA_FILE: &str = "meta.json";
    const BANNER_FILENAME: &str = "banner";
    const ICON_FILENAME: &str = "icon";
    
    /// Load a cached container from a local cache directory
    async fn load(directory: &Path) -> Result<Self, CachedContainerLoadError> {
        tracing::trace!("Loading cached container from {}", directory.display());

        let meta_path = directory.join(Self::METADATA_FILE);
        let data_str = tokio::fs::read_to_string(&meta_path)
            .await
            .map_err(|err| CachedContainerLoadError::IO { path: meta_path, err })?;
        let data = serde_json::from_str::<CachedContainerData>(&data_str)?;

        let banner = Self::load_image(directory.join(Self::BANNER_FILENAME)).await;
        let icon = Self::load_image(directory.join(Self::ICON_FILENAME)).await;

        Ok(
            Self {
                data,
                banner,
                icon,
            }
        )
    }
    
    /// Save all state to the filesystem, creating cache directories as required
    async fn save(&self, cache_dir: &Path) -> Result<(), CachedContainerLoadError> {
        let dir = self.directory(cache_dir);
        tracing::trace!("Saving container {} to {}", self.data.id, dir.display());
        

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
