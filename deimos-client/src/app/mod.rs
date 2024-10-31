use std::{process::ExitCode, sync::Arc};

use fltk::{app::App, prelude::{GroupExt, WidgetExt}, window::Window};
use header::Header;
use settings::Settings;

use crate::context::Context;


pub mod orbit;
pub mod widget;
pub mod header;
mod settings;



pub async fn run() -> ExitCode {
    let ctx = Arc::new(Context::load().await);
    let fltk_ev = App::default()
        .with_scheme(fltk::app::Scheme::Gtk);
    
    widget::orbit_scheme();

    let mut window = Window::default()
        .with_size(400, 600);
    window.set_color(orbit::NIGHT[2]);
    window.make_resizable(true);
    window.set_label("Deimos");

    window.end();
    window.show();

    

    let mut settings = Settings::new(ctx.clone(), &mut window);
    let mut header = Header::create(&mut window);
    
    header.group_mut().hide();

    /*for pod in ctx.pods.values() {
        let mut button = Button::default();
        button.set_size(window.width(), window.height() / 6);
        button.set_color(Color::from_rgb(0x1b, 0x1b, 0x1c));
        button.set_label(&pod.data.name);
        button.set_label_color(Color::from_rgb(0xff, 0xf4, 0xea));
        window.add(&button);
    }*/

    window.redraw();

    match fltk_ev.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("Failed to run FLTK event loop: {}", e);
            ExitCode::FAILURE
        }
    }
}
