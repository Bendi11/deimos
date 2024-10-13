use std::{net::SocketAddr, path::PathBuf, pin::Pin, sync::Arc, task::Poll, time::Duration};

use deimos_shared::{util, ContainerBrief, ContainerDockerRunStatus, ContainerDockerStatus, ContainerImagesRequest, ContainerImagesResponse, ContainerStatusNotification, ContainerStatusRequest, ContainerStatusResponse, ContainerStatusStreamRequest, DeimosService, DeimosServiceServer, QueryContainersRequest, QueryContainersResponse, UpdateContainerMethod, UpdateContainerRequest, UpdateContainerResponse};
use futures::{future::BoxFuture, Future, FutureExt, Stream};
use igd_next::PortMappingProtocol;
use tokio::sync::{broadcast, Mutex};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tonic::transport::{Certificate, Server, ServerTlsConfig};
use tonic::transport::Identity;
use zeroize::Zeroize;

use super::docker::container::{ManagedContainerRunning, ManagedContainerState};
use super::upnp::{Upnp, UpnpLease};
use super::Deimos;

/// A connection to a remote client, with references to state required to serve RPC requests
pub struct ApiState {
    pub config: ApiConfig,
    /// Address leased for the API
    pub lease: Option<UpnpLease>,
    /// Status notification sender for gRPC subscribers
    pub sender: tokio::sync::broadcast::Sender<ContainerStatusNotification>,
}

/// Configuration used to initialize and inform the Deimos API service
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    pub bind: SocketAddr,
    #[serde(default)]
    pub upnp: bool,
    pub client_ca_root: PathBuf,
    pub certificate: PathBuf,
    pub privkey: PathBuf,
    #[serde(default = "ApiConfig::default_timeout")]
    pub timeout: Duration,
}

impl ApiState {
    /// Load the Deimos API service configuration and store a handle to the local Docker instance
    /// to manage containers
    pub async fn new(upnp: &Upnp, config: ApiConfig) -> Result<Self, ApiInitError> {
        let lease = match config.upnp {
            true => Some(
                    upnp.lease(
                    std::iter::once((config.bind.port(), PortMappingProtocol::TCP))
                ).await
            ),
            false => None,
        };


        let (sender, _) = tokio::sync::broadcast::channel(2);

        Ok(
            Self {
                config,
                lease,
                sender,
            }
        )
    }
}

pub struct ContainerStatusStreamer {
    channel: Arc<Mutex<broadcast::Receiver<ContainerStatusNotification>>>,
    state: Pin<Box<dyn Future<Output = Option<ContainerStatusNotification>> + 'static + Send>>,
}

impl Deimos {
    pub async fn serve_api(self: Arc<Self>, cancel: CancellationToken) -> Result<(), ApiInitError> {
        /*let mut ca_cert_pem = util::load_check_permissions(&self.api.config.certificate)
            .await
            .map_err(|e| ApiInitError::LoadSensitiveFile(self.api.config.certificate.clone(), e))?;

        let mut privkey_pem = util::load_check_permissions(&self.api.config.privkey)
            .await
            .map_err(|e| ApiInitError::LoadSensitiveFile(self.api.config.privkey.clone(), e))?;

        let mut client_ca_root_pem = util::load_check_permissions(&self.api.config.client_ca_root)
            .await
            .map_err(|e| ApiInitError::LoadSensitiveFile(self.api.config.client_ca_root.clone(), e))?;*/


        let server = Server::builder()
            .timeout(self.api.config.timeout)
            /*.tls_config(
                ServerTlsConfig::new()
                    .client_auth_optional(true)
                    .identity(Identity::from_pem(&ca_cert_pem, &privkey_pem))
                    .client_ca_root(Certificate::from_pem(&client_ca_root_pem))
            )*/;

        /*ca_cert_pem.zeroize();
        privkey_pem.zeroize();
        client_ca_root_pem.zeroize();*/

        match Ok(server) {
            Ok(mut server) => if let Err(e) = server
                .add_service(DeimosServiceServer::from_arc(self.clone()))
                .serve_with_shutdown(self.api.config.bind, cancel.cancelled())
                .await {
                tracing::error!("Failed to run gRPC server: {e}");
            },
            Err(e) => return Err(
                ApiInitError::TlsConfig(e)
            )
        }
        
        Ok(())
    }

