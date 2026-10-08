//! The native port of `rig.js::resolveLook`, shared by the GPUI painter and
//! legibility metrics. The web boundary check guards parity with the reference.

use super::rig::Role;

/// How a named shape is filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    Solid(u32),
    /// The body's two-stop gradient at 135°: first stop top-left.
    Gradient([u32; 2]),
}

/// The light eye/throat apertures the color whale paints.
pub const APERTURE: u32 = 0xfaf8f5;

/// The contrast rim a surface over an unknown backdrop paints behind the
/// silhouette (see [`keyline_stroke`]): a deep-sea ink, darker than every
/// body stop, so the edge holds on paper, mid-grey and photographs while
/// the body itself holds on dark wallpaper. A light keyline on paper
/// wallpaper measures 1.00:1, so this stays dark.
pub const RIM: u32 = 0x08101c;
/// The rim's opacity. Solid enough to carry 3:1 against mid-grey.
pub const RIM_ALPHA: f64 = 0.95;

/// The light hairline one device pixel outside the rim: the cursor trick.
/// The dark rim alone sinks into dark and mid-tone wallpaper (a worst median
/// edge of 2.2:1 over real photographs); a light line alone vanishes on
/// paper. Together one of the two always separates the form. A crisp
/// stroke, never blurred: it is a keyline, not a glow.
pub const HALO: u32 = 0xfaf8f5;
/// The hairline's opacity.
pub const HALO_ALPHA: f64 = 0.85;
/// Below this side, in logical pixels, the hairline fills the C's inner
/// opening and reads as a grey badge, so it is left off. The floating
/// companion, the one carrier over an unknown desktop, renders far above it.
pub const HALO_MIN_SIDE: f64 = 32.;

/// The body gradient stops for a theme.
pub fn body(dark: bool) -> [u32; 2] {
    if dark {
        [0x3594d8, 0x2b70d5]
    } else {
        [0x1e8fd8, 0x0b48bb]
    }
}

/// The color fill for a shape role, or `None` for roles that never paint
/// on their own (compound cutouts).
pub fn fill(role: Role, dark: bool) -> Option<Fill> {
    Some(match role {
        Role::Cutout => return None,
        Role::Hole => Fill::Solid(APERTURE),
        Role::Pointer => Fill::Solid(if dark { 0x66d6de } else { 0x147888 }),
        Role::Accent => Fill::Solid(0xd2a34e),
        Role::Water => Fill::Solid(body(dark)[0]),
        Role::Tool => Fill::Solid(if dark { 0xfaf8f5 } else { 0x202123 }),
        Role::Body => Fill::Gradient(body(dark)),
    })
}

/// Centered stroke width, in logical pixels, for a window `scale`.
///
/// The fill covers the inner half of a centered stroke, so the width is
/// twice the visible device-pixel count. That count is `round(scale)` and
/// never less than one, which is one device pixel at 1× and one logical
/// pixel at an integer scale. A non-positive or non-finite scale is 1×.
pub fn keyline_stroke(scale: f64) -> f64 {
    let scale = if scale.is_finite() && scale > 0. {
        scale
    } else {
        1.
    };
    2. * scale.round().max(1.) / scale
}

/// Centered stroke width for the light hairline: twice the rim's, so after
/// the rim paints over its inner half exactly one more device pixel shows
/// outside the dark one.
pub fn halo_stroke(scale: f64) -> f64 {
    2. * keyline_stroke(scale)
}

#[cfg(test)]
mod tests {
    use super::keyline_stroke;

    #[test]
    fn keyline_stroke_is_one_visible_device_pixel_on_the_grid() {
        assert_eq!(keyline_stroke(1.), 2.);
        assert_eq!(keyline_stroke(2.), 2.);
        assert_eq!(keyline_stroke(3.), 2.);
        assert!((keyline_stroke(1.25) - 1.6).abs() < 1e-9);
        for scale in [0.5, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0] {
            let visible = keyline_stroke(scale) / 2. * scale;
            let want = scale.round().max(1.);
            assert!(
                (visible - want).abs() < 1e-9,
                "scale {scale}: visible {visible} device px, want {want}"
            );
        }
        assert_eq!(keyline_stroke(0.), keyline_stroke(1.));
        assert_eq!(keyline_stroke(-2.), keyline_stroke(1.));
        assert_eq!(keyline_stroke(f64::NAN), keyline_stroke(1.));
    }
}
