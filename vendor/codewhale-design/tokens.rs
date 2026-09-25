// Generated from Codewhale GPUI design 1.1.1; sha256 c08d7d4b253afe340207658b1f233dd1a9dbe94fa57e925bde025a3db70543ee. Do not edit.
#![allow(dead_code)]

pub const VERSION: &str = "1.1.1";
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
    pub live: u32,
    pub danger: u32,
    pub border_strong: u32,
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
    live: 0x9ec7b2,
    danger: 0xe39a90,
    border_strong: 0x828386,
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
    live: 0x3a6352,
    danger: 0x9e3f36,
    border_strong: 0x807c76,
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
pub const RADIUS_SHEET: f32 = 14.0;
pub const FOCUS_WIDTH: f32 = 2.0;
pub const FOCUS_OFFSET: f32 = 3.0;
pub const ICONS_GRID: f32 = 24.0;
pub const ICONS_STROKE: f32 = 1.7;
pub const MOTION_SPRING_STIFFNESS: f32 = 420.0;
pub const MOTION_SPRING_DAMPING: f32 = 42.0;
pub const MOTION_SPRING_MASS: f32 = 1.0;
pub const MOTION_PET_POLL_MS: f32 = 30.0;
pub const MOTION_REDUCED_POLL_MS: f32 = 500.0;
pub const MOTION_DURATION_STATE_MS: f32 = 120.0;
pub const MOTION_DURATION_ARRIVE_MS: f32 = 180.0;
pub const MOTION_DURATION_PANEL_MS: f32 = 340.0;
pub const SELECTION_OPACITY: f32 = 0.28;
pub const PRIMARY_HOVER_OPACITY: f32 = 0.9;
pub const MONO_PX: f32 = 13.0;
pub const TYPE_BODY_PX: f32 = 14.0;
pub const TYPE_CAPTION_PX: f32 = 12.0;
pub const TYPE_PROSE_PX: f32 = 17.0;
pub const TYPE_HEADING_PX: f32 = 17.0;
pub const TYPE_TITLE_PX: f32 = 22.0;
