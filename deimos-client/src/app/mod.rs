use std::{ops::Deref, process::ExitCode, sync::Arc};

use fltk::{app::App, group::Group, prelude::{GroupExt, WidgetExt}, window::Window};
use once_cell::sync::OnceCell;
use over::Overview;
use settings::Settings;
use tokio::sync::Mutex;

use crate::context::Context;


pub mod orbit;
pub mod widget;
pub mod over;
mod settings;


pub struct DeimosState {
    ctx: Context,
    active: Mutex<Group>,
    settings: Settings,
    overview: Overview,
}

#[derive(Clone, Default)]
pub struct DeimosStateHandle(Arc<OnceCell<DeimosState>>);

impl Deref for DeimosStateHandle {
    type Target = DeimosState;
    fn deref(&self) -> &Self::Target {
        self.0.wait()
    }
}

impl DeimosStateHandle {
    /// Hide the current view widget and show the given group
    pub fn set_view(self, group: Group) {
        tokio::spawn(
            async move {
                let mut active = self.active.lock().await;
                active.hide();
                *active = group;
                active.show();
            }
        );
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
    
    let overview = Overview::new(state.clone(), &window);
    window.add(&overview.group());
    let settings = Settings::new(state.clone(), &window);
    window.add(&settings.group());
    
    overview.group().show();

    let _ = state.0.set(
        DeimosState {
            ctx,
            active: Mutex::new(overview.group()),
            settings,
            overview,
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
