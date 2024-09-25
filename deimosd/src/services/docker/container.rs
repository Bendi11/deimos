use std::path::PathBuf;

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
}

/// Configuration for a local volume mounted to a Docker container
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedContainerDockerMountConfig {
    pub local: PathBuf,
    pub container: PathBuf,
}

/// A managed container that represents a running or stopped container
pub struct ManagedContainer {
    /// Configuration provided in a directory for this container
    pub(super) config: ManagedContainerConfig,
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

                Ok(Self { config })
            }
            None => Err(ManagedContainerLoadError::MissingImage(config.docker.image)),
        }
    }

    /// Get the name of the Docker container when run
    pub fn container_name(&self) -> &str {
        &self.config.name
    }
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
