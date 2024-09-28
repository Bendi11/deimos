use std::{process::ExitCode, sync::{Arc, Weak}};

use iced::{widget::Text, Task};
use style::{Element, Theme};

use crate::context::{container::CachedContainerInfo, Context, ContextState};

mod config;
pub mod style;

pub struct DeimosApplication {
    state: DeimosApplicationState,
    ctx: Arc<Context>,
    view: DeimosView,
}

/// Persistent state maintained for the whole application
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct DeimosApplicationState {
    pub context: ContextState,
}

#[derive(Debug, Clone)]
pub enum DeimosView {
    Empty,
    Settings,
    Server(Weak<CachedContainerInfo>),
}

#[derive(Debug, Clone)]
pub enum DeimosMessage {}

impl DeimosApplication {
    pub const CONFIG_DIR_NAME: &str = "deimos";
    pub const CONFIG_FILE_NAME: &str = "settings.json";
}

impl DeimosApplication {
    pub fn run() -> ExitCode {
        let state = match Self::load_config() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("{e}");
                return ExitCode::FAILURE
            }
        };

        let ctx = Arc::new(Context::new(&state.context));

        match iced::application(
                "Deimos",
                Self::update,
                Self::view
            )
            .theme(|_| Theme::default())
            .run_with(move || (Self { ctx, state, view: DeimosView::Empty }, ().into())) {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                tracing::error!("Failed to run iced application: {e}");
                ExitCode::FAILURE
            },
        }
    }

    fn update(&mut self, msg: DeimosMessage) -> Task<DeimosMessage> {
        ().into()
    }

    fn view(&self) -> Element<DeimosMessage> {
        Text::new("Test")
            .into()
    }
}
