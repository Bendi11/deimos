use std::{net::SocketAddr, path::PathBuf};


/// Configuration used to initialize the server's authentication data and Docker connection
#[derive(Debug, serde::Deserialize)]
pub struct DeimosConfig {
    pub bind: SocketAddr,
    pub keyfile: PathBuf,
}
