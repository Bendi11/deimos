//! Implementation of the priviledged internal API served only over a unix domain socket
//! to a control application on the server.

use std::sync::Arc;

use tonic::async_trait;

use crate::server::Deimos;

#[async_trait]
impl deimosproto::internal_server::Internal for Deimos {
    async fn get_pending(self: Arc<Self>, _req: tonic::Request<deimosproto::GetPendingRequest>)
        -> Result<tonic::Response<deimosproto::GetPendingResponse>, tonic::Status> {
        let pending = self
            .api
            .auth
            .pending
            .iter()
            .map(|pending| pending.proto())
            .collect();

        Ok(
            tonic::Response::new(deimosproto::GetPendingResponse { pending })
        )
    }

    async fn approve(self: Arc<Self>, req: tonic::Request<deimosproto::ApproveRequest>)
        -> Result<tonic::Response<deimosproto::ApproveResponse>, tonic::Status> {
        let user = req.into_inner().username;

        let pending = self.api.auth.pending.remove(&*user);

        match pending {
            Some((user, pend)) => {
                tracing::info!("Approved token request for '{}'", user);

                self
                    .api
                    .auth
                    .approve(pend)
                    .await
                    .map(|_| tonic::Response::new(deimosproto::ApproveResponse {}))
                    .map_err(|e| tonic::Status::internal(e.to_string()))
            },
            None => {
                Err(
                    tonic::Status::not_found(format!("Request with username {} not found", user))
                )
            }
        }
    }
}

