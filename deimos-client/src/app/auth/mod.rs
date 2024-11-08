use fltk::{enums::{Align, Font, FrameType}, frame::Frame, group::{Flex, Group}, image::SvgImage, prelude::*};

use super::{orbit, widget, DeimosStateHandle};



pub fn authorization(state: DeimosStateHandle) -> Group {
    let mut top = Group::default_fill();
    top.hide();
    let mut column = Flex::default_fill().column();
    column.set_margins(16, 0, 16, 0);

    let header = header(state.clone());
    column.fixed(&header, 64);

    token_box(state);
    

    column.end();
    top.end();
    top
}

fn token_box(state: DeimosStateHandle) -> Flex {
    let mut container = Flex::default_fill().column();
    container.set_frame(FrameType::RShadowBox);
    container.set_color(orbit::NIGHT[1]);

    let mut title = Frame::default();
    title.set_label_size(20);
    title.set_label_color(orbit::SOL[0]);
    title.set_label_font(Font::Courier);
    title.set_label("Token");
    container.fixed(&title, 22);
    
    {
        let mut container = container.clone();
        tokio::task::spawn(async move {
            let mut sub = state.ctx.clients.persistent.token.subscribe();
            loop {
                {
                    let token = sub.borrow();
                    if let Some(ref token) = *token {
                        title.set_label(token.user());
                    } else {
                        container.hide();
                    }
                }

                if sub.changed().await.is_err() {
                    break
                }
            }
        });
    }

    container.end();
    container
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

    let mut title = Frame::default();
    title.set_label_color(orbit::SOL[0]);
    title.set_label_font(Font::CourierBold);
    title.set_label("Token Management");

    row.resize_callback(move |r,_,_,_,_| {
        r.fixed(&back_button, r.height());
        title.set_label_size(r.height() / 2);
    });



    row.end();
    row
}