    fn state_to_api(state: &ManagedContainerState) -> ContainerDockerStatus {
        ContainerDockerStatus {
            run_status: ContainerDockerRunStatus::from(state.running).into(),
        }
    }
}

impl From<ManagedContainerRunning> for ContainerDockerRunStatus {
    fn from(value: ManagedContainerRunning) -> ContainerDockerRunStatus {
        match value {
            ManagedContainerRunning::Dead => ContainerDockerRunStatus::Dead,
            ManagedContainerRunning::Paused => ContainerDockerRunStatus::Paused,
            ManagedContainerRunning::Running => ContainerDockerRunStatus::Running,
        }
    }
}


#[async_trait]
impl DeimosService for Deimos {
    async fn query_containers(self: Arc<Self>, _request: tonic::Request<QueryContainersRequest>) -> Result<tonic::Response<QueryContainersResponse>, tonic::Status> {
        let mut containers = Vec::new();
        for c in self.docker.containers.iter() {
            let c = c.value();
            containers.push(
                ContainerBrief {
                    id: c.config.id.to_string(),
                    title: c.config.name.to_string(),
                    updated: c.last_modified.timestamp(),
                    running: c.state.lock().await.as_ref().map(Self::state_to_api),
                }
            )
        }

        Ok(
            tonic::Response::new(QueryContainersResponse { containers, })
        )
    }

    async fn get_container_image(self: Arc<Self>, _req: tonic::Request<ContainerImagesRequest>) -> Result<tonic::Response<ContainerImagesResponse>, tonic::Status> {
        Ok(
            tonic::Response::new(
                ContainerImagesResponse {
                    banner: None,
                    icon: None
                }
            )
        )
    }

    async fn container_status(self: Arc<Self>, req: tonic::Request<ContainerStatusRequest>) -> Result<tonic::Response<ContainerStatusResponse>, tonic::Status> {
        let req = req.into_inner();
        match self.docker.containers.get(&req.container_id) {
            Some(container) => {
                let state = container.state.lock().await;
                Ok(
                    tonic::Response::new(
                        ContainerStatusResponse {
                            status: state.as_ref().map(Self::state_to_api),
                        }
                    )
                )
            },
            None => Err(
                tonic::Status::not_found(format!("No such container '{}'", req.container_id))
            )
        }
    }

    type ContainerStatusStreamStream = ContainerStatusStreamer;

    async fn container_status_stream(self: Arc<Self>, _: tonic::Request<ContainerStatusStreamRequest>) -> Result<tonic::Response<ContainerStatusStreamer>, tonic::Status> {
        let rx = self.api.sender.subscribe();

        Ok(
            tonic::Response::new(
                ContainerStatusStreamer::new(rx)
            )
        )
    }

    async fn update_container(self: Arc<Self>, req: tonic::Request<UpdateContainerRequest>) -> Result<tonic::Response<UpdateContainerResponse>, tonic::Status> {
        let req = req.into_inner();
        let Ok(method) = UpdateContainerMethod::try_from(req.method) else { return Err(tonic::Status::invalid_argument("Request method")) };
        match self.docker.containers.get(&req.id).map(|v| v.value().clone()) {
            Some(c) => match method {
                UpdateContainerMethod::Start => {
                    if let Err(e) = self.start(c.clone()).await {
                        tracing::error!("Failed to start container '{}' in response to gRPC request: {}", c.container_name(), e);
                        return Err(tonic::Status::internal("failed"))
                    }
                    Ok(tonic::Response::new(UpdateContainerResponse {}))
                },
                UpdateContainerMethod::Stop => {
                    if let Err(e) = self.destroy(c.clone()).await {
                        tracing::error!("Failed to destroy container '{}' in response to gRPC request: {}", c.container_name(), e);
                        return Err(tonic::Status::internal("failed"))
                    }
                    Ok(tonic::Response::new(UpdateContainerResponse {} ))
                }
            },
            None => Err(
                tonic::Status::not_found(format!("No such container: {}", req.id))
            )
        }
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
    #[error("Failed to load sensitive file {}: {}", .0.display(), .1)]
    LoadSensitiveFile(PathBuf, std::io::Error),
    #[error("Failed to set server TLS configuration: {}", .0)]
    TlsConfig(tonic::transport::Error),
}

impl ApiConfig {
    pub fn default_timeout() -> Duration {
        Duration::from_secs(120)
    }
}
