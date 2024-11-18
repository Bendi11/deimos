use std::{ops::Deref, path::PathBuf, process::ExitCode, sync::Arc};

use fltk::{app::App, enums::{Align, Font}, group::Group, prelude::{GroupExt, WidgetExt}, window::Window};
use once_cell::sync::OnceCell;
use tokio::sync::Mutex;

use crate::context::Context;


pub mod orbit;
pub mod widget;
mod over;
mod auth;
mod settings;


pub struct DeimosState {
    ctx: Context,
    active: Mutex<Group>,
    settings: Group,
    overview: Group,
    authorization: Group,
}

#[derive(Clone, Default)]
pub struct DeimosStateHandle(Arc<OnceCell<DeimosState>>);

impl Deref for DeimosStateHandle {
    type Target = DeimosState;
    fn deref(&self) -> &Self::Target {
        self.0.wait()
    }
}

pub const HEADER_FONT: Font = Font::CourierBold;
pub const SUBTITLE_FONT: Font = Font::Helvetica;
pub const GENERAL_FONT: Font = Font::Courier;

impl DeimosStateHandle {
    /// Hide the current view widget and show the given group
    pub async fn set_view(&self, group: Group) {
        fltk::app::lock().ok();

        let mut active = self.active.lock().await;
        active.hide();
        *active = group;
        active.show();

        fltk::app::unlock();
        fltk::app::awake();
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
    window.set_align(Align::Inside | Align::Center);

    let state = DeimosStateHandle::default();
    
    let mut overview = over::overview(state.clone());
    window.resizable(&overview);
    let settings = settings::settings(state.clone());
    window.resizable(&settings);
    let authorization = auth::authorization(state.clone());
    window.resizable(&authorization);

    window.end();
    window.show();
    
    overview.show();

    let _ = state.0.set(
        DeimosState {
            ctx,
            active: Mutex::new(overview.clone()),
            settings,
            overview,
            authorization,
        }
    );

    window.redraw();

    state.ctx.init().await;
    
    let ctx_loop = {
        let state = state.clone();
        tokio::task::spawn(async move {
            state.ctx.pod_event_loop().await;
        })
    };

    match fltk_ev.run() {
        Ok(()) => {
            ctx_loop.abort();
            state.ctx.save();
            ExitCode::SUCCESS
        },
        Err(e) => {
            tracing::error!("Failed to run FLTK event loop: {}", e);
            ExitCode::FAILURE
        }
    }
}
