use std::process::ExitCode;

use fltk::{app::App, enums::Color, prelude::{GroupExt, WidgetExt}, window::Window};
use header::Header;

use crate::context::Context;

pub mod header;

pub struct DeimosApplication {
    ctx: Context,
    fltk_ev: App,
    window: Window,
    header: Header,
}

impl DeimosApplication {
    pub fn run() -> ExitCode {
        let ctx = Context::load();
        let fltk_ev = App::default()
            .with_scheme(fltk::app::Scheme::Gtk);

        let mut window = Window::default()
            .with_size(400, 600);
        window.set_color(Color::from_rgb(0x12, 0x12, 0x14));
        window.make_resizable(true);
        window.set_label("Deimos");

        window.end();
        window.show();

        let header = Header::create(&mut window);

        window.redraw();

        let this = Self {
            ctx,
            fltk_ev,
            window,
            header,
        };

        match this.fltk_ev.run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                tracing::error!("Failed to run FLTK event loop: {}", e);
                ExitCode::FAILURE
            }
        }
    }
}
