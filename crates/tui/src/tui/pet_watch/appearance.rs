//! Presentation preferences never enter the simulation or its replay journal.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Appearance {
    pub background: [u8; 3],
    pub background_top: [u8; 3],
    pub particle: [u8; 3],
    pub event_colors: bool,
    pub brightness: f64,
    pub dot_scale: f64,
    pub glow: f64,
    pub environment: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            background: [8, 15, 21],
            background_top: [24, 45, 56],
            particle: [135, 221, 219],
            event_colors: true,
            brightness: 1.25,
            dot_scale: 1.0,
            glow: 0.5,
            environment: true,
        }
    }
}
impl Appearance {
    pub fn valid(&self) -> bool {
        self.brightness.is_finite()
            && (0.25..=2.0).contains(&self.brightness)
            && self.dot_scale.is_finite()
            && (0.65..=1.8).contains(&self.dot_scale)
            && self.glow.is_finite()
            && (0.0..=1.0).contains(&self.glow)
    }
}
