use std::{path::PathBuf, process::ExitCode, time::Duration};

use clap::{Parser, Subcommand};
use crossterm::{style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor, Stylize}, ExecutableCommand};
use futures::{future::BoxFuture, FutureExt};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Uri};
use tower::Service;

#[derive(Debug,)]
pub struct UnixSocketConnector(PathBuf);

pub async fn main() -> std::io::Result<ExitCode> {
    let args = DeimosCtlArgs::parse();
    let mut stdout = std::io::stdout();

    let channel = match Channel::builder(Uri::default())
        .connect_timeout(Duration::from_secs(args.timeout))
        .connect_with_connector(UnixSocketConnector(args.bind))
        .await {
        Ok(c) => c,
        Err(e) => return stdout
            .execute(SetForegroundColor(Color::Red))?
            .execute(Print(format_args!("Failed to connect to deimos daemon: {}", e)))?
            .execute(ResetColor)
            .map(|_| ExitCode::FAILURE)
    };

    let mut client = deimosproto::internal_client::InternalClient::new(channel);
    match args.cmd {
        DeimosCommand::Approve(approve) => {
            let request = deimosproto::ApproveRequest {
                username: approve.username.clone()
            };

            match client.approve(request).await {
                Ok(_) => stdout
                    .execute(SetForegroundColor(Color::Green))?
                    .execute(Print(format_args!("Approved token request for {}", approve.username.bold())))?
                    .execute(ResetColor)
                    .map(|_| ExitCode::SUCCESS),
                Err(e) => stdout
                    .execute(SetForegroundColor(Color::Red))?
                    .execute(Print(format_args!("Failed to approve token request for {}: {}", approve.username.bold(), e)))?
                    .execute(ResetColor)
                    .map(|_| ExitCode::FAILURE)
            }
        },
        DeimosCommand::List(..) => {
            let pending = client.get_pending(deimosproto::GetPendingRequest {}).await;
            let pending = match pending {
                Ok(v) => v.into_inner().pending,
                Err(e) => return stdout
                    .execute(SetForegroundColor(Color::Red))?
                    .execute(Print(format_args!("Failed to retrieve token requests: {}", e)))?
                    .execute(ResetColor)
                    .map(|_| ExitCode::FAILURE)
            };

            const USERNAME_HEADER: &str = "username";
            const DATETIME_HEADER: &str = "datetime";
            const REQIADDR_HEADER: &str = "address";

            //Width is constrained to 12 characters due to format string
            const DATETIME_WIDTH: usize = 12;

            let strings = pending.into_iter().map(|i| (
                i.username,
                chrono::DateTime::from_timestamp(i.requested_dt, 0).unwrap_or_default().format("%b %d, %Y"),
                i.requester_address.to_string()
            )).collect::<Vec<_>>();

            let max_username = strings.iter().map(|(username, _, _)| username.len()).max().unwrap_or_default();
            let uname_width = max_username.max(USERNAME_HEADER.len());
            
            let max_addr = strings.iter().map(|(_, _, addr)| addr.len()).max().unwrap_or_default();
            let addr_width = max_addr.max(REQIADDR_HEADER.len());

            stdout
                .execute(SetAttribute(Attribute::Bold))?
                .execute(Print(format_args!("{0:^1$}  {2:^3$}  {4:^5$}\n", USERNAME_HEADER, uname_width, DATETIME_HEADER, DATETIME_WIDTH, REQIADDR_HEADER, addr_width)))?
                .execute(SetAttribute(Attribute::NoBold))?;
            
            for (username, datetime, addr) in strings {
                stdout
                    .execute(Print(format_args!("{0:^1$}  {2:^3$}  {4:^5$}\n", username.bold(), uname_width, datetime, DATETIME_WIDTH, addr, addr_width)))?;
            }

            Ok(ExitCode::SUCCESS)
        }
    }
}

#[derive(Parser)]
#[command(about = "")]
struct DeimosCtlArgs {
    #[arg(long, help="Connection timeout in seconds", default_value="5")]
    timeout: u64,
    #[arg(short, long, default_value="/tmp/deimos/api")]
    bind: PathBuf,
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

impl Service<Uri> for UnixSocketConnector {
    type Response = TokioIo<UnixStream>;
    type Error = std::io::Error;

    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: Uri) -> Self::Future {
        UnixStream::connect(self.0.clone()).map(|stream| stream.map(TokioIo::new)).boxed()
    }
}
