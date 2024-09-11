use iced::{Application, Settings};
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity};

mod app;

#[tokio::main]
async fn main() {
    rustls::crypto::aws_lc_rs::default_provider().install_default().unwrap();
    let channel = Channel::from_static("https://localhost:9115")
        .tls_config(ClientTlsConfig::new()
            .domain_name("localhost")
            .ca_certificate(Certificate::from_pem(tokio::fs::read_to_string("../ca.pem").await.unwrap()))
            .identity(
                Identity::from_pem(
                    tokio::fs::read_to_string("./cert.pem").await.unwrap(),
                    tokio::fs::read_to_string("./key.pem").await.unwrap()
                )
            )
        )
        .unwrap()
        .connect()
        .await
        .unwrap();

    if let Err(e) = app::DeimosApplication::run(Settings::with_flags(channel)) {
        eprintln!("Failed to start iced application: {e}");
    }
}
