//! Theme identifiers, setting normalizers and hex parsing.
//!
//! Everything here compiles without the `ratatui` feature: a settings
//! layer (the headless runtime) validates `theme = "..."` and colour
//! strings without linking a renderer. Resolving an id to a `UiTheme`
//! lives in `themes` behind the feature.

/// Stable identifiers for the named themes the user can select. `System`
/// defers to `PaletteMode::detect()` (terminal-driven dark/light). Each
/// dark/light id resolves to a single fixed `UiTheme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeId {
    System,
    Terminal,
    Shoreline,
    ShorelineLight,
    Underwater,
    UnderwaterRetro,
    Whale,
    WhaleLight,
    Grayscale,
    CatppuccinMocha,
    TokyoNight,
    Dracula,
    GruvboxDark,
    Claude,
    Matrix,
    SolarizedLight,
    Uwu,
}

impl ThemeId {
    /// Parse a settings string (`"system"`, `"dark"`, `"catppuccin-mocha"`, …).
    /// Accepts a few aliases (`"whale"` for dark, `"light"` for whale-light)
    /// so existing config files keep working. Case-insensitive.
    #[must_use]
    pub fn from_name(value: &str) -> Option<Self> {
        match normalize_theme_name(value)? {
            "system" => Some(Self::System),
            "terminal" => Some(Self::Terminal),
            "underwater" | "deepsea" => Some(Self::Underwater),
            "underwater-retro" | "retro" => Some(Self::UnderwaterRetro),
            "shoreline" => Some(Self::Shoreline),
            "shoreline-light" => Some(Self::ShorelineLight),
            "dark" => Some(Self::Whale),
            "light" => Some(Self::WhaleLight),
            "grayscale" => Some(Self::Grayscale),
            "catppuccin-mocha" => Some(Self::CatppuccinMocha),
            "tokyo-night" => Some(Self::TokyoNight),
            "dracula" => Some(Self::Dracula),
            "gruvbox-dark" => Some(Self::GruvboxDark),
            "claude" => Some(Self::Claude),
            "matrix" => Some(Self::Matrix),
            "solarized-light" => Some(Self::SolarizedLight),
            "uwu" => Some(Self::Uwu),
            _ => None,
        }
    }

    /// Canonical settings string (lowercase, dash-separated). Round-trips
    /// through `from_name`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Terminal => "terminal",
            Self::Shoreline => "shoreline",
            Self::ShorelineLight => "shoreline-light",
            Self::Underwater => "underwater",
            Self::UnderwaterRetro => "underwater-retro",
            Self::Whale => "dark",
            Self::WhaleLight => "light",
            Self::Grayscale => "grayscale",
            Self::CatppuccinMocha => "catppuccin-mocha",
            Self::TokyoNight => "tokyo-night",
            Self::Dracula => "dracula",
            Self::GruvboxDark => "gruvbox-dark",
            Self::Claude => "claude",
            Self::Matrix => "matrix",
            Self::SolarizedLight => "solarized-light",
            Self::Uwu => "uwu",
        }
    }

    /// Human-readable label for picker rows.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Terminal => "Terminal",
            Self::Shoreline => "Shoreline",
            Self::ShorelineLight => "Shoreline Light",
            Self::Underwater => "Underwater",
            Self::UnderwaterRetro => "Underwater Retro",
            Self::Whale => "Blue Stage",
            Self::WhaleLight => "Blue Stage Light",
            Self::Grayscale => "Grayscale",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::TokyoNight => "Tokyo Night",
            Self::Dracula => "Dracula",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::Claude => "Claude",
            Self::Matrix => "Matrix",
            Self::SolarizedLight => "Solarized Light",
            Self::Uwu => "Uwu",
        }
    }

    /// Short tagline for picker rows.
    #[must_use]
    pub const fn tagline(self) -> &'static str {
        match self {
            Self::System => "Follow terminal background (COLORFGBG / macOS appearance)",
            Self::Terminal => "Inherit terminal colors fully (transparent surfaces, ANSI accents)",
            Self::Shoreline => "Warm charcoal, one restrained blue — the desktop client's palette",
            Self::ShorelineLight => "Shoreline on warm paper — the desktop client's light mode",
            Self::Underwater => "The painted ocean field: ombre water, ambient life, the whale",
            Self::UnderwaterRetro => "Flat phosphor-teal ocean: the legacy deepsea look, no ombre",
            Self::Whale => "Stage black, action blue, and one Signal Gold human beacon",
            Self::WhaleLight => "Paper, cobalt action, and one Signal Gold human beacon",
            Self::Grayscale => "Color-minimal high contrast",
            Self::CatppuccinMocha => "Soft pastels on warm dark",
            Self::TokyoNight => "Deep blue/violet night palette",
            Self::Dracula => "Classic high-contrast purple",
            Self::GruvboxDark => "Vintage warm earth tones",
            Self::Claude => "Warm navy & coral",
            Self::Matrix => "The Matrix films inspired theme",
            Self::SolarizedLight => {
                "Solarized light — Light, calming palette on warm ivory — easy on the eyes"
            }
            Self::Uwu => "Soft kawaii night — sakura, mint, and peach",
        }
    }
}

