use std::{collections::HashMap, path::PathBuf, sync::Arc};

use bollard::Docker;
use config::ManagedContainerConfig;
use tokio::{
    io::AsyncReadExt,
    sync::{watch, Mutex},
};

pub mod config;
mod id;

pub use id::{DeimosId, DockerId};

use crate::server::upnp::{UpnpError, UpnpLease};

/// A managed container that represents a running or stopped container
/// Maintains several invariants of the Docker container manager.
/// - One mutating Docker request may take place at a time per container
/// - All mutations to the shared state (incl. those due to external events like OOM killer)
/// must be propogated to the gRPC server
/// - Immutable accesses to container data should access the most recent shared state without
/// blocking, regardless of if the state is currently being mutated by another task
pub struct ManagedContainer {
    /// Configuration provided in a directory for this container
    pub config: ManagedContainerConfig,
    /// Directory that the container's config file was loaded from, used to build relative paths
    /// specified in the config
    dir: PathBuf,
    /// Mutex here to allow only one mutating Docker request at a time
    tx: Mutex<watch::Sender<ManagedContainerShared>>,
    /// Receiver used to access the most recent instance of shared state without blocking
    pub(super) rx: watch::Receiver<ManagedContainerShared>,
}

/// A guard allowing mutation of the given container's shared state, representing a single
/// transaction with the Docker server.
pub struct ManagedContainerTransaction<'a> {
    container: &'a Arc<ManagedContainer>,
    tx: tokio::sync::MutexGuard<'a, watch::Sender<ManagedContainerShared>>,
}

/// State that may be mutated in transactions for a given [ManagedContainer]
#[derive(Clone)]
pub struct ManagedContainerShared {
    /// Status of the container in Docker
    pub directive: ManagedContainerDirective,
    /// Lease for ports requested from the UPnP manager
    pub upnp_lease: UpnpLease,
}

#[derive(Clone,)]
pub enum ManagedContainerDirective {
    Stop,
    Pause,
    Run(ManagedContainerRunDirective),
}

/// State managed for a manged container that should be running
#[derive(Clone,)]
pub struct ManagedContainerRunDirective {
    pub lease: UpnpLease,
}

impl<'a> ManagedContainerTransaction<'a> {
    /// Update the container's state according to operations performed in a transaction
    pub fn update(&self, state: ManagedContainerShared) {
        self.tx.send_replace(state);
    }

    /// Modify the current state with the provided function
    pub fn modify<F: FnOnce(&mut ManagedContainerShared)>(&self, fun: F) {
        self.tx.send_modify(fun)
    }

    /// Get the current state, to be modified and re-written
    pub fn state(&self) -> ManagedContainerShared {
        self.container.rx.borrow().clone()
    }

    /// Get the container that this transaction modifies
    pub fn container(&self) -> &Arc<ManagedContainer> {
        self.container
    }
}

impl<'a> AsRef<Arc<ManagedContainer>> for ManagedContainerTransaction<'a> {
    fn as_ref(&self) -> &Arc<ManagedContainer> {
        self.container
    }
}

impl ManagedContainer {
    const CONFIG_FILENAME: &str = "container.toml";

    /// Wait for all other transactions for this container to complete, then begin a new
    /// transaction allowing state changes
    pub async fn transaction(self: &Arc<Self>) -> ManagedContainerTransaction {
        ManagedContainerTransaction {
            container: self,
            tx: self.tx.lock().await,
        }
    }
    
    /// Attempt to start a transaction, returning `Some` if there are no ongoing transactions
    pub fn try_transaction(self: &Arc<Self>) -> Option<ManagedContainerTransaction> {
        self
            .tx
            .try_lock()
            .ok()
            .map(|tx| ManagedContainerTransaction { container: self, tx, })
    }

