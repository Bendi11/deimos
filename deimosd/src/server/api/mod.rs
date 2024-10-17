use std::{net::SocketAddr, path::PathBuf, pin::Pin, sync::Arc, task::Poll, time::Duration};

use futures::StreamExt;
use futures::{future::BoxFuture, Future, FutureExt, Stream};
use igd_next::PortMappingProtocol;
use tokio::sync::{broadcast, Mutex};
use async_trait::async_trait;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::{Certificate, Server, ServerTlsConfig};
use tonic::transport::Identity;
use zeroize::Zeroize;

use super::docker::container::{ManagedContainer, ManagedContainerRunning, ManagedContainerShared};
use super::docker::state::StatusStream;
use super::upnp::{Upnp, UpnpLease};
use super::Deimos;

use deimosproto::{self as proto, ContainerStatusNotification};

/// A connection to a remote client, with references to state required to serve RPC requests
pub struct ApiState {
    pub config: ApiConfig,
    /// Address leased for the API
    pub lease: Option<UpnpLease>,
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

        Ok(
            Self {
                config,
                lease,
            }
        )
    }
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

    fn container_api_run_status(state: &Option<ManagedContainerShared>) -> deimosproto::ContainerUpState {
        state.as_ref().map(|s| proto::ContainerUpState::from(s.running)).unwrap_or(proto::ContainerUpState::Dead)
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
        for (_, c) in self.docker.containers.iter() {
            containers.push(
                proto::ContainerBrief {
                    id: c.config.id.to_string(),
                    title: c.config.name.to_string(),
                    up_state: Self::container_api_run_status(&c.state()) as i32,
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

    type ContainerStatusStreamStream = futures::stream::Map<
        StatusStream,
        Box<dyn
            FnMut(Arc<ManagedContainer>) -> Result<proto::ContainerStatusNotification, tonic::Status>
            + Send + Sync
        >
    >;

    async fn container_status_stream(self: Arc<Self>, _: tonic::Request<proto::ContainerStatusStreamRequest>) -> Result<tonic::Response<Self::ContainerStatusStreamStream>, tonic::Status> {
        Ok(
            tonic::Response::new(
                self.docker.subscribe_state_stream()
                    .map(
                        Box::new(
                            |container: Arc<ManagedContainer>|Ok(
                                proto::ContainerStatusNotification {
                                    container_id: container.managed_id().to_owned(),
                                    up_state: Self::container_api_run_status(&container.state()) as i32
                                }
                            )
                        )
                    )
            )
        )
    }

    async fn update_container(self: Arc<Self>, req: tonic::Request<proto::UpdateContainerRequest>) -> Result<tonic::Response<proto::UpdateContainerResponse>, tonic::Status> {
        let req = req.into_inner();
        let Ok(method) = proto::ContainerUpState::try_from(req.method) else { return Err(tonic::Status::invalid_argument("Request method")) };
        match self.docker.containers.get(&req.id).cloned() {
            Some(c) => {
                match method {
                    proto::ContainerUpState::Running => {
                        tokio::task::spawn(
                            async move {
                                let mut ts = c.transaction().await;
                                if let Err(e) = self.start(&mut ts).await {
                                    tracing::error!("Failed to start server in response to gRPC request: {e}");
                                }
                            }
                        );
                    }
                    proto::ContainerUpState::Dead => {
                        tokio::task::spawn(
                            async move {
                                let mut ts = c.transaction().await;
                                if let Err(e) = self.destroy(&mut ts).await {
                                    tracing::error!("Failed to destroy server in response to gRPC request: {e}");
                                }
                            }
                        );
                    },
                    _ => ()
                };

                Ok(
                    tonic::Response::new(
                        proto::UpdateContainerResponse {}
                    )
                )
            },
            None => Err(
                tonic::Status::not_found(format!("No such container: {}", req.id))
            )
        }
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
