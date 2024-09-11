use iced::{widget::{button, column, text}, Application, Command, Element};


pub struct DeimosApplication {
    status: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DeimosMessage {
    RequestStatus,
    StatusRecv(String),
}

impl Application for DeimosApplication {
    type Message = DeimosMessage;
    type Executor = iced::executor::Default;
    type Theme = iced::Theme;
    type Flags = ();

    fn title(&self) -> String {
        "Deimos".to_owned()
    }

    fn new(flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            Self {
                status: None,
            },
            Command::none()
        )
    }

    fn view(&self) -> Element<Self::Message> {
        column![
            button(text(format!("{:?}", self.status)))
                .on_press(DeimosMessage::RequestStatus)
        ].into()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        Command::none()
    }
}
