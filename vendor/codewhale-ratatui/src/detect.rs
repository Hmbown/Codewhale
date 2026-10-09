//! Light and dark detection.
//!
//! Extracted from the Codewhale engine's `crates/palette/src/detect.rs`
//! (`Hmbown/CodeWhale` `58b1dd3dd`, last changed in `c6416b203`). Detection
//! returns evidence, not just a verdict: [`TerminalBackground`] carries the
//! color we actually measured and [`BackgroundSource`] records how we learned
//! it. Only a measurement (OSC 11) or the terminal's own hint (`COLORFGBG`)
//! yields a known [`Appearance`]; the macOS system setting describes the OS,
//! not the terminal, so it is kept as a hint and never trusted for painting.

#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::OnceLock;

use ratatui::style::Color;

use crate::color::relative_luminance;
use crate::osc11;

/// Whether the terminal's ground is light or dark, as far as we know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Appearance {
    Dark,
    Light,
    /// Nothing measured the ground. Components paint no grounds and use the
    /// terminal's own named colors, which were chosen for its own ground.
    Unknown,
}

/// How the terminal background was learned, strongest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundSource {
    /// The terminal answered an OSC 11 query with its background color.
    Osc11,
    /// `COLORFGBG` was set. Carries a palette index, not an RGB value.
    ColorFgBg,
    /// macOS `AppleInterfaceStyle`. Describes the system, not the terminal: a
    /// dark-mode Mac can run a light terminal profile.
    MacOsAppearance,
    /// No evidence at all.
    Unknown,
}

/// What we know about the surface we are drawing onto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalBackground {
    polarity: Appearance,
    color: Option<Color>,
    source: BackgroundSource,
}

impl TerminalBackground {
    #[must_use]
    pub const fn new(polarity: Appearance, color: Option<Color>, source: BackgroundSource) -> Self {
        Self {
            polarity,
            color,
            source,
        }
    }

    /// No evidence.
    #[must_use]
    pub const fn unknown() -> Self {
        Self::new(Appearance::Unknown, None, BackgroundSource::Unknown)
    }

    /// The appearance components may paint for. `Unknown` unless the
    /// terminal itself told us (OSC 11 or `COLORFGBG`).
    #[must_use]
    pub const fn appearance(&self) -> Appearance {
        match self.source {
            BackgroundSource::Osc11 | BackgroundSource::ColorFgBg => self.polarity,
            BackgroundSource::MacOsAppearance | BackgroundSource::Unknown => Appearance::Unknown,
        }
    }

    /// Best guess including the macOS system setting. Use it to pick a
    /// default theme, never to decide contrast.
    #[must_use]
    pub const fn hint(&self) -> Appearance {
        self.polarity
    }

    /// The measured background, when a source supplied one.
    #[must_use]
    pub const fn color(&self) -> Option<Color> {
        self.color
    }

    #[must_use]
    pub const fn source(&self) -> BackgroundSource {
        self.source
    }
}

/// Luminance at which black and white text have equal contrast against a
/// surface: `(L+0.05)/0.05 == 1.05/(L+0.05)`. Above it a surface is light.
const LIGHT_SURFACE_LUMINANCE: f32 = 0.179_129_5;

/// Classify a background color as light or dark by relative luminance, or
/// `None` when the terminal owns the color's RGB.
#[must_use]
pub fn appearance_for_background(color: Color) -> Option<Appearance> {
    let luminance = relative_luminance(color)?;
    Some(if luminance > LIGHT_SURFACE_LUMINANCE {
        Appearance::Light
    } else {
        Appearance::Dark
    })
}

/// The background segment of `COLORFGBG`: the last numeric field.
fn colorfgbg_index(value: &str) -> Option<u16> {
    value
        .split(';')
        .rev()
        .find_map(|part| part.parse::<u16>().ok())
}

/// Parse `COLORFGBG`. Indices 0-15 are remapped by the terminal profile, so
/// they yield a polarity without a color (>= 8 means a light profile);
/// indices >= 16 are fixed by xterm and resolve exactly.
fn colorfgbg_background(value: &str) -> Option<(Appearance, Option<Color>)> {
    let index = colorfgbg_index(value)?;
    if let Ok(index) = u8::try_from(index)
        && index >= 16
        && let Some(appearance) = appearance_for_background(Color::Indexed(index))
    {
        return Some((appearance, Some(Color::Indexed(index))));
    }
    Some((
        if index >= 8 {
            Appearance::Light
        } else {
            Appearance::Dark
        },
        None,
    ))
}

