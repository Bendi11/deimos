use std::process::ExitCode;

use deimos_shared::util;
use serde::Deserialize;
use server::{Deimos, DeimosConfig};

mod server;
mod services;

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
            return ExitCode::FAILURE;
        }
    };

    let toml_de = toml::Deserializer::new(&config_str);
    let conf = match DeimosConfig::deserialize(toml_de) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to parse config file at {CONFIG_PATH}: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = Deimos::start(conf).await {
        tracing::error!("Failed to start Deimos server: {e}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
