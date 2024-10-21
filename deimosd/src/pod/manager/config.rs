use std::path::PathBuf;

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
