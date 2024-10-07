use std::{net::SocketAddr, path::PathBuf, pin::Pin, sync::Arc, task::Poll, time::Duration};

use deimos_shared::{util, ContainerBrief, ContainerImagesRequest, ContainerImagesResponse, ContainerStatusNotification, ContainerStatusRequest, ContainerStatusResponse, ContainerStatusStreamRequest, DeimosService, DeimosServiceServer, QueryContainersRequest, QueryContainersResponse};
use futures::{future::BoxFuture, Future, FutureExt, Stream};
use igd_next::PortMappingProtocol;
use tokio::sync::{broadcast, Mutex};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tonic::transport::{Certificate, Server, ServerTlsConfig};
use tonic::transport::Identity;
use zeroize::Zeroize;

use super::upnp::{Upnp, UpnpLease};
use super::Deimos;

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

pub struct ContainerStatusStreamer {
    channel: Arc<Mutex<broadcast::Receiver<ContainerStatusNotification>>>,
    state: Pin<Box<dyn Future<Output = Option<ContainerStatusNotification>> + 'static + Send>>,
}

impl Deimos {
    pub async fn serve_api(self: Arc<Self>, cancel: CancellationToken) -> Result<(), ApiInitError> {
        tracing::error!("Beginning gRPC server");

        let mut ca_cert_pem = util::load_check_permissions(&self.api.config.certificate)
            .await
            .map_err(|e| ApiInitError::LoadSensitiveFile(self.api.config.certificate.clone(), e))?;

        let mut privkey_pem = util::load_check_permissions(&self.api.config.privkey)
            .await
            .map_err(|e| ApiInitError::LoadSensitiveFile(self.api.config.privkey.clone(), e))?;

        let mut client_ca_root_pem = util::load_check_permissions(&self.api.config.client_ca_root)
            .await
            .map_err(|e| ApiInitError::LoadSensitiveFile(self.api.config.client_ca_root.clone(), e))?;


        let server = Server::builder()
            .timeout(self.api.config.timeout)
            .tls_config(
                ServerTlsConfig::new()
                    .client_auth_optional(true)
                    .identity(Identity::from_pem(&ca_cert_pem, &privkey_pem))
                    .client_ca_root(Certificate::from_pem(&client_ca_root_pem))
            );

        ca_cert_pem.zeroize();
        privkey_pem.zeroize();
        client_ca_root_pem.zeroize();

        match server {
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
}


#[async_trait]
impl DeimosService for Deimos {
    async fn query_containers(self: Arc<Self>, _request: tonic::Request<QueryContainersRequest>) -> Result<tonic::Response<QueryContainersResponse>, tonic::Status> {
        let containers = self
            .docker
            .containers
            .iter()
            .map(|entry| ContainerBrief {
                id: entry.value().config.id.to_string(),
                title: entry.value().config.name.to_string(),
                updated: entry.value().last_modified.timestamp()
            })
            .collect::<Vec<_>>();

        Ok(
            tonic::Response::new(QueryContainersResponse { containers, })
        )
    }

    async fn get_container_image(self: Arc<Self>, _req: tonic::Request<ContainerImagesRequest>) -> Result<tonic::Response<ContainerImagesResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("unimplemented"))
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
