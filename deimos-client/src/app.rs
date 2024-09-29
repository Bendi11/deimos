use std::{process::ExitCode, sync::{Arc, Weak}};

use iced::{widget::Text, Task};
use style::{Element, Theme};

use crate::context::{container::CachedContainerInfo, Context, ContextState};

mod config;
pub mod style;

pub struct DeimosApplication {
    ctx: Option<Arc<Context>>,
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
pub enum DeimosMessage {
    ApplicationInit(Arc<Context>),
}

impl DeimosApplication {
    pub const CONFIG_DIR_NAME: &str = "deimos";
    pub const CONFIG_FILE_NAME: &str = "state.json";
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

        match iced::application(
                "Deimos",
                Self::update,
                Self::view
            )
            .executor::<iced::executor::Default>()
            .theme(|_| Theme::default())
            .run_with(move ||
                (
                    Self {
                        ctx: None,
                        view: DeimosView::Empty
                    },
                    Task::perform(
                        async { Context::new(state.context).await },
                        |ctx| DeimosMessage::ApplicationInit(Arc::new(ctx)),
                    )
                )
            ) {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                tracing::error!("Failed to run iced application: {e}");
                ExitCode::FAILURE
            },
        }
    }

    fn update(&mut self, msg: DeimosMessage) -> Task<DeimosMessage> {
        match msg {
            DeimosMessage::ApplicationInit(ctx) => {
                self.ctx = Some(ctx);
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<DeimosMessage> {
        Text::new("Test")
            .into()
    }
}
