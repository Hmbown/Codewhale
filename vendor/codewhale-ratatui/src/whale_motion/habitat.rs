//! Port of `habitat.js`: the companion's little cove for large surfaces.
//!
//! The cove is scenery on the Director's own clock (`f`); it never advances
//! a clock of its own and never changes presence or the acting state. While
//! the whale rests, its gaze may follow the pointer over the water; a tap
//! leaves a decorative ripple. Reduced motion paints the still cove and no
//! gaze. Compact icons never carry it.

use super::acting::Director;
use super::data::{Act, p};
use super::math::{clamp, lerp};
use super::rig::{Path, ellipse, parse};
use std::collections::VecDeque;

/// One filled cove layer: its paths are filled together with the even-odd
/// rule, in the cove's own palette.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub paths: Vec<Path>,
    pub color: u32,
    pub alpha: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ripple {
    x: f64,
    y: f64,
    at: f64,
}

/// A ripple lasts this many frames.
const RIPPLE_LIFE: f64 = 78.;

#[derive(Clone, Debug, Default)]
pub struct Cove {
    target: (f64, f64),
    gaze: (f64, f64),
    last_frame: f64,
    ripples: VecDeque<Ripple>,
}

struct Palette {
    air: u32,
    shore: u32,
    water: u32,
    deep: u32,
    reed: u32,
    glint: u32,
}

fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            air: 0x243c4b,
            shore: 0x426878,
            water: 0x254e64,
            deep: 0x1d4055,
            reed: 0x4c8490,
            glint: 0x699cab,
        }
    } else {
        Palette {
            air: 0xedf5f5,
            shore: 0xc4d8d7,
            water: 0xd6eaed,
            deep: 0xb9dce3,
            reed: 0x86b5b6,
            glint: 0xf7fcfb,
        }
    }
}

fn layer(d: String, color: u32, alpha: f64) -> Layer {
    Layer {
        paths: vec![parse(&d)],
        color,
        alpha,
    }
}

