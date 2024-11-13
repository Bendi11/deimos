//#![windows_subsystem = "windows"]

use std::{io::Stdout, process::ExitCode, sync::Mutex};

use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt::writer::EitherWriter, layer::SubscriberExt, util::SubscriberInitExt, FmtSubscriber};

pub mod context;
pub mod app;

pub enum LogWriter {
    File(std::fs::File),
    Stdout(Stdout),
}

#[tokio::main]
async fn main() -> ExitCode {
    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("deimos_client", LevelFilter::TRACE)
        .with_target("iced", LevelFilter::WARN)
        .with_target("tonic", LevelFilter::INFO);

    let log_path = std::env::args().nth(1);

    let log_file = match log_path.map(std::fs::File::create) {
        Some(Ok(file)) => EitherWriter::A(file),
        _ => EitherWriter::B(std::io::stdout()),
    };

    let log_file = Mutex::new(log_file);

    let subscriber = FmtSubscriber::builder()
        .with_writer(log_file)
        .with_max_level(LevelFilter::TRACE)
        .with_ansi(true)
        .compact()
        .without_time()
        .finish();

    subscriber.with(filter).init();

    app::run().await
}


