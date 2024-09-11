mod pb {
    tonic::include_proto!("deimos");
    pub mod status { tonic::include_proto!("deimos.status"); }
}

pub use crate::pb::*;

#[cfg(feature="server")]
pub mod server {
    pub use crate::pb::deimos_service_server::*;
}

#[cfg(feature="channel")]
pub mod channel {
    pub use crate::pb::deimos_service_client::*;
}