impl Cove {
    /// The pointer is over the water at design coordinates `x`, `y`.
    pub fn observe(&mut self, x: f64, y: f64) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        self.target = (clamp(x / 62., -1., 1.), clamp(y / 62., -1., 1.));
    }

    /// The pointer left the cove.
    pub fn leave(&mut self) {
        self.target = (0., 0.);
    }

    /// A decorative tap on the water; it cannot change Engine state.
    pub fn tap(&mut self, x: f64, y: f64, director: &Director) {
        if !x.is_finite() || !y.is_finite() || !director.f.is_finite() {
            return;
        }
        let point = Ripple {
            x: clamp(x, -42., 42.),
            y: clamp(y, 25., 48.),
            at: director.f,
        };
        self.ripples.push_back(point);
        if self.ripples.len() > 3 {
            self.ripples.pop_front();
        }
        self.observe(point.x, point.y);
    }

    /// Forget every ripple and gaze: a new session starts in a still cove.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn ripples(&self) -> usize {
        self.ripples.len()
    }

    /// `pose(d)`: absolute overrides for the whale's pose while it rests,
    /// easing back to the Director's own pose in every other state.
    /// Idempotent within a frame: two surfaces painting the same frame see
    /// the same gaze.
    pub fn pose(&mut self, d: &Director) -> Vec<(usize, f64)> {
        let dt = clamp((d.f - self.last_frame) / 30., 0., 0.25);
        self.last_frame = d.f;
        if d.reduced {
            self.gaze = (0., 0.);
            return Vec::new();
        }
        let active = d.acting == Act::Rest;
        let w = 1. - (-dt * 4.).exp();
        let (tx, ty) = if active { self.target } else { (0., 0.) };
        self.gaze = (lerp(self.gaze.0, tx, w), lerp(self.gaze.1, ty, w));
        let pose = d.pose();
        vec![
            (p::lookX, pose[p::lookX] + self.gaze.0 * 0.85),
            (p::lookY, pose[p::lookY] + self.gaze.1 * 0.65),
            (p::head, pose[p::head] - self.gaze.1 * 5.),
            (p::rot, pose[p::rot] + self.gaze.0 * 2.),
        ]
    }

    /// `behind()`: sky, shore, water, reeds, glints and rising bubbles.
    pub fn behind(&self, d: &Director, dark: bool) -> Vec<Layer> {
        let c = palette(dark);
        let t = if d.reduced { 0. } else { d.f / 30. };
        let w = (t * 0.55).sin() * 1.4;
        let mut out = vec![
            layer(
                "M-59 7 C-63 -29 -37 -58 0 -59 C35 -62 60 -41 61 -6 C66 31 48 57 13 60 C-22 67 -54 48 -59 7 Z".into(),
                c.air,
                1.,
            ),
            layer(
                "M-61 29 C-61 13 -56 5 -51 11 C-46 16 -48 22 -43 25 C-39 27 -37 30 -35 34 C-45 37 -55 36 -61 29 Z".into(),
                c.shore,
                1.,
            ),
            layer(
                format!(
                    "M-60 {} C-37 {} -14 {} 6 21 C26 {} 43 {} 61 18 C61 42 42 59 12 61 C-19 65 -51 47 -60 {} Z",
                    21. + w,
                    15. + w,
                    23. - w,
                    18. + w,
                    15. - w,
                    21. + w
                ),
                c.water,
                1.,
            ),
            layer(
                "M-55 40 C-29 49 -9 42 12 45 C32 49 44 39 56 36 C44 56 21 63 -3 61 C-25 59 -44 49 -55 40 Z".into(),
                c.deep,
                0.7,
            ),
        ];
        for (x, h, phase) in [
            (-54., 29., 0.),
            (-49., 22., 1.),
            (53., 19., 2.),
            (57., 26., 3.),
        ] {
            let sway = (t * 0.65 + phase).sin() * 2.;
            out.push(layer(
                format!(
                    "M{x} 52 C{} 42 {} {} {} {} C{} {} {} 40 {} 52 Z",
                    x - 2.,
                    x + sway - 5.,
                    52. - h + 5.,
                    x + sway,
                    52. - h,
                    x + sway - 1.,
                    52. - h + 9.,
                    x + 3.,
                    x + 1.
                ),
                c.reed,
                0.8,
            ));
        }
        out.push(layer(
            format!(
                "M-53 33 C-45 {} -40 {} -34 33 C-42 32 -47 34 -53 33 Z",
                30. + w,
                30. + w
            ),
            c.glint,
            0.7,
        ));
        out.push(layer(
            format!(
                "M35 30 C43 {} 48 {} 54 30 C47 30 42 32 35 30 Z",
                28. - w,
                28. - w
            ),
            c.glint,
            0.65,
        ));
        // Water bubbles live low in the cove, away from the thinking cloud.
        for i in 0..3 {
            let fi = i as f64;
            let phase = if d.reduced {
                0.35
            } else {
                (t / (7. + fi * 2.) + fi * 0.31) % 1.
            };
            let x = [-47., 47., 39.][i] + (t * 0.6 + fi).sin() * 1.2;
            let y = 55. - phase * 24.;
            let r = 0.55 + phase * 0.5;
            out.push(Layer {
                paths: vec![ellipse(x, y, r, r)],
                color: c.glint,
                alpha: (phase * std::f64::consts::PI).sin() * 0.65,
            });
        }
        out
    }

    /// `front()`: the near current below the face, and tap ripples.
    pub fn front(&self, d: &Director, dark: bool) -> Vec<Layer> {
        let c = palette(dark);
        let t = if d.reduced { 0. } else { d.f / 30. };
        let w = (t * 0.45).sin() * 2.;
        let mut out = vec![
            layer(
                format!(
                    "M-40 51 C-23 {} -9 {} 8 51 C21 {} 30 49 40 47 C26 54 16 54 5 54 C-11 57 -26 50 -40 51 Z",
                    47. + w,
                    54. - w,
                    48. + w
                ),
                c.glint,
                0.6,
            ),
            layer(
                format!(
                    "M-23 58 C-8 {} 8 60 21 57 C9 62 -8 60 -23 58 Z",
                    56. - w * 0.3
                ),
                c.glint,
                0.55,
            ),
        ];
        for ripple in &self.ripples {
            let age = if d.reduced { 12. } else { d.f - ripple.at };
            if !(0. ..=RIPPLE_LIFE).contains(&age) {
                continue;
            }
            for i in 0..2 {
                let a = age - i as f64 * 9.;
                if a < 0. {
                    continue;
                }
                let radius = 2. + a * 0.19;
                out.push(Layer {
                    paths: vec![
                        ellipse(ripple.x, ripple.y, radius, radius * 0.25),
                        ellipse(
                            ripple.x,
                            ripple.y,
                            (radius - 0.7).max(0.),
                            (radius * 0.25 - 0.55).max(0.),
                        ),
                    ],
                    color: c.glint,
                    alpha: (1. - a / RIPPLE_LIFE) * 0.72,
                });
            }
        }
        out
    }

    /// Whether a ripple is still spreading at the Director's frame.
    pub fn rippling(&self, d: &Director) -> bool {
        !d.reduced
            && self
                .ripples
                .iter()
                .any(|r| (0. ..=RIPPLE_LIFE).contains(&(d.f - r.at)))
    }
}
