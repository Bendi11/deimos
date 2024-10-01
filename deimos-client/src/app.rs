use std::{process::ExitCode, sync::{Arc, Weak}};

use config::LoadStateError;
use iced::{alignment::Horizontal, widget::{container, svg, Space, Svg}, Length, Padding, Pixels, Size, Task};
use loader::{LoaderMessage, LoadWrapper};
use settings::{Settings, SettingsMessage};
use style::{Column, Container, Element, Row, Rule, Text, Theme};

use crate::context::{container::CachedContainerInfo, Context, ContextState};

mod loader;
mod config;
mod settings;
pub mod style;

#[derive(Debug)]
pub struct DeimosApplication {
    ctx: Arc<Context>,
    icon: svg::Handle,
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


#[derive(Debug)]
pub enum DeimosMessage {
    Navigate(DeimosView),
    Settings(SettingsMessage),
}

impl DeimosApplication {
    pub const CONFIG_DIR_NAME: &str = "deimos";
    pub const CONFIG_FILE_NAME: &str = "state.json";
}

impl DeimosApplication {
    /// Load application state from a save file and return the application
    async fn load() -> Result<Self, DeimosApplicationLoadError>  {
        let state = Self::load_config()?;
        let ctx = Arc::new(Context::new(state.context).await);

        let settings = Settings::new(ctx.clone());
        let view = DeimosView::Empty;
        
        let icon = svg::Handle::from_memory(include_bytes!("../assets/mars-deimos.svg"));

        Ok(
            Self {
                ctx,
                icon,
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
            DeimosMessage::Navigate(view) => {
                self.view = view;
                iced::Task::none()
            },
            DeimosMessage::Settings(msg) => self.settings.update(msg).map(DeimosMessage::Settings),
        }
    }

    fn view(&self) -> Element<DeimosMessage> {
        Row::new()
            .push(
                Column::new()
                    .push(
                        Row::new()
                            .push(Space::new(50f32, 0f32))
                            .push(Svg::new(self.icon.clone())
                                .class(style::orbit::MARS[1])
                                .width(Length::Shrink)
                            )
                            .push(Text::new("Deimos")
                                .size(32f32)
                                .center()
                            )
                            .height(Length::Fixed(64f32))
                    )
                    .push(Rule::horizontal(Pixels(2f32)))
            )
            .push(
                Rule::vertical(Pixels(3f32))
            )
            .push(
                Column::new()
                    .push(
                        Container::new(
                            self.settings.icon()
                                .map(DeimosMessage::Navigate)
                        )
                            .class(<Theme as container::Catalog>::Class::Invisible)
                            .align_right(Length::Fill)
                            .height(Length::Fixed(45f32))
                    )
                    .push(
                        Text::new("Main view")
                    )
                    .width(Length::FillPortion(3))
            )
            .into()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeimosApplicationLoadError {
    #[error("Failed to load application state: {0}")]
    LoadState(#[from] LoadStateError),
}
