use std::{str::Split, sync::Arc};

use bollard::container::{AttachContainerOptions, AttachContainerResults, LogOutput};
use futures::StreamExt;
use tokio::sync::Mutex;

use crate::server::{docker::{BollardError, DockerService}, Deimos};


pub struct ValheimService {
    docker: Arc<DockerService>,
    container: Mutex<AttachContainerResults>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ValheimConfig {
    pub container: String,
}

impl ValheimService {
    pub async fn new(config: Option<ValheimConfig>, docker: Arc<DockerService>) -> Result<Self, BollardError> {
        let config = config.unwrap_or_default();

        let container = docker
            .client()
            .attach_container(
                &config.container,
                Some(AttachContainerOptions::<String> {
                        stdin: Some(false),
                        stdout: Some(true),
                        stderr: Some(false),
                        stream: Some(true),
                        logs: Some(false),
                        detach_keys: None,
                })
            ).await?;

        Ok(Self {
            docker,
            container: Mutex::new(container),
        })
    }

    pub async fn run(self: Arc<Self>) -> ! {
        let mut container = self.container.lock().await;
        let mut player_count = 0usize;

        loop {
            if let Some(log) = container.output.next().await {
                match log {
                    Ok(LogOutput::StdOut { message }) => {
                        if let Ok(message) = std::str::from_utf8(message.as_ref()) {
                            tracing::info!("Got log message {message}");
                            if let Some(conn) = message.rsplit("Got connection SteamID").next() {
                                let steam_id = conn.trim();
                                tracing::info!("Player with steam ID {steam_id} connected");
                            } else if let Some(disc) = message.rsplit("Closing socket").next() {
                                let steam_id = disc.trim();
                                tracing::info!("Player with steam ID {steam_id} disconnected");
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to read log output from valheim container: {e}");
                    },
                    _ => (),
                }
            }
        }
    }
}


impl Default for ValheimConfig {
    fn default() -> Self {
        Self {
            container: String::from("valheim"),
        }
    }
}
