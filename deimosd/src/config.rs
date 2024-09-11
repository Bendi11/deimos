use std::path::Path;



/// Configuration used to initialize the server's authentication data and Docker connection
#[derive(Debug, serde::Deserialize)]
pub struct DeimosConfig<'a> {
    pub port: Option<u16>,
    #[serde(borrow)]
    pub cert: DeimosAuthenticationConfig<'a>
}

#[derive(Debug, serde::Deserialize)]
pub struct DeimosAuthenticationConfig<'a> {
    /// Path to the X.509 certificate used as the CA when authenticating client connections
    #[serde(borrow)]
    pub ca_root: &'a Path,
    /// Path to public X.509 certificate to present to clients
    #[serde(borrow)]
    pub identity_cert: &'a Path,
    /// Path to the server's private key file
    #[serde(borrow)]
    pub identity_key: &'a Path,
}

impl Default for DeimosAuthenticationConfig<'static> {
    fn default() -> Self {
        Self {
            ca_root: Path::new("./ca_cert.pem"),
            identity_cert: Path::new("./identity.pem"),
            identity_key: Path::new("./identity_key.pem")
        }
    }
}
