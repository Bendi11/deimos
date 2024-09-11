use std::sync::Arc;

use deimos_shared::{channel::DeimosServiceClient, status::ServerStatusRequest};
use iced::{widget::{button, column, text}, Application, Command, Element};
use tokio::sync::Mutex;
use tonic::transport::Channel;


pub struct DeimosApplication {
    status: Option<String>,
    grpc: Arc<Mutex<DeimosServiceClient<Channel>>>,
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
    type Flags = Channel;

    fn title(&self) -> String {
        "Deimos".to_owned()
    }

    fn new(flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            Self {
                status: None,
                grpc: Arc::new(Mutex::new(DeimosServiceClient::new(flags)))
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
        let grpc = self.grpc.clone();
        match message {
            DeimosMessage::RequestStatus => Command::perform(
                async move {
                    grpc.lock().await.server_status(ServerStatusRequest {}).await
                },
                |status| DeimosMessage::StatusRecv(format!("{:?}", status))
            ),
            DeimosMessage::StatusRecv(val) => {
                self.status = Some(val);
                Command::none()
            }
        }
    }
}
