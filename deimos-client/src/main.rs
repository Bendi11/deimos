use std::{path::{Path, PathBuf}, process::ExitCode};

use app::settings::ApplicationSettings;
use iced::{Application, Settings};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, FmtSubscriber};

mod app;

#[tokio::main]
async fn main() -> ExitCode {
    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("deimos_client", LevelFilter::TRACE)
        .with_target("iced", LevelFilter::WARN)
        .with_target("tonic", LevelFilter::WARN);

    let subscriber = FmtSubscriber::builder()
        .with_max_level(LevelFilter::TRACE)
        .with_ansi(true)
        .compact()
        .without_time()
        .finish();

    subscriber
        .with(filter)
        .init();

    let mut args = std::env::args()
        .collect::<Vec<_>>();

    let config_dir = if args.len() == 2 {
        PathBuf::from(args.pop().unwrap())
    } else {
        match dirs::config_local_dir() {
            Some(dir) => dir.join(app::DeimosApplication::CONFIG_DIR_NAME),
            None => Path::new("./").join(app::DeimosApplication::CONFIG_DIR_NAME)
        }
    };

    let config_path = config_dir.join(app::DeimosApplication::CONFIG_FILE_NAME);
    let config = match std::fs::File::open(&config_path) {
        Ok(rdr) => match serde_json::from_reader::<_, ApplicationSettings>(rdr) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::error!("Failed to parse configuration file {}: {}", config_path.display(), e);
                return ExitCode::FAILURE
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!("Failed to load config file from {}: creating a default", config_path.display());

            if !std::fs::exists(&config_dir).unwrap_or(false) {
                if let Err(e) = std::fs::create_dir(&config_dir) {
                    tracing::warn!("Failed to create config directory {}: {}", config_dir.display(), e);
                }
            }

            let config = ApplicationSettings::default();
            
            let file = match std::fs::File::create(&config_path) {
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

            config
        },
        Err(e) => {
            tracing::error!("Failed to load configuration file {}: {}", config_path.display(), e);
            return ExitCode::FAILURE
        }
    };

    if let Err(e) = app::DeimosApplication::run(Settings::with_flags(config)) {
        eprintln!("Failed to start iced application: {e}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
