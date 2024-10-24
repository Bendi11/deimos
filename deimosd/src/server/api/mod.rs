use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use igd_next::PortMappingProtocol;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

use crate::pod::docker::logs::PodLogStream;
use crate::pod::id::DeimosId;
use crate::pod::{Pod, PodState, PodStateStream};

use super::upnp::{Upnp, UpnpLease, UpnpLeaseData};
use super::Deimos;

use deimosproto::{self as proto};

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
            true => Some(
                upnp
                    .request(vec![
                        UpnpLeaseData {
                            port: config.bind.port(),
                            protocol: PortMappingProtocol::TCP,
                            name: "Deimos gRPC server".to_owned(),
                        }
                    ])
                    .await?
            ),
            false => None,
        };

        Ok(Self { config, lease })
    }
    
    /// Apply settings given in the API configuration to create a new gRPC server
    async fn init_server(config: &ApiConfig) -> Result<Server, ApiInitError> {
        let server = Server::builder().timeout(config.timeout);
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
                return;
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

    /// Get a pod by the ID as received from a client, and map not found to a [tonic::Status]
    /// indicating the error
    fn lookup_pod(&self, id: String) -> Result<Arc<Pod>, tonic::Status> {
        self.pods
            .get(&id)
            .ok_or_else(|| tonic::Status::not_found(id))
    }
}

impl From<PodState> for proto::PodState {
    fn from(value: PodState) -> Self {
        match value {
            PodState::Disabled => proto::PodState::Disabled,
            PodState::Transit => proto::PodState::Transit,
            PodState::Paused => proto::PodState::Paused,
            PodState::Enabled => proto::PodState::Enabled,
        }
    }
}

#[async_trait]
impl proto::DeimosService for Deimos {
    async fn query_pods(
        self: Arc<Self>,
        _: tonic::Request<proto::QueryPodsRequest>,
    ) -> Result<tonic::Response<proto::QueryPodsResponse>, tonic::Status> {
        let pods = self
            .pods
            .iter()
            .map(|(_, pod)| proto::PodBrief {
                id: pod.id().owned(),
                title: pod.title().to_owned(),
                state: proto::PodState::from(pod.state()) as i32,
            })
            .collect::<Vec<_>>();

        Ok(tonic::Response::new(proto::QueryPodsResponse { pods }))
    }

    async fn update_pod(
        self: Arc<Self>,
        req: tonic::Request<proto::UpdatePodRequest>,
    ) -> Result<tonic::Response<proto::UpdatePodResponse>, tonic::Status> {
        let req = req.into_inner();
        let pod = self.lookup_pod(req.id)?;
        let id = pod.id();

        match proto::PodState::try_from(req.method) {
            Ok(proto::PodState::Disabled) => tokio::task::spawn(async move {
                if let Err(e) = self.pods.disable(pod).await {
                    tracing::error!(
                        "Failed ot disable pod {} in response to API request: {}",
                        id,
                        e
                    );
                }
            }),
            Ok(proto::PodState::Enabled) => tokio::task::spawn(async move {
                if let Err(e) = self.pods.enable(pod).await {
                    tracing::error!(
                        "Failed to enable pod {} in response to API request: {}",
                        id,
                        e
                    );
                }
            }),
            Ok(proto::PodState::Paused) => tokio::task::spawn(async move {
                if let Err(e) = self.pods.pause(pod).await {
                    tracing::error!(
                        "Failed to puase pod {} in response to API request: {}",
                        id,
                        e
                    );
                }
            }),
            Ok(proto::PodState::Transit) => {
                return Err(tonic::Status::invalid_argument(String::from(
                    "Cannot set pod to reserved state Transit",
                )))
            }
            Err(_) => {
                return Err(tonic::Status::invalid_argument(format!(
                    "Unknown pod state enumeration value {}",
                    req.method
                )))
            }
        };

        Ok(tonic::Response::new(proto::UpdatePodResponse {}))
    }

    type SubscribePodStatusStream = futures::stream::Map<
        PodStateStream,
        Box<PodStatusApiMapper>,
    >;

    async fn subscribe_pod_status(
        self: Arc<Self>,
        _: tonic::Request<proto::PodStatusStreamRequest>,
    ) -> Result<tonic::Response<Self::SubscribePodStatusStream>, tonic::Status> {
        let stream = self.pods.stream().map(Box::<PodStatusApiMapper>::from(Box::new(move |(id, state)| {
            Ok(proto::PodStatusNotification {
                id: id.owned(),
                state: proto::PodState::from(state) as i32,
            })
        })));

        Ok(tonic::Response::new(stream))
    }

    type SubscribePodLogsStream = futures::stream::Map<
        PodLogStream,
        Box<PodLogApiMapper>
    >;

    async fn subscribe_pod_logs(self: Arc<Self>, req: tonic::Request<proto::PodLogStreamRequest>) -> Result<tonic::Response<Self::SubscribePodLogsStream>, tonic::Status> {
        let pod = self.lookup_pod(req.into_inner().id)?;
        tracing::trace!("Client subscribed to logs for {}", pod.id());

        self
            .pods
            .subscribe_logs(pod)
            .await
            .map_err(|e| tonic::Status::failed_precondition(e.to_string()))
            .map(|sub|
                tonic::Response::new(
                    sub
                        .map(
                            Box::<PodLogApiMapper>::from(
                                Box::new(|bytes: Bytes|
                                    Ok(
                                        proto::PodLogChunk {
                                            chunk: bytes.to_vec()
                                        }
                                    )
                                )
                            )
                        )
                )
            )
    }
}

type PodStatusApiMapper = dyn FnMut((DeimosId, PodState)) -> Result<proto::PodStatusNotification, tonic::Status> + Send + Sync;
type PodLogApiMapper = dyn FnMut(Bytes) -> Result<proto::PodLogChunk, tonic::Status> + Send + Sync;

#[derive(Debug, thiserror::Error)]
pub enum ApiInitError {
    #[error("Failed to get UPnP lease for gRPC server: {0}")]
    UpnpLease(#[from] crate::server::upnp::UpnpError),
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
