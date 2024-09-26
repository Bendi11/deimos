use iced::color;


pub type ColorLevels = [iced::Color ; 4];

pub const NIGHT: ColorLevels = [
    color!(0x2e2e2e),
    color!(0x1B1B1C),
    color!(0x121214),
    color!(0x09090B),
];

pub const MERCURY: ColorLevels = [
    color!(0x9b98a1),
    color!(0x8F8B96),
    color!(0x898492),
    color!(0x837C8E),
];

pub const MARS: ColorLevels = [
    color!(0xAD6242),
    color!(0x964632),
    color!(0x8A382A),
    color!(0x7E2A22),
];

pub const VENUS: ColorLevels = [
    color!(0xC49656),
    color!(0xBF8E4A),
    color!(0xB48349),
    color!(0xA87748),
];

pub const EARTH: ColorLevels = [
    color!(0x3793B2),
    color!(0x3689B3),
    color!(0x2F71A1),
    color!(0x23577D),
];

pub const SOL: ColorLevels = [
    color!(0xfff4ea),
    color!(0xFFEDDB),
    color!(0xFFE8D1),
    color!(0xFFE4C7),
];
