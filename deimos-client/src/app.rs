use std::sync::{Arc, Weak};

use container::CachedContainerInfo;
use deimos_shared::DeimosClient;
use iced::{
    alignment::Horizontal, widget::scrollable, Alignment, Application, Command, Length, Pixels
};
use settings::ApplicationSettings;
use style::{Button, Column, Element, Row, Rule, Scrollable, Text, Theme};
use tonic::transport::Channel;

pub mod container;
pub mod settings;
pub mod style;

pub struct DeimosApplication {
    api: DeimosClient<Channel>,
    view: DeimosView,
    containers: Vec<Arc<CachedContainerInfo>>,
    settings: ApplicationSettings,
}

#[derive(Debug, Clone,)]
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
            Scrollable::new(container_list)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Properties::new().width(Pixels(6f32)),
                ))
                .into(),
        ])
        .width(Length::FillPortion(1))
        .into()
    }
}

impl Application for DeimosApplication {
    type Message = DeimosMessage;
    type Executor = iced::executor::Default;
    type Theme = Theme;
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
            self.containerlist(),
            Rule::vertical(Pixels(3f32)).into(),
            Column::with_children([
                Row::with_children([Text::new("Connecting...")
                    .horizontal_alignment(Horizontal::Right)
                    .into()])
                .into(),
                Rule::horizontal(Pixels(3f32)).into(),
            ])
            .width(Length::FillPortion(3))
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
        Theme::default()
    }
}
