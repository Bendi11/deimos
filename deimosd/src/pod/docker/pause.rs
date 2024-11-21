use std::sync::Arc;

use crate::pod::{state::{PodPaused, PodStateWriteHandle}, Pod, PodManager, PodStateKnown};

impl PodManager {
    /// Pause the given container if it is enabled and running, or no-op
    pub async fn pause(&self, pod: Arc<Pod>, mut lock: PodStateWriteHandle<'_>) -> Result<(), PausePodResult> {
        match lock.state() {
            PodStateKnown::Disabled => Err(PausePodResult::PodDisabled),
            PodStateKnown::Paused(..) => Ok(()),
            PodStateKnown::Enabled(ref run) => {
                self.docker
                    .pause_container(&run.docker_id)
                    .await
                    .map_err(PausePodResult::Docker)?;

                tracing::trace!("Paused container {} for {}", run.docker_id, pod.id());

                lock.set(PodStateKnown::Paused(PodPaused {
                    docker_id: run.docker_id.clone(),
                }));

                Ok(())
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PausePodResult {
    #[error("Pod is disabled")]
    PodDisabled,
    #[error("Pause API call failed: {0}")]
    Docker(#[source] bollard::errors::Error),
}
