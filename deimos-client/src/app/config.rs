use std::path::{Path, PathBuf};

use super::{DeimosApplication, DeimosApplicationState};


impl DeimosApplication {
    pub(super) fn load_config() -> Result<DeimosApplicationState, LoadStateError> {
        let mut args = std::env::args()
            .collect::<Vec<_>>();

        let config_dir = if args.len() == 2 {
            PathBuf::from(args.pop().unwrap())
        } else {
            match dirs::config_local_dir() {
                Some(dir) => dir.join(DeimosApplication::CONFIG_DIR_NAME),
                None => Path::new("./").join(DeimosApplication::CONFIG_DIR_NAME)
            }
        };

        let config_path = config_dir.join(DeimosApplication::CONFIG_FILE_NAME);
        match std::fs::File::open(&config_path) {
            Ok(rdr) => Ok(serde_json::from_reader::<_, DeimosApplicationState>(rdr)
                    .map_err(|e| LoadStateError { config_path: config_path.clone(), kind: LoadStateErrorKind::Parse(e)})?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!("Failed to load config file from {}: creating a default", config_path.display());

                if !std::fs::exists(&config_dir).unwrap_or(false) {
                    if let Err(e) = std::fs::create_dir(&config_dir) {
                        tracing::warn!("Failed to create config directory {}: {}", config_dir.display(), e);
                    }
                }

                let config = DeimosApplicationState::default();
                
                let file = std::fs::File::create(&config_path)
                        .map_err(|e| LoadStateError { config_path: config_path.clone(), kind: LoadStateErrorKind::FailedToCreateDefault(e) })?;

                serde_json::to_writer(file, &config)
                    .map_err(|e| LoadStateError { config_path: config_path.clone(), kind: LoadStateErrorKind::Serialize(e) })?;

                Ok(config)
            },
            Err(e) => Err(
                LoadStateError {
                    config_path,
                    kind: LoadStateErrorKind::FailedToOpen(e)
                }
            )
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Loading config file '{config_path}': {kind}")]
pub struct LoadStateError {
    pub config_path: PathBuf,
    pub kind: LoadStateErrorKind,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadStateErrorKind {
    #[error("Failed to create default config file {0}")]
    FailedToCreateDefault(#[source] std::io::Error),
    #[error("Failed to open config file: {0}")]
    FailedToOpen(#[source] std::io::Error),
    #[error("Failed to parse config file: {0}")]
    Parse(#[source] serde_json::Error),
    #[error("Failed to serialize config: {0}")]
    Serialize(#[source] serde_json::Error),
}
