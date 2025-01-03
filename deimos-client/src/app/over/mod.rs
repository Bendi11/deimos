use std::{collections::BTreeMap, sync::Arc};

use fltk::{button::Button, enums::{Align, Event, FrameType}, frame::Frame, group::{Flex, Group, Pack, PackType, Scroll, ScrollType}, image::SvgImage, prelude::{GroupExt, WidgetBase, WidgetExt}};

use crate::context::pod::{CachedPod, CachedPodState};

use super::{orbit, style, DeimosStateHandle};

pub mod header;


pub fn overview(state: DeimosStateHandle) -> Group {
    let mut top = {
        let top = Group::default_fill();
        let mut flex = Flex::default_fill().column();
        flex.set_margins(8, 8, 8, 0);
        flex.set_spacing(32);

        {
            let header = header::header(state.clone());
            flex.fixed(&header, 64);

            {
                let mut servers_container = Flex::default().row();
                servers_container.set_margins(16, 0, 16, 0);
                flex.fixed(&servers_container, 20);

                let mut label = Frame::default();
                label.set_label_font(crate::app::HEADER_FONT);
                label.set_label_size(22);
                label.set_label_color(orbit::SOL[0]);
                label.set_label("Servers");
                label.set_align(Align::Inside | Align::Left);
                
                let reload_svg = SvgImage::from_data(include_str!("../../../assets/reload.svg")).unwrap();
                let reload_rgb = style::svg::svg_color(reload_svg, servers_container.height(), orbit::MERCURY[2]);
                let mut reload_button = style::button::button::<Button>(orbit::NIGHT[2], orbit::NIGHT[0]);
                servers_container.fixed(&reload_button, servers_container.height());
                reload_button.set_image(Some(reload_rgb));
                reload_button.set_align(Align::Center);
                
                {
                    let state = state.clone();
                    reload_button.set_callback(move |_| {
                        let state = state.clone();
                        tokio::task::spawn(
                            async move {
                                state.ctx.synchronize().await;
                            }
                        );
                    });
                }
                servers_container.end();
            }

            {

                {
                    let mut scroll = Scroll::default_fill();
                    scroll.set_frame(FrameType::NoBox);
                    scroll.set_color(orbit::NIGHT[2]);
                    scroll.set_type(ScrollType::Vertical);
                    scroll.set_align(Align::Center | Align::Inside);
                    
                    let mut pods_pack = Pack::default_fill();
                    pods_pack.set_spacing(16);
                    pods_pack.set_frame(FrameType::NoBox);
                    pods_pack.set_color(orbit::SOL[0]);
                    pods_pack.set_type(PackType::Vertical);
                    
                    let mut pods_pack_resize = pods_pack.clone();
                    scroll.resize_callback(
                        move |s,_,_,_,_| {
                            pods_pack_resize.set_pos(s.x() + 8, s.y() + 4);
                            pods_pack_resize.set_size(s.width() - 16, s.height());
                        }
                    );

                    pods_pack.end();

                    tokio::spawn(
                        async move {
                            let mut buttons = BTreeMap::<String, Flex>::new();
                            let mut sub = state.ctx.pods.subscribe();
                            loop {
                                {
                                    fltk::app::lock().ok();

                                    for button in buttons.values() {
                                        pods_pack.remove(button);
                                    }

                                    let pods = sub.borrow_and_update();

                                    buttons.retain(|id,_| pods.contains_key(id));
                                    for (id, pod) in pods.clone() {
                                        buttons
                                            .entry(id)
                                            .or_insert_with(|| pod_button(state.clone(), pod.clone()));
                                    }

                                    for button in buttons.values_mut() {
                                        pods_pack.add(button);
                                    }
                                    

                                    pods_pack.set_damage(true);
                                    fltk::app::unlock();
                                    fltk::app::awake();
                                }

                                let Ok(_) = sub.changed().await else {
                                    break
                                };
                            }
                        }
                    );
                }
            }
        }
        top
    };
    
    top.end();
    top.hide();
    
    top
}

