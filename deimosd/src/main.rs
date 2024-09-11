use std::{net::{IpAddr, SocketAddr}, path::Path, process::ExitCode, str::FromStr, time::Duration};

use config::DeimosConfig;
use deimos_shared::server::DeimosServiceServer;
use igd_next::{PortMappingProtocol, SearchOptions};
use logger::Logger;
use serde::Deserialize;
use server::ServerState;
use tokio::{fs::File, io::AsyncReadExt};
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};

mod config;
mod server;
mod logger;

const CONFIG_PATH: &str = "./deimos.toml";

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(e) = Logger::install() {
        eprintln!("Failed to initialize logger: {e}");
        return ExitCode::FAILURE
    }

    if let Err(e) = rustls::crypto::aws_lc_rs::default_provider().install_default() {
        log::error!("Failed to install default rustls cryptography provider: {e:?}");
        return ExitCode::FAILURE
    }

    log::trace!("Installed aws_lc crypto provider");

    let config_str = match load_check_permissions(CONFIG_PATH).await {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to load config file {CONFIG_PATH}: {e}");
            return ExitCode::FAILURE
        }
    };
    
    let toml_de = toml::Deserializer::new(&config_str);
    let conf = match DeimosConfig::deserialize(toml_de) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to parse config file at {CONFIG_PATH}: {e}");
            return ExitCode::FAILURE
        }
    };

    let ca_cert = match load_check_permissions(&conf.cert.ca_root).await {
        Ok(v) => Certificate::from_pem(v),
        Err(e) => {
            log::error!("Failed to load CA certificate at {}: {e}", conf.cert.ca_root.display());
            return ExitCode::FAILURE
        }
    };

    let identity_cert = match load_check_permissions(&conf.cert.identity_cert).await {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to load identity certificate {}: {e}", conf.cert.identity_cert.display());
            return ExitCode::FAILURE
        }
    };

    let identity_key = match load_check_permissions(&conf.cert.identity_key).await {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to load identity key {}: {e}", conf.cert.identity_key.display());
            return ExitCode::FAILURE
        }
    };

    let gateway = match igd_next::aio::tokio::search_gateway(SearchOptions {
        timeout: Some(Duration::from_secs(60)),
        ..Default::default()
    }).await {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to discover IGD enabled device: {e}");
            return ExitCode::FAILURE
        }
    };

    log::trace!("Found IGD gateway {}", gateway.addr);

    if let Err(e) = gateway.add_port(
        PortMappingProtocol::TCP,
        conf.port,
        SocketAddr::new(IpAddr::from_str("192.168.1.204").unwrap(), conf.port),
        10000,
        "test IGD"
    ).await {
        log::error!("Failed to add port mapping: {e}");
    }

    let state = ServerState {

    };

    let server = match Server::builder()
        .tls_config(ServerTlsConfig::new()
            .identity(Identity::from_pem(identity_cert, identity_key))
            .client_ca_root(ca_cert)
            .client_auth_optional(false)
        ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to set TLS config for server: {e:?}");
            return ExitCode::FAILURE
        }
    };
    
    log::trace!("Starting tonic gRPC server");

    match server
        .timeout(Duration::from_secs(30))
        .add_service(DeimosServiceServer::new(state))
        .serve(SocketAddr::new(conf.bind, conf.port))
        .await {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("tonic server error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn load_check_permissions(path: impl AsRef<Path>) -> Result<String, std::io::Error> {
    let mut file = File::open(&path).await?;
    let meta = file.metadata().await?;

    #[cfg(unix)]
    {
        let permissions = meta.permissions();
        use std::os::unix::fs::PermissionsExt;
        let mode = permissions.mode();
        if mode & 0o077 != 0 {
            log::error!("Sensitive file {} has group and/or other read/write permissions - change to 600 or 400", path.as_ref().display());
            return Err(tokio::io::ErrorKind::InvalidInput.into())
        }
    }
    
    let mut string = String::with_capacity(meta.len() as usize);
    file.read_to_string(&mut string).await?;

    Ok(string)
}
