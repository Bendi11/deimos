use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use iced::widget::{image, Image};

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
    pub async fn synchronize_container() {}
}

impl CachedContainer {
    const BANNER_FILENAME: &str = "banner";
    const ICON_FILENAME: &str = "icon";
    
    /// Load a cached container from a local cache directory
    async fn load(directory: &Path) -> Result<Self, CachedContainerLoadError> {
        let data = CachedContainerData::load(directory).await?;
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
    
    /// Load an image, ignoring errors if it was not found and reporting them as warnings otherwise
    async fn load_image(from: PathBuf) -> Option<image::Handle> {
        match tokio::fs::read(&from).await {
            Ok(bytes) => Some(image::Handle::from_bytes(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                tracing::warn!("Failed to load file '{}': {}. Should get replaced next container synchronization", from.display(), e);
                None
            }
        }
    }
}

impl CachedContainerData {
    const METADATA_FILE: &str = "meta.json";

    /// Get the local directory where container state is cached in files
    fn directory(&self, cache_dir: &Path) -> PathBuf {
        cache_dir.join(&self.id)
    }
    
    /// Load and attempt to deserialize the cached container state
    async fn load(directory: &Path) -> Result<Self, CachedContainerLoadError> {
        let path = directory.join(Self::METADATA_FILE);
        let metadata = tokio::fs::read_to_string(&path)
            .await
            .map_err(|err| CachedContainerLoadError::IO { path, err })?;
        serde_json::from_str::<Self>(&metadata)
            .map_err(Into::into)
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
