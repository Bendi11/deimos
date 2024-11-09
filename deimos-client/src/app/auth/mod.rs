use fltk::{enums::{Align, CallbackTrigger, Font, FrameType}, frame::Frame, group::{Flex, Group, Pack, PackType}, image::SvgImage, input::Input, prelude::*};

use super::{orbit, widget, DeimosStateHandle};



pub fn authorization(state: DeimosStateHandle) -> Group {
    let mut top = Pack::default_fill();
    top.set_color(orbit::NIGHT[2]);
    top.set_type(PackType::Vertical);
    top.set_size(top.width() - 16, top.height());
    top.set_pos(8, 0);
    top.hide();

    header(state.clone()).with_size(top.width(), 42);

    let mut current_title = Frame::default().with_size(top.width(), 40);
    current_title.set_label("Current Token");
    current_title.set_label_size(20);
    current_title.set_label_font(Font::CourierBold);
    current_title.set_label_color(orbit::MERCURY[1]);
    current_title.set_align(Align::Inside | Align::Left);
    token_box(state.clone()).with_size(top.width(), 140);
   

    let space = Frame::default().with_size(0, 64);
    top.resizable(&space);

    let mut reqtitle = Frame::default().with_size(top.width(), 40);
    reqtitle.set_label("Request New Token");
    reqtitle.set_label_size(20);
    reqtitle.set_label_font(Font::CourierBold);
    reqtitle.set_label_color(orbit::MERCURY[1]);
    reqtitle.set_align(Align::Inside | Align::Left);
    

    let (_, mut username) = widget::input::input_box::<Input>("Requested Token Username");
    let mut request_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
    request_button.set_size(top.width(), 40);
    request_button.set_label("Request Token");
    request_button.set_label_color(orbit::MERCURY[0]);
    
    
    username.set_trigger(CallbackTrigger::Changed);
    username.set_callback(move |u| {
        u.set_color(orbit::NIGHT[1]);
        u.redraw();
    });
    
    request_button.set_callback(move |_| {
        let state = state.clone();
        let name = username.value();
        if name.is_empty() {
            username.set_color(orbit::MARS[3]);
            username.redraw();
            return
        }

        tokio::task::spawn(async move {
            state.ctx.clients.request_token(name).await;
        });
    });

    top.end();
    top.as_group().unwrap()
}

fn token_box(state: DeimosStateHandle) -> Flex {
    let mut container = Flex::default_fill().column();
    container.set_margins(8, 8, 8, 8);
    container.set_frame(FrameType::RShadowBox);
    container.set_color(orbit::NIGHT[1]);

    let mut username = token_box_field("Username");
    let mut issued = token_box_field("Issued Date");
    let mut fingerprint = token_box_field("Fingerprint");
    
    tokio::task::spawn(async move {
        let mut sub = state.ctx.clients.token.subscribe();
        loop {
            {
                let token = sub.borrow_and_update();

                fltk::app::lock().ok();

                if let Some(ref token) = *token {
                    username.set_label(&token.user);
                    issued.set_label(&token.issued.naive_local().format("%B %d %Y").to_string());
                    fingerprint.set_label(&token.key.fingerprint());
                } else {
                    username.set_label("");
                    issued.set_label("");
                    fingerprint.set_label("");
                }

                fltk::app::redraw();

                fltk::app::unlock();
                fltk::app::awake();
            }

            if sub.changed().await.is_err() {
                break
            }
        }
    });

    container.end();
    container
}

fn token_box_field(name: &'static str) -> Frame {
    let mut row = Flex::default_fill().row();
    
    let mut label = Frame::default();
    label.set_label(name);
    label.set_label_font(Font::CourierBold);
    label.set_label_color(orbit::SOL[1]);
    label.set_label_size(20);
    row.fixed(&label, row.width() / 3);

    let mut field = Frame::default();
    field.set_label_font(Font::Screen);
    field.set_label_color(orbit::MERCURY[0]);
    field.set_label_size(20);

    row.end();

    field
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
