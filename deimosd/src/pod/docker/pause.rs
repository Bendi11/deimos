use std::sync::Arc;

use crate::pod::{manager::PodManager, Pod, PodPaused, PodStateKnown};



impl PodManager {
    /// Pause the given container if it is enabled and running, or no-op
    pub async fn pause(&self, pod: Arc<Pod>) -> Result<(), PausePodResult> {
        let mut lock = pod.state_lock().await;
        match lock.state() {
            PodStateKnown::Disabled => Err(PausePodResult::PodDisabled),
            PodStateKnown::Paused(..) => Ok(()),
            PodStateKnown::Enabled(ref run) => {
                self
                    .docker
                    .pause_container(&run.docker_id)
                    .await
                    .map_err(PausePodResult::Docker)?;

                lock.set(PodStateKnown::Paused(
                    PodPaused {
                        docker_id: run.docker_id.clone()
                    }
                ));

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
