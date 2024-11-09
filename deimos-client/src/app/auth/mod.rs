use fltk::{enums::{Align, Font, FrameType}, frame::Frame, group::{Flex, Group, Pack, PackType}, image::SvgImage, input::Input, prelude::*};

use super::{orbit, widget, DeimosStateHandle};



pub fn authorization(state: DeimosStateHandle) -> Group {
    let mut top = Pack::default_fill();
    top.set_color(orbit::NIGHT[2]);
    top.set_type(PackType::Vertical);
    top.set_size(top.width() - 8, top.height());
    top.set_pos(8, 0);
    top.hide();

    header(state.clone()).with_size(top.width(), 42);
    token_box(state.clone()).with_size(top.width(), 240);
    
    let (frame, username) = widget::input::input_box::<Input>("Requested Token Username");
    let mut request_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
    request_button.set_size(top.width(), 40);
    request_button.set_label("Request Token");
    request_button.set_label_color(orbit::MERCURY[0]);
    
    request_button.set_callback(move |_| {
        let state = state.clone();
        let username = username.clone();
        tokio::task::spawn(async move {
            state.ctx.clients.request_token(username.value()).await;
        });
    });

    top.end();
    top.as_group().unwrap()
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
            let mut sub = state.ctx.clients.token.subscribe();
            loop {
                {
                    let token = sub.borrow_and_update();
                    if let Some(ref token) = *token {
                        title.set_label(&token.user);
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
