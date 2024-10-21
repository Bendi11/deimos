use std::{collections::HashMap, sync::Arc};

use crate::pod::{config::PodDockerConfig, id::DockerId, manager::PodManager, Pod, PodRun, PodState};


impl PodManager {
    /// Top-level operation to enable the given pod.
    /// Creates and starts Docker container as required based on the current state of the pod.
    /// If the pod is already enabled, this is a no-op.
    pub async fn enable(&self, pod: Arc<Pod>) -> Result<(), PodEnableError> {
        let mut lock = pod.state.lock().await;
        let docker_id = match *lock {
            PodState::Enabled(..) => return Ok(()),
            PodState::Paused(ref paused) => paused.docker_id.clone(),
            PodState::Disabled => {
                let container = self.create_container(&pod).await?;
                if let Err(e) = self.start_container(&container).await {
                    tracing::warn!("Container for pod {} failed to start, destroying it", pod.id());
                    if let Err(e) = self.destroy_container(&container, true).await {
                        tracing::error!("Failsafe destroy failed for pod {}: {}", pod.id(), e);
                    }
                    
                    return Err(e)
                }

                container
            },
        };
        
        *lock = PodState::Enabled(
            PodRun {
                docker_id,
            }
        );

        Ok(())
    }
    
    
    async fn create_container(&self, pod: &Pod) -> Result<DockerId, PodEnableError> {        
        let config = docker_config(&pod.config.docker);
        let create_response = self
            .docker
            .create_container(
                Some(
                    bollard::container::CreateContainerOptions {
                        name: pod.id().owned(),
                        platform: None,
                    }
                ),
                config
            )
            .await
            .map_err(PodEnableError::CreateContainer)?;

        for warn in create_response.warnings {
            tracing::warn!("creating container for pod {}: {}", pod.id(), warn);
        }

        let docker_id = DockerId::from(create_response.id);
        tracing::trace!("Created container {} for {}", docker_id, pod.id());

        Ok(docker_id)
    }

    async fn start_container(&self, container: &DockerId) -> Result<(), PodEnableError> {
        self
            .docker
            .start_container(
                container,
                Option::<bollard::container::StartContainerOptions::<&'static str>>::None
            )
            .await
            .map_err(PodEnableError::StartContainer)
    }
}

/// Convert a [Pod](super::Pod)'s parsed [PodDockerConfig] to a type that can be used in the Docker
/// API
pub(super) fn docker_config(config: &PodDockerConfig) -> bollard::container::Config<String> {
    let image = Some(config.image.clone());

    let exposed_ports = (!config.port.is_empty()).then(|| {
        config
            .port
            .iter()
            .map(|conf| {
                (
                    format!("{}/{}", conf.expose, conf.protocol.docker_name()),
                    HashMap::new(),
                )
            })
            .collect()
    });

    let env = (!config.env.is_empty()).then(|| {
        config
            .env
            .iter()
            .map(|var| format!("{}={}", var.key, var.value))
            .collect()
    });

    let binds = (!config.volume.is_empty()).then(|| {
        config
            .volume
            .iter()
            .map(|volume| format!("{}:{}", volume.local.display(), volume.container.display()))
            .collect()
    });

    let host_config = Some(bollard::models::HostConfig {
        binds,
        ..Default::default()
    });

    bollard::container::Config {
        image,
        exposed_ports,
        env,
        host_config,
        ..Default::default()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PodEnableError {
    #[error("Failed to create Docker container: {0}")]
    CreateContainer(#[source] bollard::errors::Error),
    #[error("Failed to start Docker container: {0}")]
    StartContainer(#[source] bollard::errors::Error),
}
