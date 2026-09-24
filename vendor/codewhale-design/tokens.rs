// Generated from Codewhale GPUI design 1.0.0; sha256 a0c23f8a235bbc61e42fb8a60795ae7d9825dbbd1730142acc9ffec9c0abb3af. Do not edit.
#![allow(dead_code)]

pub const VERSION: &str = "1.0.0";
#[derive(Clone, Copy, Debug)]
pub struct Colors {
    pub background: u32,
    pub foreground: u32,
    pub surface: u32,
    pub muted_foreground: u32,
    pub border: u32,
    pub sidebar: u32,
    pub primary: u32,
    pub primary_foreground: u32,
    pub hover: u32,
    pub selected: u32,
    pub attention: u32,
}
pub const DARK: Colors = Colors {
    background: 0x202123,
    foreground: 0xefeeeb,
    surface: 0x2a2b2e,
    muted_foreground: 0xb1b1ad,
    border: 0x3b3c3f,
    sidebar: 0x191a1c,
    primary: 0x90b9ff,
    primary_foreground: 0x15243e,
    hover: 0x303134,
    selected: 0x37393d,
    attention: 0xe8b077,
};
pub const LIGHT: Colors = Colors {
    background: 0xfaf8f5,
    foreground: 0x28292b,
    surface: 0xffffff,
    muted_foreground: 0x5f605d,
    border: 0xd9d5cf,
    sidebar: 0xf0ede8,
    primary: 0x245bc7,
    primary_foreground: 0xfbf5ee,
    hover: 0xe8e5e0,
    selected: 0xdfdcd6,
    attention: 0x86520d,
};
pub fn colors(dark: bool) -> Colors {
    if dark { DARK } else { LIGHT }
}
pub const FONT_FAMILY: &str = "Shannon Sans";
pub const FONT_FALLBACKS: &[&str] = &[
    "PingFang SC",
    "Hiragino Sans GB",
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "Noto Sans CJK SC",
    "Noto Sans SC",
    "Source Han Sans SC",
];
pub const SPACING_COMPACT: f32 = 4.0;
pub const SPACING_CONTROL: f32 = 8.0;
pub const SPACING_GROUP: f32 = 12.0;
pub const SPACING_SECTION: f32 = 16.0;
pub const SPACING_LARGE: f32 = 24.0;
pub const SPACING_PAGE: f32 = 32.0;
pub const RADIUS_CONTROL: f32 = 6.0;
pub const RADIUS_PANEL: f32 = 10.0;
pub const FOCUS_WIDTH: f32 = 2.0;
pub const FOCUS_OFFSET: f32 = 3.0;
pub const ICONS_GRID: f32 = 24.0;
pub const ICONS_STROKE: f32 = 1.7;
pub const MOTION_SPRING_STIFFNESS: f32 = 420.0;
pub const MOTION_SPRING_DAMPING: f32 = 42.0;
pub const MOTION_SPRING_MASS: f32 = 1.0;
pub const MOTION_PET_POLL_MS: f32 = 30.0;
pub const MOTION_REDUCED_POLL_MS: f32 = 500.0;
pub const SELECTION_OPACITY: f32 = 0.28;
pub const PRIMARY_HOVER_OPACITY: f32 = 0.9;
pub const MONO_PX: f32 = 13.0;
