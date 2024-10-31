use fltk::enums::Color;

pub type ColorLevels = [Color; 4];


pub const NIGHT: ColorLevels = [
    Color::from_u32(0x2e2e2e),
    Color::from_u32(0x1B1B1C),
    Color::from_u32(0x121214),
    Color::from_u32(0x09090B),
];

pub const MERCURY: ColorLevels = [
    Color::from_u32(0x9b98a1),
    Color::from_u32(0x8F8B96),
    Color::from_u32(0x898492),
    Color::from_u32(0x837C8E),
];

pub const MARS: ColorLevels = [
    Color::from_u32(0xAD6242),
    Color::from_u32(0x964632),
    Color::from_u32(0x8A382A),
    Color::from_u32(0x7E2A22),
];

pub const VENUS: ColorLevels = [
    Color::from_u32(0xC49656),
    Color::from_u32(0xBF8E4A),
    Color::from_u32(0xB48349),
    Color::from_u32(0xA87748),
];

pub const EARTH: ColorLevels = [
    Color::from_u32(0x3793B2),
    Color::from_u32(0x3689B3),
    Color::from_u32(0x2F71A1),
    Color::from_u32(0x23577D),
];

pub const SOL: ColorLevels = [
    Color::from_u32(0xfff4ea),
    Color::from_u32(0xFFEDDB),
    Color::from_u32(0xFFE8D1),
    Color::from_u32(0xFFE4C7),
];
