use std::{net::IpAddr, path::PathBuf};



/// Configuration used to initialize the server's authentication data and Docker connection
#[derive(Debug, serde::Deserialize)]
pub struct DeimosConfig {
    pub bind: IpAddr,
    pub port: u16,
    pub keyfile: PathBuf,
}
