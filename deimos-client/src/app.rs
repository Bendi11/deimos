use std::process::ExitCode;

use pview::{PodView, PodViewMessage};
use iced::{
    widget::{svg, Svg},
    Length, Task,
};
use settings::{Settings, SettingsMessage};
use sidebar::{Sidebar, SidebarMessage};
use style::{orbit, Button, Column, Container, Element, Row, Theme};

use crate::context::{Context, ContextMessage};

mod pview;
mod settings;
mod sidebar;
mod style;

#[derive(Debug)]
pub struct DeimosApplication {
    ctx: Context,
    settings_icon: svg::Handle,
    sidebar: Sidebar,
    settings: Settings,
    pod_view: PodView,
    view: DeimosView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeimosView {
    Empty,
    Settings,
    PodView,
}

#[derive(Debug, Clone)]
pub enum DeimosMessage {
    Close,
    Navigate(DeimosView),
    Context(ContextMessage),
    Settings(SettingsMessage),
    Sidebar(SidebarMessage),
    PodView(PodViewMessage),
}

impl DeimosApplication {
    /// Load application state from a save file and return the application
    fn load() -> Self {
        let ctx = Context::load();
        let view = DeimosView::Empty;

        let settings_icon = svg::Handle::from_memory(include_bytes!("../assets/settings.svg"));
        let sidebar = Sidebar::new();
        let settings = Settings::new();
        let container_view = PodView::new();

        Self {
            ctx,
            sidebar,
            settings,
            settings_icon,
            pod_view: container_view,
            view,
        }
    }
    
    /// Create iced application from the root element and run it to completion
    pub fn run() -> ExitCode {
        let this = Self::load();
        let task = this.ctx.post_load_init();

        match iced::application("Deimos", Self::update, Self::view)
            .antialiasing(true)
            .executor::<iced::executor::Default>()
            .theme(|_| Theme::default())
            .exit_on_close_request(false)
            .subscription(Self::subscription_window_event)
            .run_with(move || (this, task.map(DeimosMessage::Context)))
        {
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
            }
            DeimosMessage::Navigate(view) => {
                if view == self.view {
                    return iced::Task::none()
                }

                let task = match self.view {
                    DeimosView::Settings => self.ctx.reload_settings().map(DeimosMessage::Context),
                    DeimosView::PodView => self.pod_view.update(&mut self.ctx, PodViewMessage::Closed).map(DeimosMessage::PodView),
                    _ => iced::Task::none(),
                };

                self.view = view;
                task
            }
            DeimosMessage::Context(msg) => self.ctx.update(msg).map(DeimosMessage::Context),
            DeimosMessage::Settings(msg) => self
                .settings
                .update(&mut self.ctx, msg)
                .map(DeimosMessage::Settings),
            DeimosMessage::Sidebar(m) => match m {
                SidebarMessage::Refresh => self
                    .ctx
                    .synchronize_from_server()
                    .map(DeimosMessage::Context),
                SidebarMessage::UpdateContainer(container, state) => self
                    .ctx
                    .update_pod(container, state)
                    .map(DeimosMessage::Context),
                SidebarMessage::SelectContainer(container) => {
                    let task = self
                        .pod_view
                        .update(&mut self.ctx, PodViewMessage::ViewPod(container))
                        .map(DeimosMessage::PodView);
                    task.chain(iced::Task::done(DeimosMessage::Navigate(
                        DeimosView::PodView,
                    )))
                }
                other => self
                    .sidebar
                    .update(&mut self.ctx, other)
                    .map(DeimosMessage::Sidebar),
            },
            DeimosMessage::PodView(msg) => self
                .pod_view
                .update(&mut self.ctx, msg)
                .map(DeimosMessage::PodView),
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
        let pane = Column::new()
            .width(Length::FillPortion(3))
            .push(
                Container::new(
                    Button::new(
                        Svg::new(self.settings_icon.clone())
                            .class((orbit::MERCURY[1], orbit::SOL[0]))
                            .width(Length::Shrink)
                    )
                    .on_press(DeimosMessage::Navigate(DeimosView::Settings)),
                )
                .align_right(Length::Fill)
                .height(Length::Fixed(45f32))
            )
            .push(
                match self.view {
                    DeimosView::Settings => self.settings.view(&self.ctx).map(DeimosMessage::Settings),
                    DeimosView::PodView => {
                        self.pod_view.view(&self.ctx).map(DeimosMessage::PodView)
                    }
                    _ => self.empty_view(),
                }
            );

        Row::new()
            .push(self.sidebar.view(&self.ctx).map(DeimosMessage::Sidebar))
            .push(pane.width(Length::FillPortion(5)))
            .into()
    }

    fn subscription_window_event(&self) -> iced::Subscription<DeimosMessage> {
        iced::window::close_requests().map(|_| DeimosMessage::Close)
    }
}
