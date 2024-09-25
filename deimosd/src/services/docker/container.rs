use std::path::PathBuf;



/// Configuration for a managed Docker container
#[derive(Debug, serde::Deserialize)]
pub struct ManagedContainerConfig {
    /// Name that identifies this container
    pub name: String,
    /// Image to be displayed in user interfaces
    pub image: Option<PathBuf>,
    /// Configuration for the Docker container
    pub docker: ManagedContainerDockerConfig,
}

/// Configuration to be passed to Docker when  starting this container
#[derive(Debug, serde::Deserialize)]
pub struct ManagedContainerDockerConfig {
    /// Docker image used to create the Docker container
    pub image: String,
    /// List of volumes to mount inside the container
    pub volumes: Vec<String>,
}

/// A managed container that represents a running or stopped container
pub struct ManagedContainer {
    /// Configuration provided in a directory for this container
    pub(super) config: ManagedContainerConfig,
}


impl ManagedContainer {
    /// Get the name of the Docker container when ran
    pub fn container_name(&self) -> &str {
        &self.config.name
    }
}
