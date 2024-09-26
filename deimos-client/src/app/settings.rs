use http::Uri;


/// Settings that can be changed in the UI to configure server connection
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ConnectionSettings {
    #[serde(with = "http_serde::uri")]
    pub server_uri: Uri,
}

/// Global settings configuring server connection and UI styling
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ApplicationSettings {
    pub conn: ConnectionSettings
}

impl Default for ApplicationSettings {
    fn default() -> Self {
        Self {
            conn: ConnectionSettings {
                server_uri: Uri::default()
            }
        }
    }
}
