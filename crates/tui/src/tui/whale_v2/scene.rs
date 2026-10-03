//! `scene()` and `braille.js`: one evaluated performance, composed for a
//! view. A surface may simplify geometry and paint less often, but it never
//! classifies tools or advances a second clock — it only reads the Director.

use super::acting::Director;
use super::data::p;
use super::props;
use super::rig::{
    Cmd, Direction, MARK_DIRECTION, Parts, Path, Role, Shape, build, holes_for, rig_pose,
};

#[derive(Clone, Copy, Debug)]
pub struct View {
    /// Allocated square in device-independent pixels (drives optical sizes).
    pub size: f64,
    pub dpr: f64,
    /// 0 is the full contour rig; 2 is the optical-fit small icon.
    pub lod: u8,
    pub dir: Direction,
}

impl View {
    pub fn hero(size: f64, dpr: f64) -> Self {
        Self {
            size,
            dpr,
            lod: 0,
            dir: MARK_DIRECTION,
        }
    }
}

/// `scene()`: calves, whale, props and particles, in paint order.
pub fn scene(director: &Director, view: View) -> Parts {
    scene_posed(director, view, &[])
}

/// `scene()` with a surface's `view.pose` overrides (the cove's gaze),
/// applied before the pod adjustment exactly as the reference merges them.
pub fn scene_posed(director: &Director, view: View, overrides: &[(usize, f64)]) -> Parts {
    let mut pose = director.pose();
    for (param, value) in overrides {
        pose[*param] = *value;
    }
    let pod = director
        .calves
        .iter()
        .map(|c| c.value)
        .fold(f64::NEG_INFINITY, f64::max);
    pose[p::scale] *= 1. - 0.16 * pod;
    pose[p::x] += 10. * pod;
    let mut parts = build(view.dir, &pose, view.lod, view.size, view.dpr);
    let mut calves = Vec::new();
    for (i, calf) in director.calves.iter().enumerate() {
        let vis = calf.value;
        if vis < 0.002 {
            continue;
        }
        let i = i as f64;
        let phase = if director.reduced {
            0.
        } else {
            director.f / 30. * (0.8 + i * 0.13) + i * 2.1
        };
        let mut calf_pose = rig_pose();
        calf_pose[p::x] = -44. - (1. - vis) * 10.;
        calf_pose[p::y] = -30. + i * 30.;
        calf_pose[p::scale] = 0.18 * vis;
        calf_pose[p::fluke] = 6. + phase.sin() * 8.;
        calf_pose[p::lid] = 0.;
        let lod = if view.lod >= 2 { 2 } else { 0 };
        let cp = build(view.dir, &calf_pose, lod, view.size, view.dpr);
        calves.extend(cp.shapes.into_iter().map(|s| Shape {
            id: format!("calf-{}-{}", i as usize + 1, s.id),
            opacity: s.opacity * vis,
            ..s
        }));
    }
    let small = view.lod >= 2;
    let prop_shapes = props::shapes(&pose, &parts, small);
    let whale = std::mem::take(&mut parts.shapes);
    parts.shapes = calves;
    parts.shapes.extend(whale);
    parts.shapes.extend(prop_shapes);
    let particles = if director.reduced {
        director.poster_particles(&parts.anchors)
    } else {
        director.particles.clone()
    };
    parts.shapes.extend(props::particles(&particles, small));
    parts
}

/// Top-to-bottom bit layout of one 2×4 Braille cell.
pub const BITS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

pub(super) fn segments(path: &Path) -> Vec<[f64; 4]> {
    let mut edges = Vec::new();
    let mut pen: Option<[f64; 2]> = None;
    let mut start = [0., 0.];
    let mut line = |pen: &mut Option<[f64; 2]>, q: [f64; 2]| {
        if let Some(from) = *pen {
            edges.push([from[0], from[1], q[0], q[1]]);
        }
        *pen = Some(q);
    };
    for c in path {
        match c {
            Cmd::M(m) => {
                pen = Some(*m);
                start = *m;
            }
            Cmd::C(c) => {
                let from = pen.unwrap_or([0., 0.]);
                for i in 1..=10 {
                    let t = i as f64 / 10.;
                    let u = 1. - t;
                    line(
                        &mut pen,
                        [
                            u * u * u * from[0]
                                + 3. * u * u * t * c[0]
                                + 3. * u * t * t * c[2]
                                + t * t * t * c[4],
                            u * u * u * from[1]
                                + 3. * u * u * t * c[1]
                                + 3. * u * t * t * c[3]
                                + t * t * t * c[5],
                        ],
                    );
                }
            }
            Cmd::Z => line(&mut pen, start),
        }
    }
    edges
}

/// Packed row-major Braille cells, as `render_grid` consumes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<u8>,
}

/// `rasterize()`: even-odd scan conversion of the one-ink scene into dots.
pub fn rasterize(parts: &Parts, cols: usize, rows: usize, cell_aspect: f64) -> Grid {
    let width = cols * 2;
    let height = rows * 4;
    let mut dots = vec![false; width * height];
    let scale = (width as f64 * cell_aspect * 2.).min(height as f64) / 124.;
    let sx = scale / (cell_aspect * 2.);
    let sy = scale;
    let (hw, hh) = (width as f64 / 2., height as f64 / 2.);
    for shape in &parts.shapes {
        if shape.opacity < 0.5 || shape.role == Role::Hole || shape.role == Role::Cutout {
            continue;
        }
        let mut paths = vec![shape.path.clone()];
        paths.extend(holes_for(&shape.id, &parts.shapes, true));
        let edges: Vec<[f64; 4]> = paths
            .iter()
            .flat_map(segments)
            .map(|[x1, y1, x2, y2]| [hw + x1 * sx, hh + y1 * sy, hw + x2 * sx, hh + y2 * sy])
            .collect();
        let mut cross = Vec::new();
        for y in 0..height {
            let cy = y as f64 + 0.5;
            cross.clear();
            for [x1, y1, x2, y2] in &edges {
                if (*y1 > cy) != (*y2 > cy) {
                    cross.push(x1 + (cy - y1) * (x2 - x1) / (y2 - y1));
                }
            }
            cross.sort_by(f64::total_cmp);
            let mut i = 0;
            while i + 1 < cross.len() {
                let left = (cross[i] - 0.5).ceil().max(0.);
                let right = (cross[i + 1] - 0.5).ceil().min(width as f64);
                let mut x = left;
                while x < right {
                    dots[y * width + x as usize] = true;
                    x += 1.;
                }
                i += 2;
            }
        }
    }
    let mut cells = vec![0u8; cols * rows];
    for y in 0..height {
        for x in 0..width {
            if dots[y * width + x] {
                cells[(y / 4) * cols + x / 2] |= BITS[y % 4][x % 2];
            }
        }
    }
    Grid { cols, rows, cells }
}

/// `frame()`: the terminal still at `cols × rows`, via the small rig.
pub fn braille(director: &Director, cols: usize, rows: usize) -> Grid {
    let size = (cols * 2).min(rows * 4) as f64;
    rasterize(
        &scene(
            director,
            View {
                lod: 2,
                ..View::hero(size, 1.)
            },
        ),
        cols,
        rows,
        0.5,
    )
}
