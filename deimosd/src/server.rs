use std::sync::Arc;

use chacha20poly1305::{ChaCha20Poly1305, Key, KeySizeUser};
use tokio::net::TcpListener;

use crate::{config::DeimosConfig, util};


/// All maintained server state including Docker API connection,
/// certificates and CA public keys to use when authenticating clients
pub struct Server {
    listener: TcpListener,
    //pub docker: Docker,
}

impl Server {
    /// Create a new server instance, loading all required files from the configuration specified
    /// and creating a TCP listener for the control interface.
    pub async fn new(config: DeimosConfig) -> Result<Self, ServerInitError> {
        let key_str = util::load_check_permissions(&config.keyfile).await?;
        let key_pem = pem::parse(key_str)?;
        if key_pem.tag() != "DEIMOS SYMMETRIC KEY" {
            tracing::warn!("key pem file has unrecognized tag {}", key_pem.tag());
        }

        if key_pem.contents().len() != ChaCha20Poly1305::key_size() {
            return Err(ServerInitError::InvalidKeySize(key_pem.contents().len()))
        }

        let key = Key::from_slice(key_pem.contents());

        let listener = TcpListener::bind(config.bind).await?;

        Ok(Self {
            listener
        })
    }

    pub async fn serve(self: Arc<Self>) {
        loop {
            match self.listener.accept().await {
                Ok((conn, addr)) => {
                    tracing::debug!("Accepted connection from {addr}");
                },
                Err(e) => {
                    tracing::warn!("Failed to accept TCP connection: {e}");
                }
            }
        }
    }
}


#[derive(Debug, thiserror::Error)]
pub enum ServerInitError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("PEM decode error: {0}")]
    PEM(#[from] pem::PemError),
    #[error("Invalid key size: {}B")]
    InvalidKeySize(usize),
}
