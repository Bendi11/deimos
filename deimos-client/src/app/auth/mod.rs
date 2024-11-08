use fltk::{enums::Align, group::{Flex, Group}, image::SvgImage, prelude::*};

use super::{orbit, widget, DeimosStateHandle};



pub fn authorization(state: DeimosStateHandle) -> Group {
    let mut top = Group::default_fill();
    top.hide();
    let mut column = Flex::default_fill().column();

    let header = header(state);
    column.fixed(&header, 64);

    column.end();
    top.end();
    top
}

fn header(state: DeimosStateHandle) -> Flex {
    let mut row = Flex::default_fill()
            .row()
            .with_align(Align::Center);
    row.set_margins(8, 8, 0, 8);

    let back_svg = SvgImage::from_data(include_str!("../../../assets/close.svg")).unwrap();
    let back_rgb = widget::svg::svg_color(back_svg, 128, orbit::MERCURY[1]);
    let mut back_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
    back_button.set_image(Some(back_rgb));
    back_button.resize_callback(widget::svg::resize_image_cb(0, 0));
    back_button.set_callback(move |_| {
        let state = state.clone();
        tokio::task::spawn(async move {
            state.set_view(state.overview.clone()).await;
        });
    });

    row.fixed(&back_button, row.height());
    row.resize_callback(move |r,_,_,_,_| {
        r.fixed(&back_button, r.height());
    });

    row.end();
    row
}
