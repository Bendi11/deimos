//! Implementation of public authorization and pod control gRPC endpoints


use std::sync::Arc;

use bytes::Bytes;
use futures::StreamExt;
use tonic::async_trait;

use deimosproto as proto;

use crate::{pod::{docker::logs::PodLogStream, id::DeimosId, PodState, PodStateStream}, server::Deimos};

use super::auth::PendingTokenStream;


#[async_trait]
impl deimosproto::authserver::DeimosAuthorization for Deimos {
    type RequestTokenStream = PendingTokenStream;

    async fn request_token(self: Arc<Self>, request: tonic::Request<deimosproto::TokenRequest>) -> Result<tonic::Response<Self::RequestTokenStream>, tonic::Status> {
        let requester = request.remote_addr().ok_or_else(|| tonic::Status::failed_precondition("Failed to get IP address of requester"))?;
        let username = Arc::from(request.into_inner().user);
        Ok(tonic::Response::new(self.api.auth.create_request(requester.ip(), username).await))
    }
}

#[async_trait]
impl proto::server::DeimosService for Deimos {
    async fn query_pods(
        self: Arc<Self>,
        _: tonic::Request<proto::QueryPodsRequest>,
    ) -> Result<tonic::Response<proto::QueryPodsResponse>, tonic::Status> {
        let pods = self
            .pods
            .iter()
            .map(|(_, pod)| proto::PodBrief {
                id: pod.id().owned(),
                title: pod.title().to_owned(),
                state: proto::PodState::from(pod.state().current()) as i32,
            })
            .collect::<Vec<_>>();

        Ok(tonic::Response::new(proto::QueryPodsResponse { pods }))
    }

    async fn update_pod(
        self: Arc<Self>,
        req: tonic::Request<proto::UpdatePodRequest>,
    ) -> Result<tonic::Response<proto::UpdatePodResponse>, tonic::Status> {
        let req = req.into_inner();
        let pod = self.lookup_pod(req.id)?;
        let id = pod.id();

        match proto::PodState::try_from(req.method) {
            Ok(proto::PodState::Disabled) => tokio::task::spawn(async move {
                let lock = pod.state().transact().await;
                if let Err(e) = self.pods.disable(pod.clone(), lock).await {
                    tracing::error!(
                        "Failed to disable pod {} in response to API request: {}",
                        id,
                        e
                    );
                }
            }),
            Ok(proto::PodState::Enabled) => tokio::task::spawn(async move {
                let lock = pod.state().transact().await;
                if let Err(e) = self.pods.enable(pod.clone(), lock).await {
                    tracing::error!(
                        "Failed to enable pod {} in response to API request: {}",
                        id,
                        e
                    );
                }
            }),
            Ok(proto::PodState::Paused) => tokio::task::spawn(async move {
                let lock = pod.state().transact().await;
                if let Err(e) = self.pods.pause(pod.clone(), lock).await {
                    tracing::error!(
                        "Failed to puase pod {} in response to API request: {}",
                        id,
                        e
                    );
                }
            }),
            Ok(proto::PodState::Transit) => {
                return Err(tonic::Status::invalid_argument(String::from(
                    "Cannot set pod to reserved state Transit",
                )))
            }
            Err(_) => {
                return Err(tonic::Status::invalid_argument(format!(
                    "Unknown pod state enumeration value {}",
                    req.method
                )))
            }
        };

        Ok(tonic::Response::new(proto::UpdatePodResponse {}))
    }

    type SubscribePodStatusStream = futures::stream::Map<
        PodStateStream,
        Box<PodStatusApiMapper>,
    >;

    async fn subscribe_pod_status(
        self: Arc<Self>,
        _: tonic::Request<proto::PodStatusStreamRequest>,
    ) -> Result<tonic::Response<Self::SubscribePodStatusStream>, tonic::Status> {
        let stream = self.pods.stream().map(Box::<PodStatusApiMapper>::from(Box::new(move |(id, state)| {
            Ok(proto::PodStatusNotification {
                id: id.owned(),
                state: proto::PodState::from(state) as i32,
            })
        })));

        Ok(tonic::Response::new(stream))
    }

    type SubscribePodLogsStream = futures::stream::Map<
        PodLogStream,
        Box<PodLogApiMapper>
    >;

    async fn subscribe_pod_logs(self: Arc<Self>, req: tonic::Request<proto::PodLogStreamRequest>) -> Result<tonic::Response<Self::SubscribePodLogsStream>, tonic::Status> {
        let pod = self.lookup_pod(req.into_inner().id)?;
        tracing::trace!("Client subscribed to logs for {}", pod.id());

        self
            .pods
            .subscribe_logs(pod)
            .await
            .map_err(|e| tonic::Status::failed_precondition(e.to_string()))
            .map(|sub|
                tonic::Response::new(
                    sub
                        .map(
                            Box::<PodLogApiMapper>::from(
                                Box::new(|bytes: Bytes|
                                    Ok(
                                        proto::PodLogChunk {
                                            chunk: bytes.to_vec()
                                        }
                                    )
                                )
                            )
                        )
                )
            )
    }
}

type PodStatusApiMapper = dyn FnMut((DeimosId, PodState)) -> Result<proto::PodStatusNotification, tonic::Status> + Send + Sync;
type PodLogApiMapper = dyn FnMut(Bytes) -> Result<proto::PodLogChunk, tonic::Status> + Send + Sync;
