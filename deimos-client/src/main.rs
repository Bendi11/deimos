use app::settings::ApplicationSettings;
use iced::{Application, Settings};

mod app;

#[tokio::main]
async fn main() {
    if let Err(e) = app::DeimosApplication::run(Settings::with_flags(ApplicationSettings::default())) {
        eprintln!("Failed to start iced application: {e}");
    }
}
