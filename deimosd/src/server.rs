use std::path::Path;

use bollard::Docker;
use async_trait::async_trait;
use deimos_shared::{server::DeimosService, status::{ServerStatusRequest, ServerStatusResponse}};
use tonic::{Request, Response};


/// All maintained server state including Docker API connection,
/// certificates and CA public keys to use when authenticating clients
pub struct ServerState {
    pub docker: Docker,
}

#[async_trait]
impl DeimosService for ServerState {
    async fn server_status(&self, req: Request<ServerStatusRequest>) -> Result<Response<ServerStatusResponse>, tonic::Status> {
        Ok(Response::new(ServerStatusResponse { server_name: "Test Server".to_owned() }))
    }
}
