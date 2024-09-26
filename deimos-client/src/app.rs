use std::{path::PathBuf, str::FromStr};

use iced::{Application, Command, Element};
use settings::ApplicationSettings;
use tonic::transport::Channel;
use deimos_shared::DeimosClient;

mod settings;

pub struct DeimosApplication {
    api: DeimosClient<Channel>,
    settings: ApplicationSettings,
}

#[derive(Debug, Clone)]
pub enum DeimosMessage {

}


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

    fn new(flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let channel = Channel::builder(flags.conn.server_uri);
        let api = DeimosClient::new(channel);

        (
            Self {
                api
            },
            Command::none()
        )
    }

    fn view(&self) -> Element<Self::Message> {
        
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        Command::none()
    }
}
