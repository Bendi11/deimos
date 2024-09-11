use std::{net::IpAddr, path::{Path, PathBuf}};



/// Configuration used to initialize the server's authentication data and Docker connection
#[derive(Debug, serde::Deserialize)]
pub struct DeimosConfig {
    pub bind: IpAddr,
    pub port: u16,
    pub cert: DeimosAuthenticationConfig
}

#[derive(Debug, serde::Deserialize)]
pub struct DeimosAuthenticationConfig {
    /// Path to the X.509 certificate used as the CA when authenticating client connections
    pub ca_root: PathBuf,
    /// Path to public X.509 certificate to present to clients
    pub identity_cert: PathBuf,
    /// Path to the server's private key file
    pub identity_key: PathBuf,
}

impl Default for DeimosAuthenticationConfig {
    fn default() -> Self {
        Self {
            ca_root: PathBuf::from("./ca_cert.pem"),
            identity_cert: PathBuf::from("./identity.pem"),
            identity_key: PathBuf::from("./identity_key.pem")
        }
    }
}
