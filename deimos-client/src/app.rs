use std::sync::Weak;

use deimos_shared::DeimosClient;
use iced::{
    alignment::Horizontal,
    widget::{
        rule, scrollable, text::LineHeight, Button, Column, Container, Row, Rule, Scrollable, Text,
    },
    Alignment, Application, Command, Element, Length, Pixels,
};
use server::CachedContainerInfo;
use settings::ApplicationSettings;
use tonic::transport::Channel;

pub mod container;
pub mod settings;

pub struct DeimosApplication {
    api: DeimosClient<Channel>,
    view: DeimosView,
    containers: Vec<Arc<CachedContainerInfo>>,
    settings: ApplicationSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeimosView {
    Empty,
    Settings,
    Server(Weak<CachedContainerInfo>),
}

#[derive(Debug, Clone)]
pub enum DeimosMessage {
    Navigate(DeimosView),
}

impl DeimosApplication {
    pub const CONFIG_DIR_NAME: &str = "deimos";
    pub const CONFIG_FILE_NAME: &str = "settings.json";

    /// Get a list overview of all containers informed from the server
    fn containerlist(&self) -> Element<DeimosMessage> {
        let containers = self.containers.iter().map(|c| {
            Button::new(Text::new(&c.name))
                .height(Length::FillPortion(1))
                .into()
        });

        let container_list = Column::with_children(containers);

        Column::with_children([
            Text::new("Deimos")
                .width(Length::Fill)
                .horizontal_alignment(Horizontal::Center)
                .into(),
            Rule::horizontal(Pixels(3f32)).into(),
            Scrollable::new(container_list)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Properties::new().width(Pixels(6f32)),
                ))
                .into(),
        ])
        .into()
    }
}

impl Application for DeimosApplication {
    type Message = DeimosMessage;
    type Executor = iced::executor::Default;
    type Theme = iced::Theme;
    type Flags = ApplicationSettings;

    fn title(&self) -> String {
        "Deimos".to_owned()
    }

    fn new(settings: Self::Flags) -> (Self, Command<Self::Message>) {
        let channel = Channel::builder(settings.conn.server_uri.clone()).connect_lazy();
        let api = DeimosClient::new(channel);

        let view = DeimosView::Empty;

        (
            Self {
                api,
                settings,
                view,
                containers: Vec::new(),
            },
            Command::none(),
        )
    }

    fn view(&self) -> Element<Self::Message> {
        Row::with_children([
            self.containerlist().width(Length::FillPortion(1)).into(),
            Rule::vertical(Pixels(3f32)).into(),
            Column::with_children([
                Row::with_children([Text::new("Connecting...").horizontal_alignment(Horizontal::Right).into()])
                    .into(),
                Rule::horizontal(Pixels(3f32)).into(),
            ])
            .width(Length::FillPortion(4))
            .into(),
        ])
        .into()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        tracing::trace!("Got message {:#?}", message);

        match message {
            DeimosMessage::Navigate(to) => {
                self.view = to;
                Command::none()
            }
        }
    }

    fn theme(&self) -> Self::Theme {
        Self::Theme::Dark
    }
}
