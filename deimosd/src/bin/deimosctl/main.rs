use std::process::ExitCode;

#[cfg(unix)]
mod unix;

#[tokio::main]
async fn main() -> std::io::Result<ExitCode> {
    #[cfg(unix)]
    { unix::main().await }
    #[cfg(not(unix))]
    { Ok(ExitCode::SUCCESS) }
}
