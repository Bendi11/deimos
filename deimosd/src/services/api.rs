use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use deimos_shared::{Deimos, DeimosServer, QueryContainersRequest, QueryContainersResponse};
use tokio_util::sync::CancellationToken;
use async_trait::async_trait;
use tonic::transport::Server;

use super::docker::DockerService;


/// A connection to a remote client, with references to state required to serve RPC requests
pub struct ApiService {
    config: ApiConfig,
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
        Ok(
            Self {
                config,
                docker,
            }
        )
    }

    pub async fn run(self: Arc<Self>, cancel: CancellationToken) {
        if let Err(e) = Server::builder()
            .add_service(
                DeimosServer::from_arc(self.clone())
            )
            .serve_with_shutdown(self.config.bind, cancel.cancelled())
            .await {
            tracing::error!("Failed to run API server: {e}");
        }
    }
}

#[async_trait]
impl Deimos for ApiService {
    async fn query_containers(self: Arc<Self>, _request: tonic::Request<QueryContainersRequest>) -> Result<tonic::Response<QueryContainersResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Not yet finished"))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiInitError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
}