    /// Get a reference to the most recent shared state without blocking
    pub fn state(&self) -> watch::Ref<'_, Option<ManagedContainerShared>> {
        self.rx.borrow()
    }

    /// Load a new managed container from the given configuration file, ensuring that the image
    /// name given in the config exists in the local Docker engine
    pub(super) async fn load_from_dir(
        dir: PathBuf,
        docker: &Docker,
    ) -> Result<Self, ManagedContainerLoadError> {
        let config_path = dir.join(Self::CONFIG_FILENAME);
        tracing::trace!(
            "Loading container from config file {}",
            config_path.display()
        );

        let mut config_file = tokio::fs::File::open(&config_path).await.map_err(|err| {
            ManagedContainerLoadError::ConfigFileIO {
                path: config_path.clone(),
                err,
            }
        })?;

        let mut config_str = String::with_capacity(
            config_file.metadata().await.map(|m| m.len()).unwrap_or(512) as usize,
        );
        config_file
            .read_to_string(&mut config_str)
            .await
            .map_err(|err| ManagedContainerLoadError::ConfigFileIO {
                path: config_path.clone(),
                err,
            })?;

        let config = toml::de::from_str::<ManagedContainerConfig>(&config_str)?;
        tracing::trace!(
            "Found docker container with container name \"{}\"",
            config.name
        );

        let image_inspect = docker.inspect_image(&config.docker.image).await?;
        match image_inspect.id {
            Some(id) => {
                tracing::info!(
                    "Loaded container config {} with Docker image ID {}",
                    config.name,
                    id,
                );

                let (tx, rx) = watch::channel(None);
                let tx = Mutex::new(tx);

                Ok(Self {
                    dir,
                    config,
                    tx,
                    rx,
                })
            }
            None => Err(ManagedContainerLoadError::MissingImage(config.docker.image)),
        }
    }

    /// Get the container configuration options to use when creating a docker container
    pub(super) fn docker_config(&self) -> bollard::container::Config<String> {
        let image = Some(self.config.docker.image.clone());

        let exposed_ports = (!self.config.docker.port.is_empty()).then(|| {
            self.config
                .docker
                .port
                .iter()
                .map(|conf| {
                    (
                        format!("{}/{}", conf.expose, conf.protocol.docker_name()),
                        HashMap::new(),
                    )
                })
                .collect()
        });

        let env = (!self.config.docker.env.is_empty()).then(|| {
            self.config
                .docker
                .env
                .iter()
                .map(|var| format!("{}={}", var.key, var.value))
                .collect()
        });

        tracing::trace!("Env is {:#?}", env);

        let binds = (!self.config.docker.volume.is_empty()).then(|| {
            self.config
                .docker
                .volume
                .iter()
                .map(|volume| format!("{}:{}", volume.local.display(), volume.container.display()))
                .collect()
        });

        let host_config = Some(bollard::models::HostConfig {
            binds,
            ..Default::default()
        });

        bollard::container::Config {
            image,
            exposed_ports,
            env,
            host_config,
            ..Default::default()
        }
    }

    /// Get the name of the Docker container when run
    pub fn deimos_id(&self) -> &DeimosId {
        &self.config.id
    }
}

pub type BollardError = bollard::errors::Error;

#[derive(Debug, thiserror::Error)]
pub enum ManagedContainerError {
    #[error("Docker API error: {0}")]
    Bollard(#[from] BollardError),
    #[error("UPNP error: {0}")]
    Upnp(#[from] UpnpError),
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedContainerLoadError {
    #[error("Failed to load container from config file {path}: {err}")]
    ConfigFileIO { path: PathBuf, err: std::io::Error },
    #[error("Config file had invalid modified datetime {}", .0)]
    InvalidDateTime(std::time::SystemTimeError),
    #[error("Failed to parse config as TOML: {0}")]
    ConfigParse(#[from] toml::de::Error),
    #[error("Docker API error: {0}")]
    Bollard(#[from] bollard::errors::Error),
    #[error("Container config references nonexistent Docker image '{0}'. Try ensuring that you have pulled the image from a Docker registry")]
    MissingImage(String),
}
