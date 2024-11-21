use fltk::{enums::{Color, Event, FrameType}, prelude::{ButtonExt, WidgetBase}};


/// Get a button initialized with the given colors, which will change background color on hover
pub fn button<B: ButtonExt + WidgetBase + Default>(color: Color, hovered: Color) -> B {
    let mut button = B::default();
    button.set_frame(FrameType::RShadowBox);
    button.set_down_frame(FrameType::RShadowBox);
    button.set_selection_color(color);
    button.set_color(color);
    button.clear_visible_focus();
    button.handle(hover_handler(color, hovered, color));

    button
}

pub fn hover_handler<B: ButtonExt>(color: Color, hovered: Color, pressed: Color) -> impl FnMut(&mut B, Event) -> bool + 'static {
    move |b, ev|{ /*tracing::trace!("Button got event {:?}", ev);*/ match ev {
        Event::Hide => {
            b.set_color(color);
            true
        },
        Event::Enter => {
            b.set_color(hovered);
            b.redraw();
            true
        },
        Event::Leave => {
            b.set_color(color);
            b.redraw();
            true
        },
        Event::Push => {
            b.set_color(pressed);
            b.redraw();
            true
        },
        Event::Released => {
            b.set_color(hovered);
            b.redraw();
            true
        },
        _ => false,
    }}
}
