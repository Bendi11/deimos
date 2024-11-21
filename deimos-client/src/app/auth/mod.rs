use fltk::{button::Button, enums::{Align, CallbackTrigger, FrameType}, frame::Frame, group::{Flex, Group, Pack}, image::SvgImage, input::Input, prelude::*};

use crate::context::client::auth::{PersistentTokenKind, TokenStatus};

use super::{orbit, style::{self}, DeimosStateHandle};



pub fn authorization(state: DeimosStateHandle) -> Group {
    let mut top = Flex::default_fill().column();
    top.set_margins(8, 8, 8, 8);
    top.set_color(orbit::NIGHT[2]);
    top.set_spacing(8);
    top.hide();

    let header = header(state.clone());
    top.fixed(&header, 42);

    
    top.fixed(&label("Current Token"), 40);
    top.fixed(&token_box(state.clone()), 140);

    Frame::default_fill();

    top.fixed(&label("Token Request"), 40);
    let request = request_group(state.clone());
    top.fixed(&request, 140);

    Frame::default_fill();

    top.fixed(&label("Token Protection"), 40);
    let mut protection_status = Frame::default();
    top.fixed(&protection_status, 20);
    protection_status.set_label_font(crate::app::SUBTITLE_FONT);
    protection_status.set_label_size(16);
    protection_status.set_align(Align::Center | Align::Inside);

    let mut dpapi_button = style::button::button::<Button>(orbit::NIGHT[1], orbit::NIGHT[0]);
    dpapi_button.set_label_font(crate::app::SUBTITLE_FONT);
    dpapi_button.set_label_size(18);
    dpapi_button.set_label_color(orbit::MERCURY[0]);
    top.fixed(&dpapi_button, 40);

    {
        let state = state.clone();
        dpapi_button.set_callback(move |_| {
            let token_protect = state.ctx.clients.token_protect.clone();
            let current = *token_protect.read();
            token_protect.set(
                match current {
                    #[cfg(not(windows))]
                    PersistentTokenKind::Plaintext => PersistentTokenKind::Plaintext,
                    #[cfg(windows)]
                    PersistentTokenKind::Plaintext => PersistentTokenKind::Dpapi,
                    #[cfg(windows)]
                    PersistentTokenKind::Dpapi => PersistentTokenKind::Plaintext,
                }
            );
        });
    }

    {
        let state = state.clone();
        tokio::task::spawn(async move {
            let mut sub = state.ctx.clients.token_protect.subscribe();
            loop {
                let protect = *sub.borrow_and_update();

                fltk::app::lock().ok();
                match protect {
                    PersistentTokenKind::Plaintext => {
                        protection_status.set_label("Token is not encrypted at-rest");
                        protection_status.set_label_color(orbit::MARS[1]);
                        dpapi_button.set_label("Enable encryption");
                    },
                    #[cfg(windows)]
                    PersistentTokenKind::Dpapi => {
                        protection_status.set_label("Token is encrypted at-rest");
                        protection_status.set_label_color(orbit::EARTH[1]);
                        dpapi_button.set_label("Disable encryption");
                    }
                }
                
                dpapi_button.set_damage(true);
                protection_status.set_damage(true);
                fltk::app::unlock();
                fltk::app::awake();

                if sub.changed().await.is_err() {
                    break
                }
            }
        });
    }

    top.end();
    top.as_group().unwrap()
}

fn label(lbl: &str) -> Frame {
    let mut frame = Frame::default();
    frame.set_label(lbl);
    frame.set_label_size(24);
    frame.set_label_color(orbit::MERCURY[0]);
    frame.set_align(Align::Inside | Align::Left);
    frame
}

fn request_group(state: DeimosStateHandle) -> Pack {
    let pack = Pack::default_fill();

    let (mut frame, mut username) = style::input::input_box::<Input>("Username");
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

    let mut request_button = style::button::button::<Button>(orbit::NIGHT[1], orbit::NIGHT[0]);
    request_button.set_size(pack.width(), 40);
    request_button.set_label("Submit Token Request");
    request_button.set_label_font(crate::app::SUBTITLE_FONT);
    request_button.set_label_size(18);
    request_button.set_label_color(orbit::MERCURY[0]);

    let mut status = Frame::default();
    status.set_size(pack.width(), 40);
    status.set_label_font(crate::app::SUBTITLE_FONT);

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
                        
                        status.set_damage(true);
                        request_button.set_damage(true);
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
    label.set_label_font(crate::app::HEADER_FONT);
    label.set_label_color(orbit::MERCURY[0]);
    label.set_label_size(20);
    row.fixed(&label, row.width() / 3);

    let mut field = Frame::default();
    field.set_label_font(crate::app::SUBTITLE_FONT);
    field.set_label_color(orbit::MERCURY[0]);
    field.set_label_size(20);

    row.end();

    field
}

fn header(state: DeimosStateHandle) -> Flex {
    let mut row = Flex::default()
        .with_size(0, 42)
        .row()
        .with_align(Align::Center);

    let back_svg = SvgImage::from_data(include_str!("../../../assets/close.svg")).unwrap();
    let back_rgb = style::svg::svg_color(back_svg, row.height() - 16, orbit::MERCURY[1]);
    let mut back_button = style::button::button::<Button>(orbit::NIGHT[1], orbit::NIGHT[0]);
    row.fixed(&back_button, row.height());
    back_button.set_image(Some(back_rgb));
    back_button.set_callback(move |_| {
        let state = state.clone();
        tokio::task::spawn(async move {
            state.set_view(state.overview.clone()).await;
        });
    });

    let mut title = Frame::default();
    title.set_label_color(orbit::SOL[0]);
    title.set_label_font(crate::app::HEADER_FONT);
    title.set_label("Token Management");

    row.resize_callback(move |r,_,_,_,_| {
        r.fixed(&back_button, r.height());
        title.set_label_size(r.height() / 2);
    });



    row.end();
    row
}
