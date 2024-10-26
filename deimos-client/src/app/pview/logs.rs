use std::borrow::Cow;

use arraydeque::ArrayDeque;
use iced::{widget::{text::Span, Scrollable}, Length};

use crate::{app::style::{Element, Rich}, context::{Context, PodRef}};


#[derive(Debug, Clone)]
pub struct PodLogsView {
    spool: ArrayDeque<u8, 0x10000, arraydeque::behavior::Wrapping>,
    stream: Option<iced::task::Handle>,
}

#[derive(Debug, Clone)]
pub enum PodLogsMessage {
    Open,
    Close,
    Chunk(Vec<u8>),
}

impl PodLogsView {
    pub fn view(&self, _ctx: &Context) -> Element<PodLogsMessage> {
        let (log1, log2) = self.spool.as_slices();

        let chunks = log1
            .utf8_chunks()
            .chain(log2.utf8_chunks())
            .map(|chunk| Span::new(chunk.valid()))
            .collect::<Vec<_>>();

        Scrollable::new(
            Rich::with_spans(chunks)
        )
        .anchor_bottom()
        .width(Length::Fill)
        .into()
    }

    pub fn new() -> Self {
        Self {
            spool: Default::default(),
            stream: None,
        }
    }
    
    pub fn update(&mut self, ctx: &mut Context, viewed: PodRef, msg: PodLogsMessage) -> iced::Task<PodLogsMessage> {
        match msg {
            PodLogsMessage::Chunk(bytes) => {
                self.spool.extend_back(bytes);
                iced::Task::none()
            },
            PodLogsMessage::Close => {
                self.stream = None;
                iced::Task::none()
            },
            PodLogsMessage::Open => {
                let (task, stream) = ctx.pod_logs(viewed).abortable();
                let stream = stream.abort_on_drop();
                self.stream = Some(stream);
                task.map(PodLogsMessage::Chunk)
            }
        }
    }
}
