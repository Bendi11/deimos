use std::{process::ExitCode, sync::{Arc, Weak}};

use iced::{alignment::Horizontal, border::Radius, widget::{container, svg, Space, Svg}, Background, Length, Padding, Pixels, Shadow, Task, Vector};
use loader::{LoaderMessage, LoadWrapper};
use settings::{Settings, SettingsMessage};
use style::{container::ContainerClass, orbit, svg::SvgClass, Column, Container, Element, Row, Rule, Text, Theme};

use crate::context::{container::CachedContainer, Context};

mod loader;
mod settings;
pub mod style;

#[derive(Debug)]
pub struct DeimosApplication {
    ctx: Arc<Context>,
    icon: svg::Handle,
    settings: Settings,
    view: DeimosView,
}

#[derive(Debug, Clone)]
pub enum DeimosView {
    Empty,
    Settings,
    Server(Weak<CachedContainer>),
}


#[derive(Debug)]
pub enum DeimosMessage {
    Navigate(DeimosView),
    Settings(SettingsMessage),
    ContainerUpdate,
}


impl DeimosApplication {
    /// Load application state from a save file and return the application
    async fn load() -> Self {
        let ctx = Context::new().await;

        let settings = Settings::new(ctx.clone());
        let view = DeimosView::Empty;
        
        let icon = svg::Handle::from_memory(include_bytes!("../assets/mars-deimos.svg"));

        Self {
            ctx,
            icon,
            settings,
            view,
        }
    }

    pub fn run() -> ExitCode {
        match iced::application(
                "Deimos",
                LoadWrapper::update,
                LoadWrapper::view
            )
            .antialiasing(true)
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
            DeimosMessage::ContainerUpdate => ().into(),
        }
    }

    fn empty_view(&self) -> Element<DeimosMessage> {
        Column::new()
            .push(
                Container::new(
                    self.settings.icon()
                        .map(DeimosMessage::Navigate)
                )
                    .align_right(Length::Fill)
                    .height(Length::Fixed(45f32))
            )
            .push(
                Text::new("Main view")
            )
            .width(Length::FillPortion(3))
            .into()

    }

    fn view(&self) -> Element<DeimosMessage> {
        let header = Row::new()
            .push(
                Svg::new(self.icon.clone())
                    .class(orbit::MARS[1])
                .height(64f32)
                .width(Length::FillPortion(1))
            )
            .push(
                Column::new()
                    .push(Text::new("Deimos")
                        .center()
                    )
                    .align_x(Horizontal::Center)
                    .width(Length::FillPortion(1))
            )
            .padding(Padding::default()
                .top(16f32)
            )
            .height(128);

        Row::new()
            .push(
                Container::new(
                    Column::new()
                        .push(header)
                ).class(ContainerClass {
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
                            blur_radius: 16f32
                        })
                })
                .height(Length::Fill)
            )
            .push(match self.view {
                DeimosView::Empty => self.empty_view(),
                DeimosView::Settings => self.settings.view().map(DeimosMessage::Settings),
                _ => self.empty_view()
            })
            .into()
    }
}