/// Themes shown in the `/theme` picker, in display order.
pub const SELECTABLE_THEMES: &[ThemeId] = &[
    ThemeId::System,
    ThemeId::Terminal,
    ThemeId::Shoreline,
    ThemeId::ShorelineLight,
    ThemeId::Underwater,
    ThemeId::UnderwaterRetro,
    ThemeId::Whale,
    ThemeId::WhaleLight,
    ThemeId::Grayscale,
    ThemeId::CatppuccinMocha,
    ThemeId::TokyoNight,
    ThemeId::Dracula,
    ThemeId::GruvboxDark,
    ThemeId::Claude,
    ThemeId::Matrix,
    ThemeId::SolarizedLight,
    ThemeId::Uwu,
];

#[must_use]
pub fn normalize_theme_name(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "auto" | "system" | "default" => Some("system"),
        "terminal" | "term" | "transparent" | "follow-terminal" | "inherit" => Some("terminal"),
        "underwater" | "deepsea" | "deep-sea" | "ocean" | "ombre" => Some("underwater"),
        "underwater-retro" | "retro" | "uw-retro" => Some("underwater-retro"),
        "shoreline" | "warm" | "charcoal" => Some("shoreline"),
        "shoreline-light" | "paper" => Some("shoreline-light"),
        "dark" | "whale" | "whale-dark" => Some("dark"),
        "light" | "whale-light" => Some("light"),
        "grayscale" | "greyscale" | "gray" | "grey" | "mono" | "monochrome" | "black-white"
        | "black_and_white" | "blackwhite" | "bw" | "b&w" => Some("grayscale"),
        "catppuccin-mocha" | "catppuccin" | "mocha" => Some("catppuccin-mocha"),
        "tokyo-night" | "tokyonight" | "tokyo" => Some("tokyo-night"),
        "dracula" => Some("dracula"),
        "gruvbox-dark" | "gruvbox" => Some("gruvbox-dark"),
        "claude" => Some("claude"),
        "matrix" | "hacker" => Some("matrix"),
        "solarized-light" | "solarized" => Some("solarized-light"),
        "uwu" | "owo" | "kawaii" => Some("uwu"),
        _ => None,
    }
}

pub const USER_THEME_PREFIX: &str = "custom:";

pub fn normalize_user_theme_selector(value: &str) -> Result<Option<String>, String> {
    let trimmed = value.trim();
    let Some(slug) = trimmed.strip_prefix(USER_THEME_PREFIX) else {
        return Ok(None);
    };
    let slug = slug.trim().to_ascii_lowercase();
    if slug.is_empty()
        || slug.len() > 64
        || !slug
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(
            "custom theme names must be 1-64 ASCII letters, digits, '-' or '_'".to_string(),
        );
    }
    Ok(Some(format!("{USER_THEME_PREFIX}{slug}")))
}

pub fn normalize_theme_setting(value: &str) -> Result<String, String> {
    if let Some(id) = ThemeId::from_name(value) {
        return Ok(id.name().to_string());
    }
    normalize_user_theme_selector(value)?.ok_or_else(|| {
        format!("invalid theme '{value}'; use a compiled theme name or custom:<name>")
    })
}

/// Parse `#rrggbb` (or `rrggbb`) into an RGB tuple.
#[must_use]
pub fn parse_hex_rgb(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if hex.len() != 6 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Canonical lowercase `#rrggbb` form of a hex colour string.
#[must_use]
pub fn normalize_hex_rgb_color(value: &str) -> Option<String> {
    let (r, g, b) = parse_hex_rgb(value)?;
    Some(format!("#{r:02x}{g:02x}{b:02x}"))
}
