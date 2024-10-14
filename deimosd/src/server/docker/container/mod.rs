use std::{collections::HashMap, path::PathBuf, sync::Arc, time::SystemTime};

use bollard::Docker;
use chrono::{DateTime, Utc};
use config::ManagedContainerConfig;
use tokio::{io::AsyncReadExt, sync::Mutex};


pub mod config;

/// A managed container that represents a running or stopped container
pub struct ManagedContainer {
    /// Configuration provided in a directory for this container
    pub config: ManagedContainerConfig,
    /// Directory that the container's config file was loaded from, used to build relative paths
    /// specified in the config
    dir: PathBuf,
    /// Date and time of the last modification made to the config file
    pub last_modified: DateTime<Utc>,
    /// State of the container
    pub state: Mutex<Option<ManagedContainerState>>,
}

/// State populated after a Docker container is created for a [ManagedContainer]
pub struct ManagedContainerState {
    /// ID of the container running for this
    pub docker_id: Arc<str>,
    /// Status of the container in Docker
    pub running: ManagedContainerRunning,
    /// Task listening for events propogated by the docker container
    pub listener: tokio::task::JoinHandle<()>,
}

#[derive(Clone, Copy)]
pub enum ManagedContainerRunning {
    Dead,
    Paused,
    Running
}


impl ManagedContainer {
    const CONFIG_FILENAME: &str = "container.toml";

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
        


        let mut config_file = tokio::fs::File::open(&config_path)
            .await
            .map_err(|err| ManagedContainerLoadError::ConfigFileIO { path: config_path.clone(), err})?;

        let metadata = config_file
            .metadata()
            .await
            .map_err(|err| ManagedContainerLoadError::ConfigFileIO { path: config_path.clone(), err })?;
    
        let mut config_str = String::with_capacity(config_file.metadata().await.map(|m| m.len()).unwrap_or(512) as usize);
        config_file
            .read_to_string(&mut config_str)
            .await
            .map_err(|err| ManagedContainerLoadError::ConfigFileIO { path: config_path.clone(), err })?;

        let config = toml::de::from_str::<ManagedContainerConfig>(&config_str)?;
        tracing::trace!("Found docker container with container name {}", config.name);
        
        let last_modified = metadata
            .modified()
            .map_err(|err| ManagedContainerLoadError::ConfigFileIO { path: config_path, err })?;

        let last_modified = last_modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(ManagedContainerLoadError::InvalidDateTime)?;

        let last_modified = DateTime::from_timestamp(last_modified.as_secs() as i64, last_modified.subsec_nanos())
            .expect("Last modified timestamp out of range?");

        let image_inspect = docker.inspect_image(&config.docker.image).await?;
        match image_inspect.id {
            Some(id) => {
                tracing::info!(
                    "Loaded container config {} with Docker image ID {}",
                    config.name,
                    id,
                );

                Ok(
                    Self {
                        dir,
                        config,
                        last_modified,
                        state: Mutex::new(None),
                    }
                )
            }
            None => Err(ManagedContainerLoadError::MissingImage(config.docker.image)),
        }
    }
    
    /// Get the container configuration options to use when creating a docker container
    pub(super) fn docker_config(&self) -> bollard::container::Config<String> {
        let image = Some(self.config.docker.image.clone());

        let exposed_ports = (!self.config.docker.port.is_empty())
            .then(||
                self
                    .config
                    .docker
                    .port
                    .iter()
                    .map(
                        |conf| (format!("{}/{}", conf.expose, conf.protocol.docker_name()), HashMap::new())
                    )
                    .collect()
            );

        let env = (!self.config.docker.env.is_empty())
            .then(||
                self
                    .config
                    .docker
                    .env
                    .iter()
                    .map(
                        |var| format!("{}={}", var.key, var.value)
                    )
                    .collect()
            );

        tracing::trace!("Env is {:#?}", env);

        let binds = (!self.config.docker.volume.is_empty())
            .then(||
                self
                    .config
                    .docker
                    .volume
                    .iter()
                    .map(
                        |volume| format!("{}:{}", volume.local.display(), volume.container.display())
                    )
                    .collect()
            );

        let host_config = Some(
            bollard::models::HostConfig {
                binds,
                ..Default::default()
            }
        );

        bollard::container::Config {
            image,
            exposed_ports,
            env,
            host_config,
            ..Default::default()
        }
    }
    
    /// Get the ID of the Docker container that has been created for this managed container, or
    /// `None` if no container exists
    pub async fn container_id(&self) -> Option<Arc<str>> {
        self
            .state
            .lock()
            .await
            .as_ref()
            .map(|s| s.docker_id.clone())
    }

    /// Get the name of the Docker container when run
    pub fn container_name(&self) -> &str {
        &self.config.id
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedContainerError {
    #[error("Docker API error: {0}")]
    Bollard(#[from] bollard::errors::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedContainerLoadError {
    #[error("Failed to load container from config file {path}: {err}")]
    ConfigFileIO {
        path: PathBuf,
        err: std::io::Error,
    },
    #[error("Config file had invalid modified datetime {}", .0)]
    InvalidDateTime(std::time::SystemTimeError),
    #[error("Failed to parse config as TOML: {0}")]
    ConfigParse(#[from] toml::de::Error),
    #[error("Docker API error: {0}")]
    Bollard(#[from] bollard::errors::Error),
    #[error("Container config references nonexistent Docker image '{0}'. Try ensuring that you have pulled the image from a Docker registry")]
    MissingImage(String),
}
