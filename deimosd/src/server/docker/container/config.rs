use std::{path::PathBuf, sync::Arc};

use super::DeimosId;

/// Configuration for a managed Docker container
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedContainerConfig {
    /// ID of the container, must remain constant over server renames
    pub id: DeimosId,
    /// Name that identifies this container
    pub name: Arc<str>,
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
#[derive(Clone, Copy, Debug, serde::Deserialize)]
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

impl ManagedContainerDockerPortProtocol {
    /// Get the string to use when specifying the protocol to the Docker API
    pub const fn docker_name(&self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tcp => "tcp",
        }
    }
}

impl From<ManagedContainerDockerPortProtocol> for igd_next::PortMappingProtocol {
    fn from(value: ManagedContainerDockerPortProtocol) -> Self {
        match value {
            ManagedContainerDockerPortProtocol::Udp => Self::UDP,
            ManagedContainerDockerPortProtocol::Tcp => Self::TCP,
        }
    }
}
