use iced::{widget::container, Length, Task};

use super::{style::{Container, Element, Theme}, DeimosApplication, DeimosMessage};



/// Wrapper to create the application context before launching the main application
pub struct LoadWrapper {
    state: Option<DeimosApplication>,
}

#[derive(Debug)]
pub enum LoaderMessage {
    Loaded(DeimosApplication),
    Application(DeimosMessage),
}

impl LoadWrapper {
    /// Create a new LoadWrapper with no loaded application
    pub fn new() -> Self {
        Self {
            state: None,
        }
    }

    pub fn view(&self) -> Element<LoaderMessage> {
        match self.state {
            Some(ref app) => app.view().map(Into::into),
            None => Container::new(
                iced_aw::Spinner::new()
                    .width(Length::Fixed(30f32))
                    .height(Length::Fixed(30f32))
            )
                .center(Length::Fill)
                .into()
        }
    }

    pub fn update(&mut self, msg: LoaderMessage) -> Task<LoaderMessage> {
        match self.state {
            Some(ref mut app) => match msg {
                LoaderMessage::Application(msg) => app
                    .update(msg)
                    .map(Into::into),
                _ => Task::none(),
            },
            None => {
                if let LoaderMessage::Loaded(app) = msg {
                    self.state = Some(app);
                }

                Task::none()
            }
        }
    }
}

impl From<DeimosMessage> for LoaderMessage {
    fn from(value: DeimosMessage) -> Self {
        Self::Application(value)
    }
}
