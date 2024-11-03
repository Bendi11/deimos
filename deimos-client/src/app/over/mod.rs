use std::sync::Arc;

use fltk::{enums::{Align, CallbackTrigger, Font, FrameType}, frame::Frame, group::{Flex, Group, Pack, PackType, Scroll, ScrollType}, image::SvgImage, prelude::{GroupExt, WidgetBase, WidgetExt}};

use crate::context::pod::{CachedPod, CachedPodState};

use super::{orbit, widget, DeimosStateHandle};

pub mod header;


pub struct Overview {
    top: Group,
}


impl Overview {
    pub fn new(state: DeimosStateHandle) -> Self {
        let mut top = {
            let top = Group::default_fill();
            let mut flex = Flex::default_fill().column();
            flex.set_margins(8, 8, 8, 0);
            flex.set_spacing(32);

            {
                let header = Self::header(state.clone());
                flex.fixed(&header, 64);

                {
                    let mut servers_container = Flex::default().row();
                    servers_container.set_margins(16, 0, 16, 0);
                    flex.fixed(&servers_container, 20);

                    let mut label = Frame::default();
                    label.set_label_font(Font::CourierBold);
                    label.set_label_size(22);
                    label.set_label_color(orbit::SOL[0]);
                    label.set_label("Servers");
                    label.set_align(Align::Inside | Align::Left);
                    
                    let reload_svg = SvgImage::from_data(include_str!("../../../assets/reload.svg")).unwrap();
                    let reload_rgb = widget::svg::svg_color(reload_svg, 128, orbit::MERCURY[2]);
                    let mut reload_button = widget::button::button(orbit::NIGHT[2], orbit::NIGHT[0]);
                    servers_container.fixed(&reload_button, servers_container.height());
                    reload_button.set_image(Some(reload_rgb));
                    reload_button.set_align(Align::Center);
                    reload_button.resize_callback(widget::svg::resize_image_cb(0, 0));
                    
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
                        top.resizable(&scroll);
                        scroll.set_frame(FrameType::NoBox);
                        scroll.set_color(orbit::NIGHT[2]);
                        scroll.set_type(ScrollType::Vertical);
                        scroll.set_align(Align::Center | Align::Inside);
                        
                        let mut pods_pack = Pack::default_fill();
                        pods_pack.set_spacing(32);
                        pods_pack.set_frame(FrameType::NoBox);
                        pods_pack.set_color(orbit::SOL[0]);
                        pods_pack.set_type(PackType::Vertical);
                        
                        let mut pods_pack_resize = pods_pack.clone();
                        scroll.resize_callback(
                            move |s,_,_,_,_| {
                                pods_pack_resize.set_pos(s.x() + 8, s.y() + 4);
                                pods_pack_resize.set_size(s.width() - 16, 0);
                            }
                        );

                        tokio::spawn(
                            async move {
                                let mut sub = state.ctx.pods.subscribe();
                                loop {
                                    tracing::trace!("Got pods notification {:?}", *sub.borrow());
                                    
                                    fltk::app::lock().ok();

                                    while pods_pack.children() > 0 {
                                        let Some(child) = pods_pack.child(0) else {
                                            continue
                                        };
                                        
                                        tracing::trace!("Deleting widget {}", child.label());
                                        pods_pack.remove_by_index(0);
                                    }

                                    for pod in sub.borrow_and_update().values() {
                                        tracing::trace!("Adding button for {}", pod.data.name);
                                        let button = Self::pod_button(state.clone(), pod.clone());
                                        pods_pack.add(&button);
                                    }
                                    
                                    pods_pack.set_damage(true);
                                    fltk::app::unlock();
                                    fltk::app::awake();

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
        
        Self {
            top,
        }
    }
    
    /// Create a button with a brief overview of the given pod
    pub fn pod_button(state: DeimosStateHandle, pod: Arc<CachedPod>) -> impl GroupExt {
        let mut row = Flex::new(0, 0, 0, 64, "").row();
        
        let up_state = {
            let mut column = Flex::default().column();
            column.set_frame(FrameType::RShadowBox);
            column.set_color(orbit::NIGHT[1]);
            column.set_margins(8, 8, 0, 8);

            let mut title = Frame::default();
            title.set_label(&pod.data.name);
            title.set_label_font(Font::CourierBold);
            title.set_label_color(orbit::SOL[1]);
            title.set_align(Align::Inside | Align::TopLeft);
            title.set_label_size(16);

            let mut up_state = Frame::default();
            up_state.set_label_font(Font::Screen);
            up_state.set_align(Align::Inside | Align::Left);
            up_state.set_label_size(12);

            column.end();

            up_state
        };
        
        let start_svg = SvgImage::from_data(include_str!("../../../assets/start.svg")).unwrap();
        let start_rgb = widget::svg::svg_color(start_svg, 128, orbit::MERCURY[1]);
        
        let stop_svg = SvgImage::from_data(include_str!("../../../assets/stop.svg")).unwrap();
        let stop_rgb = widget::svg::svg_color(stop_svg, 128, orbit::MARS[2]);

        let load_svg = SvgImage::from_data(include_str!("../../../assets/reload.svg")).unwrap();
        let load_rgb = widget::svg::svg_color(load_svg, 128, orbit::EARTH[1]);

        let mut button = widget::button::button(orbit::NIGHT[1], orbit::NIGHT[0]);
        row.fixed(&button, row.height());
        button.resize_callback(widget::svg::resize_image_cb(16, 16));

        {
            let mut up_state = up_state.clone();
            let mut button = button.clone();
            let up = pod.data.up.clone();
            tokio::task::spawn(async move {
                let mut sub = up.subscribe();
                loop {
                    fltk::app::lock().unwrap();
                    match *sub.borrow_and_update() {
                        CachedPodState::Paused => {
                            up_state.set_label("Paused");
                            up_state.set_label_color(orbit::MERCURY[2]);
                            button.set_image_scaled(Some(start_rgb.clone()));
                        }
                        CachedPodState::Disabled => {
                            up_state.set_label("Disabled");
                            up_state.set_label_color(orbit::NIGHT[0].lighter());
                            button.set_image_scaled(Some(start_rgb.clone()));
                        },
                        CachedPodState::Transit => {
                            up_state.set_label("");
                            button.set_image_scaled(Some(load_rgb.clone()));
                        },
                        CachedPodState::Enabled => {
                            up_state.set_label("Enabled");
                            up_state.set_label_color(orbit::EARTH[1]);
                            button.set_image_scaled(Some(stop_rgb.clone()));
                        }
                    }
                
                    button.resize(button.x(), button.y(), button.w(), button.h());
                    fltk::app::unlock();
                    fltk::app::awake();

                    let Ok(_) = sub.changed().await else {
                        tracing::trace!("Pod status notifier dropped");
                        break;
                    };
                }
            });
        }
        
        {
            let up = pod.data.up.clone();
            button.set_callback(move |_| {
                let current = *up.read();
                let to = match current {
                    CachedPodState::Disabled | CachedPodState::Paused => CachedPodState::Enabled,
                    CachedPodState::Transit => return,
                    CachedPodState::Enabled => CachedPodState::Disabled,
                };
                
                let state = state.clone();
                let pod = pod.clone();
                tokio::task::spawn(async move {
                    state.ctx.update(&pod, to).await;
                });
            });
        }

        row.end();

        row
    }

    pub fn group(&self) -> Group {
        self.top.clone()
    }
}
