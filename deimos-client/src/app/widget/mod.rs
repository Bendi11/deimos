use fltk::{draw, enums::{Color, FrameType}};

use super::orbit;

pub mod svg;
pub mod button;
pub mod input;

pub fn orbit_scheme() {
    fltk::app::reload_scheme().ok();

    let (sr, sg, sb) = orbit::EARTH[1].to_rgb();
    fltk::app::set_selection_color(sr, sg, sb);
    fltk::app::set_frame_type(FrameType::RShadowBox);
    fltk::app::set_frame_type_cb(FrameType::RShadowBox, rshadow_box_cb, 2, 2, 2, 2);
    fltk::app::set_frame_type_cb(FrameType::RoundDownBox, rshadow_box_cb, 2, 2, 2, 2);
    fltk::app::set_visible_focus(false);
}

fn rshadow_box_cb(x: i32, y: i32, w: i32, h: i32, c: Color) {
    draw::set_draw_color(orbit::NIGHT[2]);
    draw::draw_rounded_rectf(x - 2, y - 2, w + 4, h + 4, 4);
    draw::set_draw_color(orbit::NIGHT[3]);
    draw::draw_rounded_rectf(x - 1, y - 1, w + 2, h + 2, 2);
    draw::set_draw_color(c);
    draw::draw_rounded_rectf(x, y, w, h, 2);
}
