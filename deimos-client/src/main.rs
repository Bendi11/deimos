use std::{path::Path, process::ExitCode};

use app::settings::ApplicationSettings;
use iced::{Application, Settings};

mod app;

#[tokio::main]
async fn main() -> ExitCode {
    let args = std::env::args()
        .iter()
        .collect();

    let config_dir = if args.len() == 2 {
        PathBuf::from(args[1])
    } else {
        match dirs::config_local_dir() {
            Some(dir) => dir.join(app::DeimosApplication::CONFIG_DIR_NAME),
            None => Path::new("./").join(app::DeimosApplication::CONFIG_DIR_NAME)
        }
    };

    let config_path = config_dir.join(app::DeimosApplication::CONFIG_FILE_NAME);
    let config = match tokio::fs::File::open(&config_path).await {
        Ok(rdr) => match serde_json::from_reader::<ApplicationSettings>(rdr) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::error!("Failed to parse configuration file {}: {}", config_path.display(), e);
                return ExitCode::FAILURE
            }
        },
        Err(e) => {
            tracing::warn!("Failed to load config file from {}: creating a default", config_path.display());
            let config = ApplicationSettings::default();
            
            let mut file = match tokio::fs::File::create(&config_path).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!("Failed to create default config file {}: {}", config_path.display(), e);
                    return ExitCode::FAILURE
                }
            };

            if let Err(e) =  serde_json::to_writer(file, &config) {
                tracing::error!("Failed to write default JSON config file {}: {}", config_path.display(), e);
                return ExitCode::FAILURE
            }
        }
    };

    if let Err(e) = app::DeimosApplication::run(Settings::with_flags(config)) {
        eprintln!("Failed to start iced application: {e}");
    }
}
