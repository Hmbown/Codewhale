//! Bounded native replacements for the ASCII fish and jelly silhouettes.
//! These are authored dot poses, not another simulation. The habitat owns
//! clock, travel, brightness, collision and lifetime. Two tiny immutable tables
//! retain their rows so a frame borrows glyphs without allocating strings.
//! Braille remains discrete: half-column / quarter-row motion is not pixels.

use super::pet_sim::BRAILLE_BITS;
use std::sync::LazyLock;

// Fish: 2 directions × 4 tail poses × 2 horizontal × 2 vertical dot offsets.
static FISH: LazyLock<Vec<String>> = LazyLock::new(|| {
    (0..32)
        .map(|index| {
            let dy = index % 2;
            let dx = index / 2 % 2;
            let pose = index / 4 % 4;
            let right = index / 16 != 0;
            let mut dots = Vec::from([(2, 1), (3, 0), (4, 0), (5, 1), (6, 1), (3, 2), (4, 2)]);
            // A small forked tail opens, folds and opens; no blinking eye or flash.
            dots.extend(match pose {
                0 => [(0, 0), (0, 2), (1, 1)],
                1 => [(0, 0), (1, 1), (1, 2)],
                2 => [(1, 0), (0, 1), (1, 2)],
                _ => [(1, 0), (1, 1), (0, 2)],
            });
            if !right {
                for point in &mut dots {
                    point.0 = 6 - point.0;
                }
            }
            raster(&dots, 4, 1, dx, dy).remove(0)
        })
        .collect()
});

// Jelly: 16 bell/tentacle poses × 2 horizontal × 4 vertical dot offsets.
// Eight-by-eight source dots plus offsets fit the existing 5×3-cell habitat.
static JELLY: LazyLock<Vec<[String; 3]>> = LazyLock::new(|| {
    (0..128)
        .map(|index| {
            let dy = index % 4;
            let dx = index / 4 % 2;
            let phase = (index / 8) as f64 / 16.0 * std::f64::consts::TAU;
            let contracted = (1.0 - phase.cos()) * 0.5;
            let inset = usize::from(contracted > 0.55);
            let mut dots = Vec::with_capacity(24);
            // An open contour, never a solid luminous block.
            dots.extend([
                (2, 0),
                (3, 0),
                (4, 0),
                (5, 0),
                (1 + inset, 1),
                (6 - inset, 1),
            ]);
            for x in inset..8 - inset {
                dots.push((x, 2));
            }
            for y in 3..8 {
                // Wave travels down both arms after the bell; columns are offset.
                let lag = (y - 2) as f64 * 0.45 + 0.75;
                let left = (2.0 + (phase - lag).sin() * 0.9).round() as usize;
                let right = (5.0 + (phase - lag - 0.8).sin() * 0.9).round() as usize;
                dots.extend([(left, y), (right, y)]);
            }
            let mut rows = raster(&dots, 5, 3, dx, dy).into_iter();
            std::array::from_fn(|_| rows.next().expect("three jelly rows"))
        })
        .collect()
});

fn raster(
    dots: &[(usize, usize)],
    width: usize,
    height: usize,
    dx: usize,
    dy: usize,
) -> Vec<String> {
    let mut cells = vec![0u8; width * height];
    for &(x, y) in dots {
        let (x, y) = (x + dx, y + dy);
        assert!(
            x < width * 2 && y < height * 4,
            "native pose escaped its habitat"
        );
        cells[y / 4 * width + x / 2] |= BRAILLE_BITS[y % 4][x % 2];
    }
    cells
        .chunks_exact(width)
        .map(|row| {
            row.iter()
                .map(|bits| {
                    if *bits == 0 {
                        ' '
                    } else {
                        char::from_u32(0x2800 + u32::from(*bits)).expect("braille dot mask")
                    }
                })
                .collect()
        })
        .collect()
}

pub(super) fn fish(right: bool, pose: usize, dx: usize, dy: usize) -> &'static str {
    &FISH[usize::from(right) * 16 + pose % 4 * 4 + dx % 2 * 2 + dy % 2]
}

pub(super) fn jelly(pose: usize, dx: usize, dy: usize) -> &'static [String; 3] {
    &JELLY[pose % 16 * 8 + dx % 2 * 4 + dy % 4]
}
