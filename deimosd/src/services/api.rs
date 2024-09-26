use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use super::docker::DockerService;

/// A connection to a remote client, with references to state required to serve RPC requests
pub struct ApiService {
    docker: Arc<DockerService>,
}

/// Configuration used to initialize and inform the Deimos API service
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    pub bind: SocketAddr,
    #[serde(default)]
    pub upnp: bool,
    pub keyfile: PathBuf,
}

impl ApiService {
    /// Load the Deimos API service configuration and store a handle to the local Docker instance
    /// to manage containers
    pub async fn new(config: ApiConfig, docker: Arc<DockerService>) -> Result<Self, ApiInitError> {
        Ok(Self { docker })
    }

    pub async fn run(self: Arc<Self>, cancel: CancellationToken) {}
}

#[derive(Debug, thiserror::Error)]
pub enum ApiInitError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
}
