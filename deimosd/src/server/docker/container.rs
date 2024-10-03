use std::{collections::HashMap, path::PathBuf, sync::Arc, time::{SystemTime, SystemTimeError}};

use bollard::{container::RemoveContainerOptions, Docker};
use chrono::{DateTime, NaiveDateTime, Utc};
use tokio::{io::AsyncReadExt, sync::Mutex};

/// Configuration for a managed Docker container
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedContainerConfig {
    /// ID of the container, must remain constant over server renames
    pub id: Arc<str>,
    /// Name that identifies this container
    pub name: Arc<str>,
    /// Banner image to be displayed in user interfaces
    pub banner_path: Option<PathBuf>,
    /// Icon image to be displayed in user interfaces
    pub icon_path: Option<PathBuf>,
    /// Configuration for the Docker container
    pub docker: ManagedContainerDockerConfig,
}

/// Configuration to be passed to Docker when  starting this container
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedContainerDockerConfig {
    /// Docker image used to create the Docker container
    pub image: String,
    /// List of volumes to mount inside the container
    #[serde(default)]
    pub volume: Vec<ManagedContainerDockerMountConfig>,
    /// List of network ports to forward to the container
    #[serde(default)]
    pub port: Vec<ManagedContainerDockerPortConfig>,
    /// List of environment variables to define for the container
    #[serde(default)]
    pub env: Vec<ManagedContainerDockerEnvConfig>,
}

/// Configuration for a local volume mounted to a Docker container
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedContainerDockerMountConfig {
    pub local: PathBuf,
    pub container: PathBuf,
}

/// Configuration for a network port forwarded to the Docker container
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedContainerDockerPortConfig {
    pub expose: u16,
    pub protocol: ManagedContainerDockerPortProtocol,
    #[serde(default)]
    pub upnp: bool,
}

/// Selectable protocol for forwarded port
#[derive(Debug, serde::Deserialize)]
pub enum ManagedContainerDockerPortProtocol {
    #[serde(rename = "udp")]
    Udp,
    #[serde(rename = "udp")]
    Tcp,
}

/// Configuration for an environment variable to be set in the container
#[derive(Debug, serde::Deserialize)]
pub struct ManagedContainerDockerEnvConfig {
    pub key: String,
    pub value: String,
}

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
    pub(super) state: Mutex<Option<ManagedContainerState>>,
}

/// State populated after a Docker container is created for a [ManagedContainer]
pub struct ManagedContainerState {
    /// ID of the container running for this
    pub docker_id: Arc<str>,
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
                        |var| format!("{}=\"{}\"", var.key, var.value)
                    )
                    .collect()
            );

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
    
    /// Create a Docker container instance from the configuration given and rename it to match the
    /// name given in the config
    pub async fn create(&self, docker: Docker) -> Result<(), ManagedContainerError> {
        let config = self.docker_config();

        let response = docker
            .create_container::<String, String>(
                None,
                config
            )
            .await?;

        tracing::trace!("Created container with ID {} for {}", response.id, self.container_name());

        for warning in response.warnings {
            tracing::warn!("Warning when creating container {}: {}", self.container_name(), warning);
        }

        docker
            .rename_container(
                &response.id,
                bollard::container::RenameContainerOptions { name: self.container_name().to_owned() }
            )
            .await?;

        tracing::trace!("Renamed container {}", self.container_name());
        
        let mut state = self.state.lock().await;
        *state = Some(
            ManagedContainerState {
                docker_id: Arc::from(response.id)
            }
        );

        Ok(())
    }
    
    /// Stop and remove the Docker container for this managed container
    pub async fn destroy(self: Arc<Self>, docker: Docker) -> Result<(), ManagedContainerError> {
        let Some(ref state) = *self.state.lock().await else { return Ok(()) };
        docker.stop_container(&state.docker_id, None).await?;
        docker.remove_container(
            &state.docker_id,
            Some(RemoveContainerOptions {
                force: false,
                ..Default::default()
            })
        ).await?;

        tracing::info!("Stopped and removed container {} for {}", state.docker_id, self.container_name());

        Ok(())
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
        &self.config.name
    }
}

impl ManagedContainerDockerPortProtocol {
    /// Get the string to use when specifying the protocol to the Docker API
    pub const fn docker_name(&self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tcp => "tcp",
        }
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
