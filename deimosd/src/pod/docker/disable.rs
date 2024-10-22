use std::sync::Arc;

use crate::pod::{id::DockerId, manager::PodManager, Pod, PodStateKnown};



impl PodManager {
    /// Top-level operation to disable the given pod.
    /// Gracefully, then forcefully stops and removes the Docker container as required.
    pub async fn disable(&self, pod: Arc<Pod>) -> Result<(), PodDisableError> {
        let mut lock = pod.state_lock().await;
        let docker_id = match lock.state() {
            PodStateKnown::Disabled => return Ok(()),
            PodStateKnown::Paused(ref paused) => paused.docker_id.clone(),
            PodStateKnown::Enabled(ref running) => {
                self.stop_container(&running.docker_id, pod.config.docker.stop_timeout).await?;
                running.docker_id.clone()
            }
        };

        if let Err(e) = self.destroy_container(&docker_id, false).await {
            tracing::error!("Failed to destroy container {} for {}, attempting forcefully: {}", docker_id, pod.id(), e);
            if let Err(e) = self.destroy_container(&docker_id, true).await {
                tracing::error!("Failed to destroy container for {} forcefully: {}", pod.id(), e);
            }
        }

        lock.set(PodStateKnown::Disabled);
        Ok(())
    }

    async fn stop_container(&self, container: &DockerId, t: u32) -> Result<(), PodDisableError> {
        self
            .docker
            .stop_container(
                container,
                Some(
                    bollard::container::StopContainerOptions {
                        t: t as i64,
                    }
                )
            )
            .await
            .map_err(PodDisableError::Stop)
    }
    
    pub(super) async fn destroy_container(&self, container: &DockerId, force: bool) -> Result<(), PodDisableError> {
        self
            .docker
            .remove_container(
                container,
                Some(
                    bollard::container::RemoveContainerOptions {
                        force,
                        ..Default::default()
                    }
                )
            )
            .await
            .map_err(PodDisableError::Destroy)
    }

}

#[derive(Debug, thiserror::Error)]
pub enum PodDisableError {
    #[error("Failed to destroy Docker container: {0}")]
    Destroy(#[source] bollard::errors::Error),
    #[error("Failed to stop Docker container: {0}")]
    Stop(#[source] bollard::errors::Error),
}
