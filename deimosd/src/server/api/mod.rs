use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use bollard::secret::ContainerStatus;
use deimos_shared::{ContainerStatusNotification, ContainerStatusRequest, ContainerStatusResponse, ContainerStatusStreamRequest, DeimosService, DeimosServiceServer, QueryContainersRequest, QueryContainersResponse};
use tokio_util::sync::CancellationToken;
use async_trait::async_trait;
use tonic::transport::Server;

use super::Deimos;



/// A connection to a remote client, with references to state required to serve RPC requests
pub struct ApiState {
    config: ApiConfig,
}

pub struct ApiServer {
    deimos: Arc<Deimos>,
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

impl ApiState {
    /// Load the Deimos API service configuration and store a handle to the local Docker instance
    /// to manage containers
    pub async fn new(config: ApiConfig) -> Result<Self, ApiInitError> {
        Ok(
            Self {
                config,
            }
        )
    }
}

pub struct ContainerStatusStreamer(Arc<Deimos>);

#[async_trait]
impl DeimosService for Deimos {
    async fn query_containers(self: Arc<Self>, _request: tonic::Request<QueryContainersRequest>) -> Result<tonic::Response<QueryContainersResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Not yet finished"))
    }

    async fn container_status(self: Arc<Self>, _: tonic::Request<ContainerStatusRequest>) -> Result<tonic::Response<ContainerStatusResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("unimplemented"))
    }

    type ContainerStatusStreamStream = ContainerStatusStreamer;

    async fn container_status_stream(self: Arc<Self>, _: tonic::Request<ContainerStatusStreamRequest>) -> Result<tonic::Response<ContainerStatusStreamer>, tonic::Status> {
        Err(tonic::Status::unimplemented("unimplemented"))
    }
}

impl futures::Stream for ContainerStatusStreamer {
    type Item = Result<ContainerStatusNotification, tonic::Status>;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Pending
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiInitError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
}