/// Combine the available evidence. Pure, so every branch is testable without
/// a terminal. A measured color beats a palette index, which beats an OS
/// setting, which beats nothing.
#[must_use]
pub fn resolve_terminal_background(
    osc11_rgb: Option<(u8, u8, u8)>,
    colorfgbg: Option<&str>,
    macos_fallback: Option<Appearance>,
) -> TerminalBackground {
    if let Some((r, g, b)) = osc11_rgb {
        let color = Color::Rgb(r, g, b);
        if let Some(appearance) = appearance_for_background(color) {
            return TerminalBackground::new(appearance, Some(color), BackgroundSource::Osc11);
        }
    }
    if let Some((appearance, color)) = colorfgbg.and_then(colorfgbg_background) {
        return TerminalBackground::new(appearance, color, BackgroundSource::ColorFgBg);
    }
    if let Some(appearance) = macos_fallback {
        return TerminalBackground::new(appearance, None, BackgroundSource::MacOsAppearance);
    }
    TerminalBackground::unknown()
}

static TERMINAL_BACKGROUND: OnceLock<TerminalBackground> = OnceLock::new();

/// The detected background without querying the terminal. Returns the probed
/// result once [`probe_terminal_background`] has run; before that it answers
/// from the environment and does not cache, so an early caller cannot lock in
/// an answer the probe would have improved.
#[must_use]
pub fn terminal_background() -> TerminalBackground {
    if let Some(background) = TERMINAL_BACKGROUND.get() {
        return *background;
    }
    resolve_terminal_background(
        None,
        std::env::var("COLORFGBG").ok().as_deref(),
        detect_macos_appearance(),
    )
}

/// Query the terminal (OSC 11) and cache the result for the process.
///
/// Call once, after raw mode is enabled and before the event loop reads
/// stdin; see [`osc11::query_terminal_background`]. Replay the user's
/// type-ahead afterwards with [`osc11::take_carried_type_ahead`].
pub fn probe_terminal_background() -> TerminalBackground {
    if let Some(background) = TERMINAL_BACKGROUND.get() {
        return *background;
    }
    let background = resolve_terminal_background(
        osc11::query_terminal_background(osc11::OSC11_QUERY_TIMEOUT),
        std::env::var("COLORFGBG").ok().as_deref(),
        detect_macos_appearance(),
    );
    *TERMINAL_BACKGROUND.get_or_init(|| background)
}

/// The macOS system setting, read once per process. It is only a hint (see
/// [`TerminalBackground::appearance`]), and reading it starts a process, so
/// a host calling [`crate::Theme::detect`] every frame must not pay for it
/// every frame.
#[cfg(target_os = "macos")]
fn detect_macos_appearance() -> Option<Appearance> {
    static MACOS: OnceLock<Option<Appearance>> = OnceLock::new();
    *MACOS.get_or_init(read_macos_appearance)
}

#[cfg(target_os = "macos")]
fn read_macos_appearance() -> Option<Appearance> {
    let output = Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .ok()?;
    if output.status.success() {
        Some(appearance_from_apple_interface_style(
            &String::from_utf8_lossy(&output.stdout),
        ))
    } else {
        Some(Appearance::Light)
    }
}

#[cfg(not(target_os = "macos"))]
fn detect_macos_appearance() -> Option<Appearance> {
    None
}

#[cfg(any(target_os = "macos", test))]
fn appearance_from_apple_interface_style(value: &str) -> Appearance {
    if value.trim().eq_ignore_ascii_case("dark") {
        Appearance::Dark
    } else {
        Appearance::Light
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc11_measurement_beats_every_hint() {
        let bg = resolve_terminal_background(
            Some((0xfa, 0xf8, 0xf5)),
            Some("15;0"),
            Some(Appearance::Dark),
        );
        assert_eq!(bg.appearance(), Appearance::Light);
        assert_eq!(bg.source(), BackgroundSource::Osc11);
        assert_eq!(bg.color(), Some(Color::Rgb(0xfa, 0xf8, 0xf5)));
    }

    #[test]
    fn colorfgbg_yields_polarity_and_exact_color_only_when_fixed() {
        let light = resolve_terminal_background(None, Some("0;15"), None);
        assert_eq!(light.appearance(), Appearance::Light);
        assert_eq!(light.color(), None);
        let dark = resolve_terminal_background(None, Some("15;0"), None);
        assert_eq!(dark.appearance(), Appearance::Dark);
        let fixed = resolve_terminal_background(None, Some("7;234"), None);
        assert_eq!(fixed.appearance(), Appearance::Dark);
        assert_eq!(fixed.color(), Some(Color::Indexed(234)));
    }

    #[test]
    fn macos_setting_is_a_hint_not_an_appearance() {
        let bg = resolve_terminal_background(None, None, Some(Appearance::Dark));
        assert_eq!(bg.hint(), Appearance::Dark);
        assert_eq!(bg.appearance(), Appearance::Unknown);
        assert_eq!(
            resolve_terminal_background(None, None, None).appearance(),
            Appearance::Unknown
        );
    }

    #[test]
    fn apple_interface_style_parses() {
        assert_eq!(
            appearance_from_apple_interface_style("Dark\n"),
            Appearance::Dark
        );
        assert_eq!(appearance_from_apple_interface_style(""), Appearance::Light);
    }
}
