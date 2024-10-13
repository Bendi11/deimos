use std::process::ExitCode;

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

use crate::context::{Context, ContextMessage};

mod loader;
mod settings;
mod sidebar;
mod style;

#[derive(Debug)]
pub struct DeimosApplication {
    ctx: Context,
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
    Close,
    Navigate(DeimosView),
    Context(ContextMessage),
    Settings(SettingsMessage),
    Sidebar(SidebarMessage),
}

impl DeimosApplication {
    /// Load application state from a save file and return the application
    async fn load() -> Self {
        let ctx = Context::load().await;
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
            .exit_on_close_request(false)
            .subscription(Self::subscription_window_event)
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
            DeimosMessage::Close => {
                self.ctx.save();
                iced::exit()
            },
            DeimosMessage::Navigate(view) => {
                let task = match self.view {
                    DeimosView::Settings => self.ctx.reload_settings().map(DeimosMessage::Context),
                    _ => iced::Task::none(),
                };

                self.view = view;
                task
            },
            DeimosMessage::Context(msg) => self.ctx.update(msg).map(DeimosMessage::Context),
            DeimosMessage::Settings(msg) => self.settings.update(&mut self.ctx, msg).map(DeimosMessage::Settings),
            DeimosMessage::Sidebar(m) => match m {
                SidebarMessage::Refresh => self.ctx.synchronize_from_server().map(DeimosMessage::Context),
                SidebarMessage::ContainerOn(container) => self.ctx.update_container(container, true).map(DeimosMessage::Context),
                SidebarMessage::ContainerOff(container) => self.ctx.update_container(container, false).map(DeimosMessage::Context),
                other => self.sidebar.update(&mut self.ctx, other).map(DeimosMessage::Sidebar),
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
                    .on_press(DeimosMessage::Navigate(DeimosView::Settings)),
                )
                .align_right(Length::Fill)
                .height(Length::Fixed(45f32)),
            )
            .width(Length::FillPortion(3))
            .into()
    }

    fn view(&self) -> Element<DeimosMessage> {
        Row::new()
            .push(self.sidebar.view(&self.ctx).map(DeimosMessage::Sidebar))
            .push(match self.view {
                DeimosView::Settings => self.settings.view(&self.ctx).map(DeimosMessage::Settings),
                _ => self.empty_view(),
            })
            .into()
    }

    fn subscription_window_event(_: &LoadWrapper) -> iced::Subscription<LoaderMessage> {
        iced::window::close_requests().map(|_| LoaderMessage::Application(DeimosMessage::Close))
    }
}
