use std::{process::ExitCode, sync::{Arc, Weak}};

use config::LoadStateError;
use iced::{Length, Pixels, Task};
use loader::{LoaderMessage, LoadWrapper};
use settings::Settings;
use style::{Container, Element, Row, Rule, Text, Theme};

use crate::context::{container::CachedContainerInfo, Context, ContextState};

mod loader;
mod config;
mod sidebar;
mod settings;
pub mod style;

#[derive(Debug)]
pub struct DeimosApplication {
    ctx: Arc<Context>,
    settings: Settings,
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
}

impl DeimosApplication {
    pub const CONFIG_DIR_NAME: &str = "deimos";
    pub const CONFIG_FILE_NAME: &str = "state.json";
}

impl DeimosApplication {
    /// Load application state from a save file and return the application
    async fn load() -> Result<Self, DeimosApplicationLoadError>  {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let state = Self::load_config()?;
        let ctx = Arc::new(Context::new(state.context).await);

        let settings = Settings::new(ctx.clone());
        let view = DeimosView::Empty;

        Ok(
            Self {
                ctx,
                settings,
                view,
            }
        )
    }

    pub fn run() -> ExitCode {
        match iced::application(
                "Deimos",
                LoadWrapper::update,
                LoadWrapper::view
            )
            .executor::<iced::executor::Default>()
            .theme(|_| Theme::default())
            .run_with(move ||
                (
                    LoadWrapper::new(),
                    Task::perform(
                        Self::load(),
                        LoaderMessage::Loaded,
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
        }
    }

    fn view(&self) -> Element<DeimosMessage> {
        Row::new()
            .push(self.sidebar())
            .push(
                Rule::vertical(Pixels(3f32))
            )
            .push(
                Text::new("Main view")
                    .width(Length::FillPortion(3))
                    .height(Length::Fill)
            )
            .into()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeimosApplicationLoadError {
    #[error("Failed to load application state: {0}")]
    LoadState(#[from] LoadStateError),
}
