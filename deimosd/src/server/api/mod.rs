use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::StreamExt;
use igd_next::PortMappingProtocol;
use tokio_util::sync::CancellationToken;
use tonic::transport::Identity;
use tonic::transport::{Certificate, Server, ServerTlsConfig};
use zeroize::Zeroize;

use super::docker::container::{ManagedContainer, ManagedContainerRunning, ManagedContainerShared};
use super::docker::state::StatusStream;
use super::upnp::{Upnp, UpnpLease, UpnpLeaseData};
use super::Deimos;

use deimosproto as proto;

/// State required exclusively for the gRPC server including UPnP port leases.
pub struct ApiState {
    pub config: ApiConfig,
    /// Address leased for the API
    pub lease: Option<UpnpLease>,
}

/// Configuration used to initialize the Deimos gRPC API server.
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
            true => match upnp.request(
                std::iter::once(UpnpLeaseData {
                    port: config.bind.port(),
                    protocol: PortMappingProtocol::TCP,
                    name: "Deimos gRPC server".to_owned()
                    })
                )
                .await {
                Ok(lease) => Some(lease),
                Err(e) => {
                    tracing::error!("Failed to get UPnP lease for gRPC server: {e}");
                    None
                }
            },
            false => None,
        };

        Ok(Self { config, lease })
    }

    async fn init_server(config: &ApiConfig) -> Result<Server, ApiInitError> {
        let server = Server::builder()
            .timeout(config.timeout);
        Ok(server)
    }
}

impl Deimos {


    /// Load all specified certificates from the paths specified in the config and attempt to run
    /// the server to completion.
    /// This method should not return until the [CancellationToken] has been cancelled.
    pub async fn api_task(self: Arc<Self>, cancel: CancellationToken) {
        let mut server = match ApiState::init_server(&self.api.config).await {
            Ok(server) => server,
            Err(e) => {
                tracing::error!("Failed to initialize gRPC server: {e}");
                return
            }
        };

        let result = server
            .add_service(proto::DeimosServiceServer::from_arc(self.clone()))
            .serve_with_shutdown(self.api.config.bind, cancel.cancelled());

        tokio::select! {
            _ = cancel.cancelled() => {},
            result = result => if let Err(e) = result {
                tracing::error!("gRPC server error: {e}");
            }
        }
    }

    /// Translate the current state of a managed container to an enum that can be serialized to
    /// protobuf and sent to clients
    fn container_api_run_status(
        state: &Option<ManagedContainerShared>,
    ) -> deimosproto::ContainerUpState {
        state
            .as_ref()
            .map(|s| proto::ContainerUpState::from(s.running))
            .unwrap_or(proto::ContainerUpState::Dead)
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
    async fn query_containers(
        self: Arc<Self>,
        _request: tonic::Request<proto::QueryContainersRequest>,
    ) -> Result<tonic::Response<proto::QueryContainersResponse>, tonic::Status> {
        let mut containers = Vec::new();
        for (_, c) in self.docker.containers.iter() {
            containers.push(proto::ContainerBrief {
                id: c.config.id.to_string(),
                title: c.config.name.to_string(),
                up_state: Self::container_api_run_status(&c.state()) as i32,
            })
        }

        Ok(tonic::Response::new(proto::QueryContainersResponse {
            containers,
        }))
    }

    type SubscribeContainerStatusStream = futures::stream::Map<
        StatusStream,
        Box<
            dyn FnMut(
                    Arc<ManagedContainer>,
                ) -> Result<proto::ContainerStatusNotification, tonic::Status>
                + Send
                + Sync,
        >,
    >;

    async fn subscribe_container_status(
        self: Arc<Self>,
        _: tonic::Request<proto::ContainerStatusStreamRequest>,
    ) -> Result<tonic::Response<Self::SubscribeContainerStatusStream>, tonic::Status> {
        Ok(tonic::Response::new(
            self.docker.subscribe_state_stream().map(Box::new(
                |container: Arc<ManagedContainer>| {
                    Ok(proto::ContainerStatusNotification {
                        container_id: container.deimos_id().owned(),
                        up_state: Self::container_api_run_status(&container.state()) as i32,
                    })
                },
            )),
        ))
    }

    async fn update_container(
        self: Arc<Self>,
        req: tonic::Request<proto::UpdateContainerRequest>,
    ) -> Result<tonic::Response<proto::UpdateContainerResponse>, tonic::Status> {
        let req = req.into_inner();
        let Ok(method) = proto::ContainerUpState::try_from(req.method) else {
            return Err(tonic::Status::invalid_argument("Request method"));
        };
        match self.docker.containers.get(req.id.as_str()).cloned() {
            Some(c) => {
                match method {
                    proto::ContainerUpState::Running => {
                        tokio::task::spawn(async move {
                            let mut ts = c.transaction().await;
                            if let Err(e) = self.start(&mut ts).await {
                                tracing::error!(
                                    "Failed to start server in response to gRPC request: {e}"
                                );
                            }
                        });
                    }
                    proto::ContainerUpState::Dead => {
                        tokio::task::spawn(async move {
                            let mut ts = c.transaction().await;
                            if let Err(e) = self.destroy(&mut ts).await {
                                tracing::error!(
                                    "Failed to destroy server in response to gRPC request: {e}"
                                );
                            }
                        });
                    }
                    _ => (),
                };

                Ok(tonic::Response::new(proto::UpdateContainerResponse {}))
            }
            None => Err(tonic::Status::not_found(format!(
                "No such container: {}",
                req.id
            ))),
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
    /// Get the default gRPC timeout, used to provide a value for `serde`'s automatic Deserialize
    /// implementation
    pub const fn default_timeout() -> Duration {
        Duration::from_secs(120)
    }
}
