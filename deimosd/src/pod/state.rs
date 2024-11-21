use std::path::{Path, PathBuf};

use crate::server::upnp::UpnpLease;

use super::{config::PodConfig, id::{DeimosId, DockerId}};

mod handle;

pub use handle::{PodStateHandle, PodStateWriteHandle, PodStateReadHandle};

/// Represents a single pod with associated config and running Docker container if any exists
pub struct Pod {
    config: PodConfig,
    state: PodStateHandle,
}

/// Current state of a pod - including if the state is currently unknown and being modified
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PodState {
    Disabled,
    Transit,
    Paused,
    Enabled,
}

/// State of a pod with the guarantee that the state is always known
#[derive(Clone)]
pub enum PodStateKnown {
    Disabled,
    Paused(PodPaused),
    Enabled(PodEnable),
}

/// State maintained for a pod that is running
#[derive(Clone)]
pub struct PodEnable {
    pub docker_id: DockerId,
    pub upnp_lease: UpnpLease,
}

/// State maintained for a pod that has been paused and can be quickly restarted
#[derive(Clone)]
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
    
    /// Get a handle to access this pod's state
    pub fn state(&self) -> &PodStateHandle {
        &self.state
    }

    /// Get immutable access to the pod's configuration data
    pub fn config(&self) -> &PodConfig {
        &self.config
    }

    /// Load the pod from config files located in the given directory
    pub(super) async fn load(dir: &Path) -> Result<Self, PodLoadError> {
        const CONFIG_FILENAME: &str = "pod.toml";

        let path = dir.join(CONFIG_FILENAME);
        let config_str = tokio::fs::read_to_string(&path)
            .await
            .map_err(|err| PodLoadError::ConfigRead { path, err })?;

        let config = toml::from_str(&config_str)?;
        let state = PodStateHandle::new(PodStateKnown::Disabled);

        Ok(Self { config, state })
    }
}

impl From<&PodStateKnown> for PodState {
    fn from(value: &PodStateKnown) -> Self {
        match value {
            PodStateKnown::Disabled => PodState::Disabled,
            PodStateKnown::Paused(..) => PodState::Paused,
            PodStateKnown::Enabled(..) => PodState::Enabled,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PodLoadError {
    #[error("Failed to read config file {}: {}", path.display(), err)]
    ConfigRead { path: PathBuf, err: std::io::Error },
    #[error("Failed to parse config file: {0}")]
    ConfigParse(#[from] toml::de::Error),
}