/// Create a button with a brief overview of the given pod
pub fn pod_button(state: DeimosStateHandle, pod: Arc<CachedPod>) -> Flex {
    let mut row = Flex::default().with_size(0, 64).row();
    row.set_spacing(1);

    let up_state = {
        let mut column = Flex::default().column();
        column.set_frame(FrameType::RShadowBox);
        column.set_color(orbit::NIGHT[1]);
        column.set_margins(8, 8, 0, 8);

        let mut title = Frame::default();
        title.set_label_font(crate::app::HEADER_FONT);
        title.set_label_color(orbit::SOL[1]);
        title.set_align(Align::Inside | Align::TopLeft);
        title.set_label_size(18);

        let mut up_state = Frame::default();
        up_state.set_label_font(crate::app::SUBTITLE_FONT);
        up_state.set_align(Align::Inside | Align::Left);
        up_state.set_label_size(12);

        column.end();
        
        let pod = pod.clone();
        tokio::task::spawn(async move {
            let mut sub = pod.data.name.subscribe();
            loop {
                fltk::app::lock().ok();
                title.set_label(&sub.borrow_and_update());
                title.set_damage(true);
                fltk::app::unlock();
                fltk::app::awake();

                if sub.changed().await.is_err() {
                    break
                }
            }
        });

        up_state
    };

    let dim = row.height() - 16;
    
    let start_svg = SvgImage::from_data(include_str!("../../../assets/start.svg")).unwrap();
    let start_rgb = style::svg::svg_color(start_svg, dim, orbit::MERCURY[1]);
    
    let stop_svg = SvgImage::from_data(include_str!("../../../assets/stop.svg")).unwrap();
    let stop_rgb = style::svg::svg_color(stop_svg, dim, orbit::MARS[2]);

    let load_svg = SvgImage::from_data(include_str!("../../../assets/reload.svg")).unwrap();
    let load_rgb = style::svg::svg_color(load_svg, dim, orbit::EARTH[1]);

    let pause_svg = SvgImage::from_data(include_str!("../../../assets/pause.svg")).unwrap();
    let pause_rgb = style::svg::svg_color(pause_svg, dim - 16, orbit::VENUS[3]);

    let mut pause_button = style::button::button::<Button>(orbit::NIGHT[1], orbit::NIGHT[0]);
    pause_button.hide();
    row.fixed(&pause_button, row.height());
    pause_button.set_image(Some(pause_rgb));

    let mut button = style::button::button::<Button>(orbit::NIGHT[1], orbit::NIGHT[0]);
    row.fixed(&button, row.height());


    {
        let row = row.clone();
        let mut up_state = up_state.clone();
        let mut button = button.clone();
        let mut pause_button = pause_button.clone();
        let up = pod.data.up.clone();
        let load_rgb = load_rgb.clone();
        tokio::task::spawn(async move {
            let mut sub = up.subscribe();
            loop {
                fltk::app::lock().unwrap();
                match *sub.borrow_and_update() {
                    CachedPodState::Paused => {
                        up_state.set_label("Paused");
                        up_state.set_label_color(orbit::VENUS[3]);
                        button.set_image(Some(start_rgb.clone()));
                        pause_button.hide();
                    }
                    CachedPodState::Disabled => {
                        up_state.set_label("Disabled");
                        up_state.set_label_color(orbit::NIGHT[0].lighter());
                        button.set_image(Some(start_rgb.clone()));
                        pause_button.hide();
                    },
                    CachedPodState::Transit => {
                        up_state.set_label("");
                        button.set_color(orbit::NIGHT[1]);
                        button.set_image(Some(load_rgb.clone()));
                        pause_button.hide();
                    },
                    CachedPodState::Enabled => {
                        up_state.set_label("Enabled");
                        up_state.set_label_color(orbit::EARTH[1]);
                        button.set_image(Some(stop_rgb.clone()));
                        pause_button.show();
                    }
                }
                
                let row = row.clone();
                fltk::app::awake_callback(move || {
                    row.layout();
                    if let Some(mut under) = fltk::app::belowmouse::<fltk::button::Button>() {
                        under.handle_event(Event::Enter);
                    }
                });
                fltk::app::unlock();
                fltk::app::awake();

                let Ok(_) = sub.changed().await else {
                    break;
                };
            }
        });
    }
    
    {
        let state = state.clone();
        let pod = pod.clone();
        let up = pod.data.up.clone();
        button.set_callback(move |b| {
            let current = *up.read();
            let to = match current {
                CachedPodState::Disabled | CachedPodState::Paused => CachedPodState::Enabled,
                CachedPodState::Transit => return,
                CachedPodState::Enabled => CachedPodState::Disabled,
            };
            
            fltk::app::lock().ok();
            b.set_image(Some(load_rgb.clone()));
            fltk::app::unlock();
            fltk::app::awake();
            
            let state = state.clone();
            let pod = pod.clone();
            tokio::task::spawn(async move {
                state.ctx.update(&pod, to).await;
            });
        });
    }

    {
        let up = pod.data.up.clone();
        pause_button.set_callback(move |_| {
            if *up.read() != CachedPodState::Enabled {
                return
            }
            
            let state = state.clone();
            let pod = pod.clone();
            tokio::task::spawn(async move {
                state.ctx.update(&pod, CachedPodState::Paused).await;
            });
        });
    }

    row.end();

    row
}
