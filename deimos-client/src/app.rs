use std::{
    process::ExitCode,
    sync::{Arc, Weak},
};

use iced::{
    alignment::Horizontal,
    border::Radius,
    futures::FutureExt,
    widget::{svg, Svg},
    Background, Length, Padding, Shadow, Task, Vector,
};
use loader::{LoadWrapper, LoaderMessage};
use settings::{Settings, SettingsMessage};
use style::{
    container::ContainerClass, orbit, Button, Column, Container, Element, Row, Text, Theme,
};

use crate::context::{container::CachedContainer, Context};

mod loader;
mod settings;
pub mod style;

#[derive(Debug)]
pub struct DeimosApplication {
    ctx: Arc<Context>,
    icon: svg::Handle,
    settings_icon: svg::Handle,
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
    ContainerUpdate,
}

impl DeimosApplication {
    /// Load application state from a save file and return the application
    async fn load() -> Self {
        let ctx = Context::new().await;
        let view = DeimosView::Empty;

        let icon = svg::Handle::from_memory(include_bytes!("../assets/mars-deimos.svg"));
        let settings_icon = svg::Handle::from_memory(include_bytes!("../assets/settings.svg"));

        Self {
            ctx,
            icon,
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
            }
            DeimosMessage::ContainerUpdate => ().into(),
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
            .push(Text::new("Main view"))
            .width(Length::FillPortion(3))
            .into()
    }

    fn view(&self) -> Element<DeimosMessage> {
        let header = Row::new()
            .push(
                Svg::new(self.icon.clone())
                    .class(orbit::MARS[1])
                    .height(64f32)
                    .width(Length::FillPortion(1)),
            )
            .push(
                Column::new()
                    .push(
                        Text::new("Deimos")
                            .size(30f32)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .center(),
                    )
                    .align_x(Horizontal::Center)
                    .width(Length::FillPortion(1)),
            )
            .padding(Padding::default().top(16f32).left(16f32).right(16f32))
            .height(128);

        Row::new()
            .push(
                Container::new(Column::new().push(header))
                    .class(ContainerClass {
                        radius: Radius {
                            top_left: 0f32,
                            top_right: 5f32,
                            bottom_right: 5f32,
                            bottom_left: 0f32,
                        },
                        background: Some(Background::Color(orbit::NIGHT[1])),
                        shadow: Some(Shadow {
                            color: orbit::NIGHT[3],
                            offset: Vector::new(1f32, 0f32),
                            blur_radius: 16f32,
                        }),
                    })
                    .width(Length::Fixed(256f32))
                    .height(Length::Fill),
            )
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
        if let DeimosView::Settings(ref s) = self.view {
            let ctx = self.ctx.clone();
            let settings = s.edited.clone();
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async move {
                    ctx.reload_settings(settings).await;
                })
            }
        }
    }
}
