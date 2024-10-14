use std::{net::SocketAddr, path::PathBuf, pin::Pin, sync::Arc, task::Poll, time::Duration};

use futures::{future::BoxFuture, Future, FutureExt, Stream};
use igd_next::PortMappingProtocol;
use tokio::sync::{broadcast, Mutex};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tonic::transport::{Certificate, Server, ServerTlsConfig};
use tonic::transport::Identity;
use zeroize::Zeroize;

use super::docker::container::{ManagedContainer, ManagedContainerRunning, ManagedContainerState};
use super::upnp::{Upnp, UpnpLease};
use super::Deimos;

use deimosproto as proto;

/// A connection to a remote client, with references to state required to serve RPC requests
pub struct ApiState {
    pub config: ApiConfig,
    /// Address leased for the API
    pub lease: Option<UpnpLease>,
    /// Status notification sender for gRPC subscribers
    pub sender: tokio::sync::broadcast::Sender<deimosproto::ContainerStatusNotification>,
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
    channel: Arc<Mutex<broadcast::Receiver<proto::ContainerStatusNotification>>>,
    state: Pin<Box<dyn Future<Output = Option<proto::ContainerStatusNotification>> + 'static + Send>>,
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
            Ok(mut server) => {
                let result = server
                    .add_service(proto::DeimosServiceServer::from_arc(self.clone()))
                    .serve_with_shutdown(self.api.config.bind, cancel.cancelled());

                tokio::select! {
                    _ = cancel.cancelled() => {},
                    result = result => if let Err(e) = result {
                        tracing::error!("gRPC server error: {e}");
                    }
                }
            },
            Err(e) => return Err(
                ApiInitError::TlsConfig(e)
            )
        }
        
        Ok(())
    }

    pub async fn container_api_run_status(c: &ManagedContainer) -> deimosproto::ContainerUpState {
        c.state.lock().await.as_ref().map(|s| proto::ContainerUpState::from(s.running)).unwrap_or(proto::ContainerUpState::Dead)
    }
}

impl From<ManagedContainerRunning> for proto::ContainerUpState {
    fn from(value: ManagedContainerRunning) -> proto::ContainerUpState {
        match value {
            ManagedContainerRunning::Dead => proto::ContainerUpState::Dead,
            ManagedContainerRunning::Paused => proto::ContainerUpState::Paused,
            ManagedContainerRunning::Running => proto::ContainerUpState::Running,
        }
    }
}


#[async_trait]
impl proto::DeimosService for Deimos {
    async fn query_containers(self: Arc<Self>, _request: tonic::Request<proto::QueryContainersRequest>) -> Result<tonic::Response<proto::QueryContainersResponse>, tonic::Status> {
        let mut containers = Vec::new();
        for c in self.docker.containers.iter() {
            let c = c.value();
            containers.push(
                proto::ContainerBrief {
                    id: c.config.id.to_string(),
                    title: c.config.name.to_string(),
                    up_state: Self::container_api_run_status(c).await as i32,
                }
            )
        }

        Ok(
            tonic::Response::new(proto::QueryContainersResponse { containers, })
        )
    }

    async fn get_container_image(self: Arc<Self>, _req: tonic::Request<proto::ContainerImagesRequest>) -> Result<tonic::Response<proto::ContainerImagesResponse>, tonic::Status> {
        Ok(
            tonic::Response::new(
                proto::ContainerImagesResponse {
                    banner: None,
                    icon: None
                }
            )
        )
    }

    type ContainerStatusStreamStream = ContainerStatusStreamer;

    async fn container_status_stream(self: Arc<Self>, _: tonic::Request<proto::ContainerStatusStreamRequest>) -> Result<tonic::Response<ContainerStatusStreamer>, tonic::Status> {
        let rx = self.api.sender.subscribe();

        Ok(
            tonic::Response::new(
                ContainerStatusStreamer::new(rx)
            )
        )
    }

    async fn update_container(self: Arc<Self>, req: tonic::Request<proto::UpdateContainerRequest>) -> Result<tonic::Response<proto::UpdateContainerResponse>, tonic::Status> {
        let req = req.into_inner();
        let Ok(method) = proto::ContainerUpState::try_from(req.method) else { return Err(tonic::Status::invalid_argument("Request method")) };
        match self.docker.containers.get(&req.id).map(|v| v.value().clone()) {
            Some(c) => match method {
                m if m == Self::container_api_run_status(&c).await => {
                    Ok(tonic::Response::new(proto::UpdateContainerResponse {}))
                },
                proto::ContainerUpState::Running => {
                    if let Err(e) = self.start(c.clone()).await {
                        tracing::error!("Failed to start container '{}' in response to gRPC request: {}", c.container_name(), e);
                        return Err(tonic::Status::internal("failed"))
                    }
                    Ok(tonic::Response::new(proto::UpdateContainerResponse {}))
                },
                proto::ContainerUpState::Dead => {
                    if let Err(e) = self.destroy(c.clone()).await {
                        tracing::error!("Failed to destroy container '{}' in response to gRPC request: {}", c.container_name(), e);
                        return Err(tonic::Status::internal("failed"))
                    }
                    Ok(tonic::Response::new(proto::UpdateContainerResponse {} ))
                },
                _ => Ok(tonic::Response::new(proto::UpdateContainerResponse {})),
            },
            None => Err(
                tonic::Status::not_found(format!("No such container: {}", req.id))
            )
        }
    }
}

impl Stream for ContainerStatusStreamer {
    type Item = Result<proto::ContainerStatusNotification, tonic::Status>;

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
    pub fn new(channel: broadcast::Receiver<proto::ContainerStatusNotification>) -> Self {
        let channel = Arc::new(Mutex::new(channel));
        let state = Self::future(channel.clone());

        Self {
            channel,
            state,
        }
    }

    fn future(channel: Arc<Mutex<broadcast::Receiver<proto::ContainerStatusNotification>>>) -> BoxFuture<'static, Option<proto::ContainerStatusNotification>> {
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
