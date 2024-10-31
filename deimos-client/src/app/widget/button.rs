use fltk::{button::Button, enums::{Color, Event, FrameType}, prelude::{ButtonExt, WidgetBase, WidgetExt}};


/// Get a button initialized with the given colors, which will change background color on hover
pub fn button(color: Color, hovered: Color) -> Button {
    let mut button = Button::default();
    button.set_frame(FrameType::RShadowBox);
    button.set_down_frame(FrameType::RoundDownBox);
    button.set_color(color);
    button.clear_visible_focus();
    button.handle(
        move |b, ev| match ev {
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
                b.set_color(color);
                b.redraw();
                true
            },
            Event::Released => {
                b.set_color(hovered);
                b.redraw();
                true
            },
            _ => false,
        }
    );

    button
}
