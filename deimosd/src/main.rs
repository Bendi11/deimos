use std::process::ExitCode;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{prelude::*, util::SubscriberInitExt, FmtSubscriber};

use deimos_shared::util;
use serde::Deserialize;
use server::{Deimos, DeimosConfig};

mod server;

const CONFIG_PATH: &str = "./deimos.toml";

#[tokio::main]
async fn main() -> ExitCode {
    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("bollard", LevelFilter::ERROR)
        .with_target("deimosd", LevelFilter::TRACE);

    let subscriber = FmtSubscriber::builder()
        .compact()
        .with_max_level(LevelFilter::TRACE)
        .with_ansi(true)
        .without_time()
        .finish();

    subscriber
        .with(filter)
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

    match Deimos::new(conf).await {
        Ok(server) => server.run().await,
        Err(e) => {
            tracing::error!("{e}");
            ExitCode::FAILURE
        }
    }
}
