use std::{path::PathBuf, time::Duration};

use chrono::DateTime;
use clap::{Parser, Subcommand};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Uri};
use tower::service_fn;


#[tokio::main]
async fn main() {
    let args = DeimosCtlArgs::parse();
    
    let channel = Channel::from_static("http://localhost:9115")
        .connect_timeout(Duration::from_secs(5))
        .connect_with_connector(service_fn(|_: Uri| async move {
            let uds = UnixStream::connect("/tmp/deimos/api").await?;

            Result::<_, std::io::Error>::Ok(TokioIo::new(uds))
        }))
        .await
        .unwrap();

    let mut client = deimosproto::internal_client::InternalClient::new(channel);
    match args.cmd {
        DeimosCommand::Approve(approve) => {
            client.approve(
                deimosproto::ApproveRequest {
                    username: approve.username
                }
            ).await.unwrap();
        },
        DeimosCommand::List(..) => {
            let pending = client.get_pending(deimosproto::GetPendingRequest {}).await.unwrap().into_inner().pending;
            println!("{:^16}|{:^32}|{:^16}", "username", "datetime", "address");
            for request in pending {
                let datetime = DateTime::from_timestamp(request.requested_dt, 0).map(|dt| dt.to_string()).unwrap_or(String::from("INVALID"));
                println!("{:^16}|{:^32}|{:^16}", request.username, datetime, request.requester_address);
            }
        }
    }
}

#[derive(Parser)]
#[command(about = "")]
struct DeimosCtlArgs {
    #[arg(short, long)]
    bind: Option<PathBuf>,
    #[command(subcommand)]
    cmd: DeimosCommand,
}

#[derive(Subcommand)]
enum DeimosCommand {
    #[command(name = "approve")]
    Approve(ApproveCommand),
    #[command(name = "list")]
    List(ListCommand),
}

#[derive(Parser)]
#[command(about = "Approve a pending token request with the given username")]
struct ApproveCommand {
    #[arg(help = "Username of the requested token")]
    username: String,
}

#[derive(Parser)]
#[command(about = "List the currently pending token requests")]
struct ListCommand {}
