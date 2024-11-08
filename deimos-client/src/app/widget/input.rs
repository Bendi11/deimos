use fltk::{enums::{Align, CallbackTrigger, Font}, frame::Frame, group::Flex, prelude::{GroupExt, InputExt, WidgetBase, WidgetExt}};

use crate::app::orbit;



pub fn input_lbl(label: &str) -> Frame {
    let mut host_lbl = Frame::default();
    host_lbl.set_align(Align::Inside | Align::Left);
    host_lbl.set_label_color(orbit::SOL[0]);
    host_lbl.set_label(label);
    host_lbl.set_label_size(18);
    host_lbl
}

pub fn input_box<I: InputExt + Default>(label: &str) -> (impl GroupExt, I) {
    let mut frame = Flex::default_fill().column();
    let label = input_lbl(label);
    frame.fixed(&label, 20);

    let mut input = I::default();
    frame.fixed(&input, 40);
    input.set_frame(fltk::enums::FrameType::RShadowBox);
    input.set_text_color(orbit::MERCURY[1]);
    input.set_text_font(Font::Courier);
    input.set_text_size(18);
    input.set_cursor_color(orbit::SOL[0]);
    input.set_color(orbit::NIGHT[1]);
    input.set_label_color(orbit::SOL[0]);
    input.set_trigger(CallbackTrigger::Changed);
    input.set_callback(|i| {
        if i.text_color() != orbit::MERCURY[1] {
            i.set_text_color(orbit::MERCURY[1]);
            i.redraw();
        }
    });
    
    frame.end();

    (frame, input)
}
