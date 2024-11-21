use std::path::PathBuf;

use clap::{Parser, Subcommand};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Uri};
use tower::service_fn;



#[tokio::main]
async fn main() {
    let args = DeimosCtlArgs::parse();
    
    let bind = args.bind.clone();
    let channel = Channel::builder(Uri::from_static("/"))
        .connect_with_connector(service_fn(|uri: Uri| async {
            let uds = UnixStream::connect(uri).await?;

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
        DeimosCommand::List(list) => {
            let pending = client.get_pending(deimosproto::GetPendingRequest {}).await.unwrap().into_inner().pending;
            println!("USERNAME | REQUESTED DATETIME | REMOTE");
            for request in pending {
                println!("");
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
