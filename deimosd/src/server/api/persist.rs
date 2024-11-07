use std::path::Path;

use dashmap::DashSet;

use super::auth::{ApiAuthorization, ApiToken};



#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct ApiPersistent {
    pub tokens: ApiAuthorization,
}


impl ApiPersistent {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        match std::fs::File::open(path) {
            Ok(file) => match serde_json::from_reader(file) {
                Ok(this) => this,
                Err(e) => {
                    tracing::error!("Failed to load API persistent data from {}: {}", path.display(), e);
                    Self::default()
                }
            },
            Err(e) => {
                tracing::error!("Failed to open API persistence data from {}: {}", path.display(), e);
                Self::default()
            }
        }

    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ApiPersistError> {
        let writer = std::fs::File::create(path)?;
        serde_json::to_writer(writer, self)
            .map_err(Into::into)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiPersistError {
    #[error("I/O error: {}", .0)]
    IO(#[from] std::io::Error),
    #[error("Serialization error: {}", .0)]
    Serialize(#[from] serde_json::Error),
}
