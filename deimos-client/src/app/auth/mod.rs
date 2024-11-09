use fltk::{enums::{Align, CallbackTrigger, Font, FrameType}, frame::Frame, group::{Flex, Group, Pack}, image::SvgImage, input::Input, prelude::*};

use crate::context::client::auth::TokenStatus;

use super::{orbit, widget, DeimosStateHandle};



pub fn authorization(state: DeimosStateHandle) -> Group {
    let mut top = Flex::default_fill().column();
    top.set_margins(8, 8, 8, 8);
    top.set_color(orbit::NIGHT[2]);
    top.hide();

    let header = header(state.clone());
    top.fixed(&header, 42);

    let mut current_title = Frame::default();
    top.fixed(&current_title, 40);
    current_title.set_label("Current Token");
    current_title.set_label_size(24);
    current_title.set_label_font(Font::CourierBold);
    current_title.set_label_color(orbit::MERCURY[1]);
    current_title.set_align(Align::Inside | Align::Left);
    token_box(state.clone()).with_size(top.width(), 140);

    Frame::default_fill();

    let mut reqtitle = Frame::default();
    top.fixed(&reqtitle, 40);
    reqtitle.set_label("Token Request");
    reqtitle.set_label_size(24);
    reqtitle.set_label_font(Font::CourierBold);
    reqtitle.set_label_color(orbit::MERCURY[1]);
    reqtitle.set_align(Align::Inside | Align::Left);

    let request = request_group(state.clone());
    top.fixed(&request, 160);

    top.end();
    top.as_group().unwrap()
}

fn request_group(state: DeimosStateHandle) -> Pack {
    let pack = Pack::default_fill();

    let (mut frame, mut username) = widget::input::input_box::<Input>("Requested Token Username");
    frame.set_size(pack.width(), 40);
    match hostname::get() {
        Ok(hostname) => match hostname.to_str() {
            Some(hostname) => {
                username.set_value(hostname);
            },
            None => {
                tracing::warn!("Failed to decode hostname as UTF-8");
            }
        },
        Err(e) => {
            tracing::warn!("Failed to get hostname for default token request: {}", e);
        }
    }

    let mut request_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
    request_button.set_size(pack.width(), 60);
    request_button.set_label("Submit Token Request");
    request_button.set_label_color(orbit::MERCURY[0]);

    let mut status = Frame::default();
    status.set_size(pack.width(), 40);
    status.set_label_font(Font::Screen);

    username.set_trigger(CallbackTrigger::Changed);
    username.set_callback(move |u| {
        u.set_color(orbit::NIGHT[1]);
        u.redraw();
    });

    pack.end();

    {
        let state = state.clone();
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
    }

    {
        tokio::task::spawn(
            async move {
                let mut sub = state.ctx.clients.token.subscribe();
                loop {
                    {
                        let token = sub.borrow();
                        fltk::app::lock().ok();
                        request_button.activate();
                        match *token {
                            TokenStatus::Denied { ref reason } => {
                                status.set_label_color(orbit::MARS[2]);
                                status.set_label(reason);
                            },
                            TokenStatus::Requested { ref user, .. } => {
                                status.set_label_color(orbit::EARTH[1]);
                                status.set_label(&format!("Requested token with username '{}'", user));
                                request_button.deactivate();
                            },
                            TokenStatus::None | TokenStatus::Token(..) => {
                                status.set_label("");
                            },
                        }

                        fltk::app::unlock();
                        fltk::app::awake();
                    }

                    if sub.changed().await.is_err() {
                        break
                    }
                }
            }
        );
    }

    pack
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

                if let Some(token) = token.token() {
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

    let back_svg = SvgImage::from_data(include_str!("../../../assets/close.svg")).unwrap();
    let back_rgb = widget::svg::svg_color(back_svg, 128, orbit::MERCURY[1]);
    let mut back_button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
    row.fixed(&back_button, row.height());
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
