use std::{path::{Path, PathBuf}, str::FromStr, sync::Arc};

use chrono::{DateTime, Utc};
use deimos_shared::{ContainerBrief, ContainerImage, ContainerImagesRequest, ContainerImagesResponse, QueryContainersRequest};
use iced::widget::image;
use mime::Mime;

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
    /// Synchronize containers from a received list of container data from the server
    pub async fn synchronize_containers(self: Arc<Self>) {
        let request = QueryContainersRequest {
            updated_since: self.state.last_sync.map(|dt| dt.timestamp()),
        };
        
        let list = {
            let mut api = self.api.lock().await;
            match api.query_containers(request).await {
                Ok(resp) => resp.into_inner().containers,
                Err(e) => {
                    tracing::error!("Failed to query containers from the server: {e}");
                    return
                }
            }
        };

        for item in list {
            self.synchronize_container_from_brief(item).await;
        }
    }
    
    /// Check if the received container is already cached at its newest version, and request full
    /// container data including images if it is not
    async fn synchronize_container_from_brief(&self, brief: ContainerBrief) {
        let Some(updated) = DateTime::from_timestamp(brief.updated, 0) else {
            tracing::warn!("Failed to create timestamp from container brief timestamp {}", brief.updated);
            return
        };
        
        {
            let containers = self.containers.read().await;
            match containers.get(&brief.id) {
                Some(existing) if existing.data.last_update >= updated => {
                    tracing::info!("Skipping update for received container {}, we already have the newest version", brief.id);
                    return
                },
                _ => ()
            };
        }
        
        let images = {
            let mut api = self.api.lock().await;
            let request = ContainerImagesRequest { container_id: brief.id.clone() };
            match api.get_container_image(request).await {
                Ok(images) => images.into_inner(),
                Err(e) => {
                    tracing::error!("Failed to get images for {}: {}", brief.id, e);
                    ContainerImagesResponse { banner: None, icon: None }
                }
            }
        };

        let load_if_supported = |image: ContainerImage| {
            let mime = match Mime::from_str(&image.mime_type) {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Invalid image MIME type '{}' from server for container {}: {}", image.mime_type, brief.id, e);
                    return None
                }
            };

            CachedContainer::supported_image_mime(mime)
                .then(|| image::Handle::from_bytes(image.image_data))
        };

        let banner = images.banner.and_then(load_if_supported);
        let icon = images.icon.and_then(load_if_supported);

        let data = CachedContainerData {
            id: brief.id,
            name: brief.title,
            last_update: DateTime::from_timestamp(brief.updated, 0).unwrap_or_default()
        };

        let container = CachedContainer {
            data,
            banner,
            icon,
        };

        self.containers.write().await.insert(container.data.id.clone(), Arc::new(container));   
    }
    
    /// Attempt to load all containers from the given local cache directory
    pub(super) async fn load_cached_containers(&self, dir: &Path) {
        if !dir.exists() {
            if let Err(e) = tokio::fs::create_dir(dir).await {
                tracing::error!("Failed to create cache directory '{}': {}", dir.display(), e);
            }
        }

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
                    let meta = match CachedContainerData::load(&path).await {
                        Ok(container) => container,
                        Err(e) => {
                            tracing::error!("Failed to load cached container {}: {} - it will be deleted and re-synchronized", path.display(), e);
                            if let Err(e) = tokio::fs::remove_dir(path.clone()).await {
                                tracing::error!("Failed to delete erroneous cached container directory {}: {}", path.display(), e);
                            }
                            continue
                        }
                    };

                    match containers.get(&meta.id) {
                        Some(existing) if existing.data.last_update >= meta.last_update => continue,
                        _ => {
                            let full = CachedContainer::load(meta, &path).await;
                            containers.insert(full.data.id.clone(), Arc::new(full));
                        }
                    }
                },
                Ok(_) => (),
                Err(e) => {
                    tracing::warn!("Failed to get file type for local cache directory entry {}: {}", entry.path().display(), e);
                }
            }
        }
    }
}

impl CachedContainerData {
    /// Load only the cached metadata for a cached container, without loading large images yet
    async fn load(directory: &Path) -> Result<Self, CachedContainerLoadError> {
        let meta_path = directory.join(CachedContainer::METADATA_FILE);
        let data_str = tokio::fs::read_to_string(&meta_path)
            .await
            .map_err(|err| CachedContainerLoadError::IO { path: meta_path, err })?;
        serde_json::from_str::<CachedContainerData>(&data_str)
            .map_err(Into::into)
    }
}

impl CachedContainer {
    const METADATA_FILE: &str = "meta.json";
    const BANNER_FILENAME: &str = "banner";
    const ICON_FILENAME: &str = "icon";
    
    /// Check if the image with the given MIME type received from the server is supported by the
    /// frontend
    fn supported_image_mime(kind: Mime) -> bool {
        kind == mime::IMAGE_JPEG ||
        kind == mime::IMAGE_PNG ||
        kind == mime::IMAGE_BMP
    }
    
    /// Load a cached container from a local cache directory
    async fn load(data: CachedContainerData, directory: &Path) -> Self {
        tracing::trace!("Loading cached container from {}", directory.display());
 
        let banner = Self::load_image(directory.join(Self::BANNER_FILENAME)).await;
        let icon = Self::load_image(directory.join(Self::ICON_FILENAME)).await;

        Self {
            data,
            banner,
            icon,
        }
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
