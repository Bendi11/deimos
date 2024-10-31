use std::process::ExitCode;

use app::DeimosApplication;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, FmtSubscriber};

pub mod context;
pub mod app;

fn main() -> ExitCode {
    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("deimos_client", LevelFilter::TRACE)
        .with_target("iced", LevelFilter::WARN)
        .with_target("tonic", LevelFilter::INFO);

    let subscriber = FmtSubscriber::builder()
        .with_max_level(LevelFilter::TRACE)
        .with_ansi(true)
        .compact()
        .without_time()
        .finish();

    subscriber.with(filter).init();

    DeimosApplication::run()
}
