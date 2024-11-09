use std::path::{Path, PathBuf};

use super::{client::ContextPersistent, Context};

impl Context {
    /// File located in the context's cache directory that stores serialized state
    pub const STATE_FILE_NAME: &str = "state.json";

    /// Load application context state from the local cache directory, or create a default one
    pub fn load_state(cache_dir: &Path) -> Result<ContextPersistent, LoadStateError> {
        let config_path = cache_dir.join(Self::STATE_FILE_NAME);
        match std::fs::File::open(&config_path) {
            Ok(rdr) => Ok(
                serde_json::from_reader::<_, ContextPersistent>(rdr).map_err(|e| LoadStateError {
                    config_path: config_path.clone(),
                    kind: LoadStateErrorKind::Parse(e),
                })?,
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(
                    "Failed to load config file from {}: creating a default",
                    config_path.display()
                );

                if !std::fs::exists(cache_dir).unwrap_or(false) {
                    if let Err(e) = std::fs::create_dir(cache_dir) {
                        tracing::warn!(
                            "Failed to create config directory {}: {}",
                            cache_dir.display(),
                            e
                        );
                    }
                }

                let config = ContextPersistent::default();

                let file = std::fs::File::create(&config_path).map_err(|e| LoadStateError {
                    config_path: config_path.clone(),
                    kind: LoadStateErrorKind::FailedToCreateDefault(e),
                })?;

                serde_json::to_writer(file, &config).map_err(|e| LoadStateError {
                    config_path: config_path.clone(),
                    kind: LoadStateErrorKind::Serialize(e),
                })?;

                Ok(config)
            }
            Err(e) => Err(LoadStateError {
                config_path,
                kind: LoadStateErrorKind::FailedToOpen(e),
            }),
        }
    }

    /// Write all context state to the save file located in the cache directory
    pub fn save_state(&self) {
        let state_path = self.cache_dir.join(Self::STATE_FILE_NAME);

        match std::fs::File::create(&state_path) {
            Ok(w) => {
                if let Err(e) = serde_json::to_writer::<_, ContextPersistent>(w, &self.clients.persistent()) {
                    tracing::error!(
                        "Failed to write context state to '{}': {}",
                        state_path.display(),
                        e
                    );
                }
            }
            Err(e) => {
                tracing::error!(
                    "Failed to create context state file '{}': {}",
                    state_path.display(),
                    e
                );
            }
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
