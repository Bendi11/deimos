use std::{path::Path, process::ExitCode};

use config::DeimosConfig;
use logger::Logger;
use serde::Deserialize;
use tokio::{fs::File, io::AsyncReadExt};

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

    let Ok(mut config_file) = load_check_permissions(CONFIG_PATH).await else { return ExitCode::FAILURE };

    let mut config_str = String::new();
    if let Err(e) = config_file.read_to_string(&mut config_str).await {
        log::error!("Failed to read config file: {e}");
        return ExitCode::FAILURE
    }
    
    let toml_de = toml::Deserializer::new(&config_str);
    let config = match DeimosConfig::deserialize(toml_de) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to parse config file at {CONFIG_PATH}: {e}");
            return ExitCode::FAILURE
        }
    };
    
    ExitCode::SUCCESS
}

async fn load_check_permissions(path: impl AsRef<Path>) -> Result<File, std::io::Error> {
    let file = File::open(&path).await?;

    #[cfg(unix)]
    {
        let meta = file.metadata().await?;
        let permissions = meta.permissions();
        use std::os::unix::fs::PermissionsExt;
        let mode = permissions.mode();
        if mode & 0o077 != 0 {
            log::error!("Sensitive file {} has group and/or other read/write permissions - change to 600 or 400", path.as_ref().display());
            return Err(tokio::io::ErrorKind::InvalidInput.into())
        }
    }

    Ok(file)
}
