use std::path::{Path, PathBuf};

use config::PodConfig;
use id::{DeimosId, DockerId};
use tokio::sync::Mutex;


pub mod config;
pub mod id;
pub mod docker;
pub mod manager;


/// Represents a single pod with associated config and running Docker container if any exists
pub struct Pod {
    config: PodConfig,
    state: Mutex<PodState>,
}

/// Current state of a pod
enum PodState {
    Disabled,
    Paused(PodPaused),
    Enabled(PodRun),
}

pub struct PodRun {
    pub docker_id: DockerId,
}

pub struct PodPaused {
    pub docker_id: DockerId,
}

impl Pod {
    /// Get the user-visible title for this container
    pub fn title(&self) -> &str {
        &self.config.name
    }
    
    /// Get the ID used to refer to the container in API requests
    pub fn id(&self) -> DeimosId {
        self.config.id.clone()
    }
    
    /// Load the pod from config files located in the given directory
    async fn load(dir: &Path) -> Result<Self, PodLoadError> {
        const CONFIG_FILENAME: &str = "pod.toml";

        let path = dir.join(CONFIG_FILENAME);
        let config_str = tokio::fs::read_to_string(&path)
            .await
            .map_err(|err| PodLoadError::ConfigRead { path, err })?;

        let config = toml::from_str(&config_str)?;

        let state = Mutex::new(PodState::Disabled);

        Ok(
            Self {
                config,
                state,
            }
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PodLoadError {
    #[error("Failed to read config file {}: {}", path.display(), err)]
    ConfigRead {
        path: PathBuf,
        err: std::io::Error,
    },
    #[error("Failed to parse config file: {0}")]
    ConfigParse(#[from] toml::de::Error),
}
