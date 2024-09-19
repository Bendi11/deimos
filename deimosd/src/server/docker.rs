use bollard::Docker;


pub struct DockerService {
    pub config: Option<DockerConfig>,
    docker: Docker,
}

/// Configuration for connecting to the local Docker API
#[derive(Debug, serde::Deserialize)]
pub struct DockerConfig {
    pub kind: DockerConnectionType,
    pub addr: String,
    pub timeout: Option<u64>,
}

#[derive(Debug, serde::Deserialize)]
pub enum DockerConnectionType {
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "local")]
    Local,
}

pub type BollardError = bollard::errors::Error;

impl DockerService {
    pub const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

    pub async fn new(config: Option<DockerConfig>) -> Result<Self, BollardError> {
        let docker = match config {
            None => {
                tracing::trace!("No docker config given, using platform defaults to connect");
                Docker::connect_with_local_defaults()
            },
            Some(ref cfg) => {
                let timeout = cfg.timeout.unwrap_or(Self::DEFAULT_TIMEOUT_SECONDS);
                match cfg.kind {
                    DockerConnectionType::Http => Docker::connect_with_http(
                        &cfg.addr,
                        timeout,
                        bollard::API_DEFAULT_VERSION
                    ),
                    DockerConnectionType::Local => Docker::connect_with_socket(
                        &cfg.addr,
                        timeout,
                        bollard::API_DEFAULT_VERSION
                    )
                }
            }
        }?;

        Ok(Self {
            config,
            docker,
        })
    }
    
    /// Get a handle to the connected Docker client
    pub fn client(&self) -> &Docker {
        &self.docker
    }
}
