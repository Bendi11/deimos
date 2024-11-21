use fltk::{enums::Color, image::{RgbImage, SvgImage}, prelude::ImageExt};


/// Render the given SVG image for the given size, and recolor all non-transparent pixels to the
/// given color
pub fn svg_color(img: SvgImage, sz: i32, color: Color) -> RgbImage {
    let mut img = img.copy_sized(sz, sz);
    img.normalize();
    convert_color(img, color)
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
