use std::{net::SocketAddr, path::PathBuf, pin::Pin, sync::Arc, task::Poll};

use deimos_shared::{ContainerStatusNotification, ContainerStatusRequest, ContainerStatusResponse, ContainerStatusStreamRequest, DeimosService, DeimosServiceServer, QueryContainersRequest, QueryContainersResponse};
use futures::{future::BoxFuture, Future, FutureExt, Stream};
use tokio::sync::{broadcast, Mutex};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

use super::Deimos;



/// A connection to a remote client, with references to state required to serve RPC requests
pub struct ApiState {
    config: ApiConfig,
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

pub struct ContainerStatusStreamer {
    channel: Arc<Mutex<broadcast::Receiver<ContainerStatusNotification>>>,
    state: Pin<Box<dyn Future<Output = Option<ContainerStatusNotification>> + 'static + Send>>,
}

impl Deimos {
    pub async fn serve_api(self: Arc<Self>, cancel: CancellationToken) {
        if let Err(e) = Server::builder()
            .add_service(DeimosServiceServer::from_arc(self.clone()))
            .serve_with_shutdown(self.api.config.bind, cancel.cancelled())
            .await {
            tracing::error!("Failed to run Deimos API server: {e}")
        }
    }
}


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
        let rx = self.status.subscribe();

        Ok(
            tonic::Response::new(
                ContainerStatusStreamer::new(rx)
            )
        )
    }
}

impl Stream for ContainerStatusStreamer {
    type Item = Result<ContainerStatusNotification, tonic::Status>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        match self.state.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(val) => {
                self.state = Self::future(self.channel.clone());
                Poll::Ready(Some(val.ok_or_else(|| tonic::Status::internal("Failed to receive container notification"))))
            }
        }
    }
}

impl ContainerStatusStreamer {
    pub fn new(channel: broadcast::Receiver<ContainerStatusNotification>) -> Self {
        let channel = Arc::new(Mutex::new(channel));
        let state = Self::future(channel.clone());

        Self {
            channel,
            state,
        }
    }

    fn future(channel: Arc<Mutex<broadcast::Receiver<ContainerStatusNotification>>>) -> BoxFuture<'static, Option<ContainerStatusNotification>> {
        Box::pin(async move {
            channel
                .lock()
                .await
                .recv()
                .map(Result::ok)
                .await
        })
    }
}



#[derive(Debug, thiserror::Error)]
pub enum ApiInitError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
}
