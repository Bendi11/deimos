use std::{process::ExitCode, time::Duration};

use config::DeimosConfig;
use igd_next::{PortMappingProtocol, SearchOptions};
use serde::Deserialize;
use server::Server;
use deimos_shared::util;

mod config;
mod server;

const CONFIG_PATH: &str = "./deimos.toml";

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .pretty()
        .with_ansi(true)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let config_str = match util::load_check_permissions(CONFIG_PATH).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to load config file {CONFIG_PATH}: {e}");
            return ExitCode::FAILURE
        }
    };
    
    let toml_de = toml::Deserializer::new(&config_str);
    let conf = match DeimosConfig::deserialize(toml_de) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to parse config file at {CONFIG_PATH}: {e}");
            return ExitCode::FAILURE
        }
    };

    
    let gateway = match igd_next::aio::tokio::search_gateway(SearchOptions {
        timeout: Some(Duration::from_secs(60)),
        ..Default::default()
    }).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to discover IGD enabled device: {e}");
            return ExitCode::FAILURE
        }
    };

    tracing::trace!("Found IGD gateway {}", gateway.addr);

    /*if let Err(e) = gateway.add_port(
        PortMappingProtocol::TCP,
        conf.port,
        SocketAddr::new(IpAddr::from_str("192.168.1.204").unwrap(), conf.port),
        1,
        "test IGD"
    ).await {
        tracing::error!("Failed to add port mapping: {e}");
    }*/

    match Server::new(conf).await {
        Ok(server) => {
            server.serve().await
        },
        Err(e) => {
            tracing::error!("Failed to initialize server - {e}");
            ExitCode::FAILURE
        }
    } 
}


