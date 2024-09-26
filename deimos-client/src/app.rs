use deimos_shared::DeimosClient;
use iced::{
    widget::{Column, Container, Row, Rule, Text},
    Alignment, Application, Command, Element, Length, Pixels,
};
use settings::ApplicationSettings;
use tonic::transport::Channel;

pub mod settings;

pub struct DeimosApplication {
    api: DeimosClient<Channel>,
    settings: ApplicationSettings,
}

#[derive(Debug, Clone)]
pub enum DeimosMessage {}

impl DeimosApplication {
    const CONFIG_DIR_NAME: &str = "deimos";
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

        (Self { api, settings }, Command::none())
    }

    fn view(&self) -> Element<Self::Message> {
        Row::with_children([
            Container::new(Text::new(""))
                .width(Length::FillPortion(1))
                .into(),
            Rule::vertical(Pixels(3f32)).into(),
            Column::with_children([
                Row::with_children([Text::new("Connecting...").into()])
                    .align_items(Alignment::End)
                    .into(),
                Rule::horizontal(Pixels(3f32)).into(),
            ])
            .width(Length::FillPortion(4))
            .into(),
        ])
        .into()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        Command::none()
    }

    fn theme(&self) -> Self::Theme {
        Self::Theme::Dark
    }
}
