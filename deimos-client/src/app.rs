use std::{
    process::ExitCode,
    sync::{Arc, Weak},
};

use iced::{
    widget::{svg, Svg},
    Length, Task,
};
use loader::{LoadWrapper, LoaderMessage};
use settings::{Settings, SettingsMessage};
use sidebar::{Sidebar, SidebarMessage};
use style::{
    orbit, Button, Column, Container, Element, Row, Theme,
};

use crate::context::{container::CachedContainer, Context};

mod loader;
mod settings;
mod sidebar;
mod style;

#[derive(Debug)]
pub struct DeimosApplication {
    ctx: Arc<Context>,
    settings_icon: svg::Handle,
    sidebar: Sidebar,
    view: DeimosView,
}

#[derive(Debug, Clone)]
pub enum DeimosView {
    Empty,
    Settings(Settings),
    Server(Weak<CachedContainer>),
}

#[derive(Debug, Clone)]
pub enum DeimosMessage {
    BeginNavigateSettings,
    Navigate(DeimosView),
    Settings(SettingsMessage),
    Sidebar(SidebarMessage),
}

impl DeimosApplication {
    /// Load application state from a save file and return the application
    async fn load() -> Self {
        let ctx = Context::new().await;
        let view = DeimosView::Empty;

        let settings_icon = svg::Handle::from_memory(include_bytes!("../assets/settings.svg"));

        let sidebar = Sidebar::new(ctx.clone());

        Self {
            ctx,
            sidebar,
            settings_icon,
            view,
        }
    }

    pub fn run() -> ExitCode {
        match iced::application("Deimos", LoadWrapper::update, LoadWrapper::view)
            .antialiasing(true)
            .executor::<iced::executor::Default>()
            .theme(|_| Theme::default())
            .run_with(move || {
                (
                    LoadWrapper::new(),
                    Task::perform(Self::load(), LoaderMessage::Loaded),
                )
            }) {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                tracing::error!("Failed to run iced application: {e}");
                ExitCode::FAILURE
            }
        }
    }

    fn update(&mut self, msg: DeimosMessage) -> Task<DeimosMessage> {
        match msg {
            DeimosMessage::Navigate(view) => {
                let ctx = self.ctx.clone();
                match std::mem::replace(&mut self.view, view) {
                    DeimosView::Settings(s) => Task::future(async move {
                        ctx.reload_settings(s.edited).await;
                    })
                    .discard(),
                    _ => iced::Task::none(),
                }
            }
            DeimosMessage::Settings(msg) => {
                if let DeimosView::Settings(ref mut settings) = self.view {
                    settings.update(msg).map(DeimosMessage::Settings)
                } else {
                    ().into()
                }
            },
            DeimosMessage::Sidebar(m) => self.sidebar.update(m).map(DeimosMessage::Sidebar),
            DeimosMessage::BeginNavigateSettings => {
                let ctx = self.ctx.clone();
                Task::future(async move {
                    ctx.synchronize_containers().await;
                }).discard()
                /*

                Task::perform(async move { Settings::new(ctx.settings().await) }, |s| {
                    DeimosMessage::Navigate(DeimosView::Settings(s))
                })*/
            }
        }
    }

    fn empty_view(&self) -> Element<DeimosMessage> {
        Column::new()
            .push(
                Container::new(
                    Button::new(
                        Svg::new(self.settings_icon.clone())
                            .class((orbit::MERCURY[1], orbit::SOL[0]))
                            .width(Length::Shrink),
                    )
                    .on_press(DeimosMessage::BeginNavigateSettings),
                )
                .align_right(Length::Fill)
                .height(Length::Fixed(45f32)),
            )
            .width(Length::FillPortion(3))
            .into()
    }

    fn view(&self) -> Element<DeimosMessage> {
        Row::new()
            .push(self.sidebar.view().map(DeimosMessage::Sidebar))
            .push(match self.view {
                DeimosView::Empty => self.empty_view(),
                DeimosView::Settings(ref s) => s.view().map(DeimosMessage::Settings),
                _ => self.empty_view(),
            })
            .into()
    }
}

impl Drop for DeimosApplication {
    fn drop(&mut self) {
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            let ctx = self.ctx.clone();
            let settings = match &self.view {
                DeimosView::Settings(s) => Some(s.edited.clone()),
                _ => None,
            };

            rt.block_on(async move {
                if let Some(settings) = settings {
                    ctx.clone().reload_settings(settings).await;
                }

                ctx.cleanup().await;
            })
        }
        
    }
}
