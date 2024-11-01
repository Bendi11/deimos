use std::{ops::Deref, process::ExitCode, sync::Arc};

use fltk::{app::App, group::Group, prelude::{GroupExt, WidgetExt}, window::Window};
use header::Header;
use once_cell::sync::OnceCell;
use settings::Settings;
use tokio::sync::Mutex;

use crate::context::Context;


pub mod orbit;
pub mod widget;
pub mod header;
mod settings;


pub struct DeimosState {
    ctx: Context,
    active: Mutex<Group>,
    settings: Settings,
    header: Header,
}

#[derive(Clone, Default)]
pub struct DeimosStateHandle(Arc<OnceCell<DeimosState>>);

impl Deref for DeimosStateHandle {
    type Target = DeimosState;
    fn deref(&self) -> &Self::Target {
        self.0.wait()
    }
}

pub async fn run() -> ExitCode {
    let ctx = Context::load().await;
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

    let state = DeimosStateHandle::default();

    let settings = Settings::new(state.clone(), &mut window);
    let mut header = Header::create(state.clone(), &mut window);
    
    header.group_mut().hide();

    let _ = state.0.set(
        DeimosState {
            ctx,
            active: Mutex::new(settings.group().clone()),
            settings,
            header,
        }
    );

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
