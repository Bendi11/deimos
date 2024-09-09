use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use deimos_shared::{server::{Deimos, DeimosServer}, ServerStatus, StatusRequest};
use async_trait::async_trait;
use tonic::transport::Server;

pub struct ServerState {

}

#[async_trait]
impl Deimos for ServerState {
    async fn status(&self, req: tonic::Request<StatusRequest>) -> Result<tonic::Response<ServerStatus>, tonic::Status> {
        Ok(tonic::Response::new(ServerStatus { server_name: "Test".to_owned() }))
    }
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9115);

    Server::builder()
        .add_service(DeimosServer::new(ServerState {}))
        .serve(addr)
        .await;
}
