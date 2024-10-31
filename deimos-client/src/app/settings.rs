use fltk::{frame::Frame, group::{Pack, PackType}, input::Input, prelude::{GroupExt, InputExt, WidgetExt}};

use super::orbit;


pub struct Settings {
    column: Pack,
    host_url: Input,
}

impl Settings {
    pub fn new<P: GroupExt>(parent: &mut P) -> Self {
        let mut column = Pack::default()
            .with_type(PackType::Vertical)
            .size_of(parent);
        column.end();
        column.set_color(orbit::NIGHT[2]);
        parent.add(&column);
       
        let mut frame = Frame::default()
            .with_size(column.width(), column.height() / 6);
        frame.set_label_color(orbit::SOL[0]);
        frame.set_label("TEST");
        column.add(&frame);

        let mut host_url = Input::default()
            .with_size(column.width(), column.height() / 6);

        host_url.set_frame(fltk::enums::FrameType::FlatBox);
        host_url.set_text_color(orbit::MERCURY[1]);
        host_url.set_label("Host URL");
        host_url.set_cursor_color(orbit::SOL[0]);
        host_url.set_color(orbit::NIGHT[1]);
        host_url.set_label_color(orbit::SOL[0]);

        column.add(&host_url);

        Self {
            column,
            host_url,
        }
    }

    pub const fn group(&self) -> &impl GroupExt {
        &self.column
    }

    pub fn group_mut(&mut self) -> &mut impl GroupExt {
        &mut self.column
    }
}
