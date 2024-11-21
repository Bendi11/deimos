use std::future::Future;
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use auth::{ApiAuthorization, ApiAuthorizationConfig, ApiAuthorizationPersistent};
use igd_next::PortMappingProtocol;
use tokio_util::sync::CancellationToken;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Server, ServerTlsConfig};
use zeroize::Zeroizing;

use crate::pod::{Pod, PodState};

use super::upnp::{Upnp, UpnpLease, UpnpLeaseData};
use super::Deimos;

use deimosproto::{self as proto};

mod auth;
mod grpc;

/// State required exclusively for the gRPC server including UPnP port leases.
pub struct ApiState {
    /// Configuration parsed from the global config file
    pub config: ApiConfig,
    /// Authorization state with all approved and pending tokens
    pub auth: ApiAuthorization,
    /// Address leased for the API
    pub _lease: Option<UpnpLease>,
}

/// Configuration used to initialize the Deimos gRPC API server.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    /// Address to bind to when serving the public interface
    pub bind: SocketAddr,
    /// Path to create a UDS socket for the internal priviledged API
    pub internal_bind: PathBuf,
    /// Enable UPnP port forwarding for the public API
    #[serde(default)]
    pub upnp: bool,
    /// Path to a public certificate file for TLS
    pub certificate: PathBuf,
    /// Path to TLS private key
    pub privkey: PathBuf,
    /// Timeout for API connections
    #[serde(default = "ApiConfig::default_timeout")]
    pub timeout: Duration,
    /// Configuration for the authorization component
    #[serde(default)]
    pub auth: ApiAuthorizationConfig,
}

/// Persistent state for the API
#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct ApiPersistent {
    pub tokens: ApiAuthorizationPersistent,
}

impl ApiState {
    /// Load the Deimos API service configuration and store a handle to the local Docker instance
    /// to manage containers
    pub async fn load(persistent: ApiPersistent, config: ApiConfig, upnp: &Upnp) -> Result<Self, ApiInitError> {
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

        let auth = ApiAuthorization::load(persistent.tokens, config.auth.clone());

        Ok(Self { config, _lease: lease, auth })
    }
    
    /// Get persistent state to be written to a save file for the server
    pub fn save(&self) -> ApiPersistent {
        ApiPersistent {
            tokens: self.auth.persistent(),
        }
    }
}

impl Deimos {
    /// Load all specified certificates from the paths specified in the config and attempt to run
    /// the server to completion.
    /// This method should not return until the [CancellationToken] has been cancelled.
    pub async fn api_task(self: Arc<Self>, cancel: CancellationToken) {
        let public = match self.clone().run_public_server(&cancel).await {
            Ok(fut) => fut,
            Err(e) => {
                tracing::error!("Failed to create public gRPC server: {e}");
                return
            }
        };

        let internal = match self.run_internal_server(&cancel).await {
            Ok(internal) => internal,
            Err(e) => {
                tracing::error!("Failed to create internal gRPC server: {e}");
                return
            }
        };

        tokio::select! {
            _ = cancel.cancelled() => {},
            result = public => if let Err(e) = result {
                tracing::error!("gRPC server error: {e:?}");
            },
            result = internal => if let Err(e) = result {
                tracing::error!("gRPC private API error: {e:?}");
            }
        }
    }

    async fn run_public_server(self: Arc<Self>, cancel: &CancellationToken) -> Result<impl Future<Output = Result<(), tonic::transport::Error>> + use<'_>, ApiInitError> {
        let config = &self.api.config;

        let certificate = deimosproto::util::load_check_permissions(&config.certificate)
            .await
            .map(Zeroizing::new)
            .map_err(|err| ApiInitError::LoadSensitiveFile(config.privkey.clone(), err))?;
        let privkey = deimosproto::util::load_check_permissions(&config.privkey)
            .await
            .map(Zeroizing::new)
            .map_err(|err| ApiInitError::LoadSensitiveFile(config.certificate.clone(), err))?;

        let identity = tonic::transport::Identity::from_pem(certificate, privkey);

        let mut server = Server::builder()
            .timeout(config.timeout)
            .tls_config(
                ServerTlsConfig::new()
                    .identity(identity)
            )?;

        Ok(server
            .add_service(
                InterceptedService::new(
                    proto::server::DeimosServiceServer::from_arc(self.clone()),
                    self.api.auth.clone(),
                )
            )
            .add_service(proto::authserver::DeimosAuthorizationServer::from_arc(self.clone()))
            .serve_with_shutdown(self.api.config.bind, cancel.cancelled())
        )
    }

    /// Apply the settings in the given configuration to create a local socket that hosts the
    /// private API
    async fn run_internal_server(self: Arc<Self>, cancel: &CancellationToken) -> Result<impl Future<Output = Result<(), tonic::transport::Error>> + use<'_>, ApiInitError> {
        #[cfg(unix)]
        {
            use tokio::net::UnixListener;
            use tokio_stream::wrappers::UnixListenerStream;
            
            let bind = self.api.config.internal_bind.clone();
            if let Some(parent) = bind.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| ApiInitError::CreateDir { path: parent.to_owned(), err })?;
            }

            if let Ok(true) = std::fs::exists(&bind) {
                if let Err(e) = std::fs::remove_file(&bind) {
                    tracing::warn!("Failed to delete old socket {}: {}", bind.display(), e);
                }
            }

            let uds = UnixListener::bind(&bind)
                .map_err(|err| ApiInitError::CreateSocket { path: bind.clone(), err })?;
            let stream = UnixListenerStream::new(uds);

            tracing::info!("Created socket at {} for private API", bind.display());
            
            Ok(async move {
                Server::builder()
                    .add_service(deimosproto::internal_server::InternalServer::from_arc(self))
                    .serve_with_incoming_shutdown(stream, cancel.cancelled())
                    .await
            })
        }
        #[cfg(not(unix))]
        {
            Ok(async move {
                panic!("Cannot create internal API socket on non-unix");
            }) 
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

#[derive(Debug, thiserror::Error)]
pub enum ApiInitError {
    #[error("Failed to get UPnP lease for gRPC server: {0}")]
    UpnpLease(#[from] crate::server::upnp::UpnpError),
    #[error("Failed to load sensitive file {}: {}", .0.display(), .1)]
    LoadSensitiveFile(PathBuf, std::io::Error),
    #[error("Failed to set server TLS configuration: {}", .0)]
    TlsConfig(#[from] tonic::transport::Error),
    #[error("Failed to create directory {} for local socket: {}", path.display(), err)]
    CreateDir {
        path: PathBuf,
        err: std::io::Error,
    },
    #[error("Failed to create socket {}: {}", path.display(), err)]
    CreateSocket {
        path: PathBuf,
        err: std::io::Error,
    },
}

impl ApiConfig {
    /// Get the default gRPC timeout, used to provide a value for `serde`'s automatic Deserialize
    /// implementation
    pub const fn default_timeout() -> Duration {
        Duration::from_secs(120)
    }
}
