use std::{collections::HashMap, path::PathBuf};

use bollard::Docker;

/// Configuration for a managed Docker container
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedContainerConfig {
    /// Name that identifies this container
    pub name: String,
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
    pub(super) config: ManagedContainerConfig,
    /// Directory that the container's config file was loaded from, used to build relative paths
    /// specified in the config
    dir: PathBuf,
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
        let config_file = tokio::fs::read_to_string(&config_path)
            .await
            .map_err(|err| ManagedContainerLoadError::ConfigFileIO { path: config_path, err})?;
        let config = toml::de::from_str::<ManagedContainerConfig>(&config_file)?;
        tracing::trace!("Found docker container with container name {}", config.name);

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
                        config
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
pub enum ManagedContainerStartError {
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
    #[error("Failed to parse config as TOML: {0}")]
    ConfigParse(#[from] toml::de::Error),
    #[error("Docker API error: {0}")]
    Bollard(#[from] bollard::errors::Error),
    #[error("Container config references nonexistent Docker image '{0}'. Try ensuring that you have pulled the image from a Docker registry")]
    MissingImage(String),
}
