use std::{
    process::ExitCode,
    sync::{Arc, Weak},
};

use iced::{
    widget::{svg, Svg},
    Length, Task,
};
use loader::{LoadWrapper, LoaderMessage};
use settings::{Settings, SettingsMessage, SettingsMessageInternal};
use sidebar::{Sidebar, SidebarEntry, SidebarMessage};
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
    settings: Settings,
    view: DeimosView,
}

#[derive(Debug, Clone)]
pub enum DeimosView {
    Empty,
    Settings,
    Server,
}

#[derive(Debug, Clone)]
pub enum DeimosMessage {
    NavigateSettings,
    Refreshed(Vec<Arc<CachedContainer>>),
    Settings(SettingsMessage),
    Sidebar(SidebarMessage),
}

impl DeimosApplication {
    /// Load application state from a save file and return the application
    async fn load() -> Self {
        let ctx = Context::new().await;
        let view = DeimosView::Empty;

        let settings_icon = svg::Handle::from_memory(include_bytes!("../assets/settings.svg"));
        let sidebar = Sidebar::new();
        let settings = Settings::new();

        Self {
            ctx,
            sidebar,
            settings,
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
            DeimosMessage::Refreshed(data) => {
                let entries = data.iter().map(|c| SidebarEntry { name: Arc::from(c.data.name.clone()), running: false} ).collect();
                self.sidebar.update(SidebarMessage::ContainerEntries(entries))
                    .map(DeimosMessage::Sidebar)
            },
            DeimosMessage::NavigateSettings => {
                self.view = DeimosView::Settings;

                let ctx = self.ctx.clone();
                Task::perform(
                    async move {
                        ctx.settings().await
                    },
                    |s| DeimosMessage::Settings(SettingsMessage::Enter(s)))
            },
            DeimosMessage::Settings(msg) => self.settings.update(msg).map(DeimosMessage::Settings),
            DeimosMessage::Sidebar(m) => match m {
                SidebarMessage::Refresh => {
                    let ctx = self.ctx.clone();
                    Task::perform(
                        async move { ctx.containers().await },
                        DeimosMessage::Refreshed
                    )
                }
                other => self.sidebar.update(other).map(DeimosMessage::Sidebar),
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
                    .on_press(DeimosMessage::NavigateSettings),
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
                DeimosView::Settings => self.settings.view().map(DeimosMessage::Settings),
                _ => self.empty_view(),
            })
            .into()
    }
}

impl Drop for DeimosApplication {
    fn drop(&mut self) {
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            let ctx = self.ctx.clone();
            let settings = self.settings.edited();

            rt.block_on(async move {
                if let Some(settings) = settings {
                    ctx.clone().reload_settings(settings).await;
                }

                ctx.cleanup().await;
            })
        }
        
    }
}
