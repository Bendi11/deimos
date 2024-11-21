use clap::{Parser, Subcommand};



fn main() {
    let args = DeimosCtlArgs::parse();
}

#[derive(Parser)]
#[command(about = "")]
struct DeimosCtlArgs {
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
