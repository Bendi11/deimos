use std::sync::{Arc, Weak};

use iced::{alignment::Horizontal, widget::scrollable, Application, Command, Length, Pixels};
use style::{Button, Column, Element, Row, Rule, Scrollable, Text, Theme};

use crate::context::{container::CachedContainerInfo, Context, ContextState};

pub mod style;

pub struct DeimosApplication {
    state: DeimosApplicationState,
    ctx: Arc<Context>,
    view: DeimosView,
}

/// Persistent state maintained for the whole application
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct DeimosApplicationState {
    pub context: ContextState,
}

#[derive(Debug, Clone)]
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
        let containers = self.ctx.containers().map(|c| {
            Button::new(Text::new(c.name.clone()))
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

    pub async fn refresh_cache(&self) {}
}

impl Application for DeimosApplication {
    type Message = DeimosMessage;
    type Executor = iced::executor::Default;
    type Theme = Theme;
    type Flags = DeimosApplicationState;

    fn title(&self) -> String {
        "Deimos".to_owned()
    }

    fn new(state: Self::Flags) -> (Self, Command<Self::Message>) {
        let ctx = Arc::new(Context::new(&state.context));
        let view = DeimosView::Empty;

        (Self { ctx, state, view }, Command::none())
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
