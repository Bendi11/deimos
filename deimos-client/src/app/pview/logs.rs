use arraydeque::ArrayDeque;
use iced::{widget::{text::Span, Scrollable}, Length};

use crate::{app::style::{Button, Column, Element, Rich}, context::{Context, PodRef}};


#[derive(Debug, Clone, Default)]
pub struct PodLogsView {
    spool: Box<ArrayDeque<u8, 0x10000, arraydeque::behavior::Wrapping>>,
    stream: Option<iced::task::Handle>,
}

#[derive(Debug, Clone)]
pub enum PodLogsMessage {
    Subscribe,
    Chunk(Vec<u8>),
}

impl PodLogsView {
    pub fn view(&self, _ctx: &Context) -> Element<PodLogsMessage> {
        let (log1, log2) = self.spool.as_slices();

        let chunks = log1
            .utf8_chunks()
            .chain(log2.utf8_chunks())
            .map(|chunk| Span::new(chunk.valid()).font(iced::Font::MONOSPACE))
            .collect::<Vec<_>>();
        
        Column::new()
            .push(
                Scrollable::new(
                    Rich::with_spans(chunks)
                )
                .width(Length::Fill)
                .height(Length::FillPortion(9))
                .anchor_bottom()
            )
            .push(
                Button::new("Subscribe")
                    .on_press(PodLogsMessage::Subscribe)
                    .width(Length::Fill)
                    .height(Length::FillPortion(1))
            )
            .into()
    }
    
    pub fn update(&mut self, ctx: &mut Context, viewed: PodRef, msg: PodLogsMessage) -> iced::Task<PodLogsMessage> {
        match msg {
            PodLogsMessage::Chunk(bytes) => {
                self.spool.extend_back(bytes);
                iced::Task::none()
            },
            PodLogsMessage::Subscribe => {
                self.stream = None;
                let (task, stream) = ctx.pod_logs(viewed).abortable();
                let stream = stream.abort_on_drop();
                self.stream = Some(stream);
                task.map(PodLogsMessage::Chunk)
            }
        }
    }
}
