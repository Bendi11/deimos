use iced::{Application, Settings};

mod app;

fn main() {
    if let Err(e) = app::DeimosApplication::run(Settings::with_flags(())) {
        eprintln!("Failed to start iced application: {e}");
    }
}
