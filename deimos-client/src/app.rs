use iced::{widget::column, Application, Command, Element};


pub struct DeimosApplication {

}

#[derive(Debug)]
pub enum DeimosMessage {

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
        (Self {}, Command::none())
    }

    fn view(&self) -> Element<Self::Message> {
        column(None).into()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        Command::none()
    }
}
