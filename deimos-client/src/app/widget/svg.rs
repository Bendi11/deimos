use fltk::{enums::Color, image::{RgbImage, SvgImage}, prelude::{ImageExt, WidgetBase}};


pub fn svg_color(img: SvgImage, sz: i32, color: Color) -> RgbImage {
    let mut img = img.copy_sized(sz, sz);
    img.normalize();
    convert_color(img, color)
}

pub fn resize_image_cb<T: WidgetBase>(mx: i32, my: i32) -> impl FnMut(&mut T, i32, i32, i32, i32) {
    move |w: &mut T, _, _, _, _| {
        if let Some(mut image) = w.image() {
            image.scale(w.height() - mx, w.height() - my, true, true);
            w.redraw();
        }
    }
}

fn convert_color(mut img: SvgImage, color: Color) -> RgbImage {
    let (r, g, b) = color.to_rgb();
    img.normalize();
    let mut data = img.to_rgb_data();
    for rgba in data.chunks_mut(4) {
        rgba[0] = r;
        rgba[1] = g;
        rgba[2] = b;
    }

    RgbImage::new(&data, img.data_w(), img.data_h(), fltk::enums::ColorDepth::Rgba8)
        .expect("Failed to convert SVG to new color space")
}
