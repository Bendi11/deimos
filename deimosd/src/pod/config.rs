use std::{path::PathBuf, sync::Arc};

use super::id::DeimosId;

/// Top-level configuration for a Pod, parsed from TOML files
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PodConfig {
    /// ID of the container, must remain constant over server renames
    pub id: DeimosId,
    /// Name that identifies this container
    pub name: Arc<str>,
    /// Configuration for the Docker container
    pub docker: PodDockerConfig,
}

/// Configuration to be passed to Docker when  starting this container
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PodDockerConfig {
    /// Docker image used to create the Docker container
    pub image: String,
    /// Time to wait in seconds before forcefully killing the container
    #[serde(default = "PodDockerConfig::default_stop_timeout")]
    pub stop_timeout: u32,
    /// List of volumes to mount inside the container
    #[serde(default)]
    pub volume: Vec<PodDockerMountConfig>,
    /// List of network ports to forward to the container
    #[serde(default)]
    pub port: Vec<PodDockerPortConfig>,
    /// List of environment variables to define for the container
    #[serde(default)]
    pub env: Vec<PodDockerEnvConfig>,
    /// List of capabilities to add to the process
    #[serde(default)]
    pub cap_add: Vec<String>,
}


/// Configuration for a local volume mounted to a Docker container
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PodDockerMountConfig {
    pub local: PathBuf,
    pub container: PathBuf,
}

/// Configuration for a network port forwarded to the Docker container
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PodDockerPortConfig {
    pub expose: u16,
    pub protocol: PodDockerPortProtocol,
    #[serde(default)]
    pub upnp: bool,
}

/// Selectable protocol for forwarded port
#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub enum PodDockerPortProtocol {
    #[serde(rename = "udp")]
    Udp,
    #[serde(rename = "tcp")]
    Tcp,
}

/// Configuration for an environment variable to be set in the container
#[derive(Debug, serde::Deserialize)]
pub struct PodDockerEnvConfig {
    pub key: String,
    pub value: String,
}

/// Configuration for the pod manager including state to connect to the local Docker server and
/// load all Pods from their configuration files
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PodManagerConfig {
    pub containerdir: PathBuf,
    pub docker: Option<DockerConnectionConfig>,
}

/// Configuration governing how the server will connect to the Docker API
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockerConnectionConfig {
    pub kind: DockerConnectionType,
    pub addr: String,
    #[serde(default = "DockerConnectionConfig::default_timeout")]
    pub timeout: u64,
}

#[derive(Debug, serde::Deserialize)]
pub enum DockerConnectionType {
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "local")]
    Local,
}

impl DockerConnectionConfig {
    /// Helper function for serde deserializer defaults
    pub const fn default_timeout() -> u64 {
        60 * 3
    }
}

impl PodDockerPortProtocol {
    /// Get the string to use when specifying the protocol to the Docker API
    pub const fn docker_name(&self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tcp => "tcp",
        }
    }
}

impl From<PodDockerPortProtocol> for igd_next::PortMappingProtocol {
    fn from(value: PodDockerPortProtocol) -> Self {
        match value {
            PodDockerPortProtocol::Udp => Self::UDP,
            PodDockerPortProtocol::Tcp => Self::TCP,
        }
    }
}

impl PodDockerConfig {
    /// Helper function for providing a default timeout when serde does not find one specified
    pub const fn default_stop_timeout() -> u32 {
        60
    }
}
