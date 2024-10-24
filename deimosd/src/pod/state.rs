use std::{path::{Path, PathBuf}, sync::Arc};

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::server::{upnp::UpnpLease, Deimos};

use super::{config::PodConfig, id::{DeimosId, DockerId}};

/// Represents a single pod with associated config and running Docker container if any exists
pub struct Pod {
    pub(super) config: PodConfig,
    pub(super) state: PodStateHandle,
}

pub struct PodStateHandle {
    pub(super) lock: Mutex<PodStateKnown>,
    pub(super) tx: tokio::sync::watch::Sender<PodState>,
}

/// A handle allowing mutations to the state of a [Pod]
pub struct PodStateWriteHandle<'a> {
    lock: tokio::sync::MutexGuard<'a, PodStateKnown>,
    tx: tokio::sync::watch::Sender<PodState>,
}

/// Current state of a pod - including if the state is currently unknown and being modified
#[derive(Debug, Clone, Copy)]
pub enum PodState {
    Disabled,
    Transit,
    Paused,
    Enabled,
}

/// State of a pod with the guarantee that the state is always known
pub enum PodStateKnown {
    Disabled,
    Paused(PodPaused),
    Enabled(PodEnable),
}

/// State maintained for a pod that is running
pub struct PodEnable {
    pub docker_id: DockerId,
    pub upnp_lease: UpnpLease,
}

/// State maintained for a pod that has been paused and can be quickly restarted
pub struct PodPaused {
    pub docker_id: DockerId,
}

impl Deimos {
    pub async fn pod_task(self: Arc<Self>, cancel: CancellationToken) {
        cancel.cancelled().await;
        self.pods.disable_all().await;
    }
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

    /// Get an immutable reference to the current state
    pub fn state(&self) -> PodState {
        self.state.current()
    }

    /// Wait until other mutable accesses to the current state have finished, then acquire a lock
    /// and return
    pub async fn state_lock(&self) -> PodStateWriteHandle {
        self.state.lock().await
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

impl PodStateHandle {
    fn new(state: PodStateKnown) -> Self {
        let (tx, _) = tokio::sync::watch::channel(PodState::from(&state));
        let lock = Mutex::new(state);

        Self { lock, tx }
    }

    /// Lock the handle to allow mutations to the current state
    pub async fn lock(&self) -> PodStateWriteHandle {
        PodStateWriteHandle {
            lock: self.lock.lock().await,
            tx: self.tx.clone(),
        }
    }

    /// Get the current state
    pub fn current(&self) -> PodState {
        self.lock
            .try_lock()
            .as_deref()
            .map(Into::into)
            .unwrap_or(PodState::Transit)
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

impl<'a> PodStateWriteHandle<'a> {
    /// Get an immutable reference to the current state
    pub fn state(&self) -> &PodStateKnown {
        &self.lock
    }

    /// Set the current state to the given value
    pub fn set(&mut self, state: PodStateKnown) {
        self.tx.send_replace((&state).into());
        *self.lock = state;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PodLoadError {
    #[error("Failed to read config file {}: {}", path.display(), err)]
    ConfigRead { path: PathBuf, err: std::io::Error },
    #[error("Failed to parse config file: {0}")]
    ConfigParse(#[from] toml::de::Error),
}
