//! `scene()` and `braille.js`: one evaluated performance, composed for a
//! view. A surface may simplify geometry and paint less often, but it never
//! classifies tools or advances a second clock — it only reads the Director.

use super::acting::Director;
use super::data::p;
use super::ink::{self, Fill};
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
    /// The reference switches to the optical small rig below 40 px.
    pub fn fitted(size: f64, dpr: f64) -> Self {
        Self {
            lod: if size < 40. { 2 } else { 0 },
            ..Self::hero(size, dpr)
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
        if value.is_finite()
            && let Some(slot) = pose.get_mut(*param)
        {
            *slot = *value;
        }
    }
    let pod = director
        .calves
        .iter()
        .map(|c| c.value)
        .fold(f64::NEG_INFINITY, f64::max);
    pose[p::scale] *= 1. - 0.16 * pod;
    pose[p::x] += 10. * pod;
    let mut parts = build(view.dir, &pose, view.lod, view.size, view.dpr);
    if parts.shapes.is_empty() {
        return parts;
    }
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

impl Grid {
    /// Canonical allocation limits; invalid public grids are never indexed.
    pub fn is_valid(&self) -> bool {
        valid_viewport(self.cols, self.rows, 0.5)
            && self.cols.checked_mul(self.rows) == Some(self.cells.len())
    }

    /// Convert the evaluated native frame for the existing Whale painter.
    pub fn to_terminal(&self) -> Option<crate::whale::Grid> {
        self.is_valid().then(|| crate::whale::Grid {
            cols: self.cols as u16,
            rows: self.rows as u16,
            cells: self.cells.clone(),
        })
    }

    pub fn text(&self) -> String {
        if !self.is_valid() {
            return String::new();
        }
        self.cells
            .chunks(self.cols)
            .map(|row| {
                row.iter()
                    .map(|b| {
                        if *b == 0 {
                            ' '
                        } else {
                            char::from_u32(0x2800 + u32::from(*b)).unwrap()
                        }
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn hex(&self) -> String {
        if !self.is_valid() {
            return String::new();
        }
        self.cells.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// `rasterize()`: even-odd scan conversion of the one-ink scene into dots.
pub fn rasterize(parts: &Parts, cols: usize, rows: usize, cell_aspect: f64) -> Grid {
    rasterize_with_inks(parts, cols, rows, cell_aspect, None).grid
}

/// Color any evaluated direction with the same byte geometry and native ink.
pub fn rasterize_colored(
    parts: &Parts,
    cols: usize,
    rows: usize,
    cell_aspect: f64,
    dark: bool,
) -> ColoredGrid {
    rasterize_with_inks(parts, cols, rows, cell_aspect, Some(dark))
}

/// The canonical Braille geometry plus one foreground color per cell.
/// Empty dots and the one-ink apertures retain the caller's ground. A cell's
/// majority visible role wins; later paint order breaks ties, and occupied
/// dot colors of that role are averaged. Thus blue keeps its native diagonal
/// ombre while a pointer, pencil or tool can retain its own ink. This is a
/// terminal color approximation, not an alteration of the byte oracle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColoredGrid {
    pub grid: Grid,
    pub colors: Vec<Option<u32>>,
}

impl ColoredGrid {
    /// Paint the live frame through the existing Whale clipping/label path.
    /// The supplied Whale owns the action's localized words. Invalid frames,
    /// ASCII terminals and cramped areas keep that painter's word fallback.
    pub fn paint(
        &self,
        whale: &crate::Whale,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        theme: &crate::Theme,
    ) {
        self.paint_with_contrast(whale, area, buf, theme, 0.0);
    }

    /// Paint native geometry with an optional decorative contrast floor.
    /// On a measured ground, only insufficient native inks are mixed toward
    /// the theme's foreground. The packed dots and canonical color plane stay
    /// unchanged. Use 3.0 for a small terminal companion; zero retains exact
    /// native ink. Unknown grounds and plain-text profiles keep their fallback.
    pub fn paint_with_contrast(
        &self,
        whale: &crate::Whale,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        theme: &crate::Theme,
        minimum: f32,
    ) {
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let valid = self.colors.len() == self.grid.cells.len();
        let terminal =
            valid
                .then(|| self.grid.to_terminal())
                .flatten()
                .unwrap_or(crate::whale::Grid {
                    cols: 0,
                    rows: 0,
                    cells: Vec::new(),
                });
        let fits = !theme.ascii()
            && valid
            && self.grid.is_valid()
            && terminal.cols >= 16
            && terminal.rows >= 8
            && terminal.cols <= area.width
            && terminal.rows < area.height;
        let label_y = if fits {
            area.y + (area.height - terminal.rows - 1) / 2 + terminal.rows
        } else {
            area.y
        };
        // ratatui resets wide-glyph continuation cells, including their
        // backgrounds. Keep the host's grounds while retaining the shared
        // painter's label, clipping and style behavior. Only one row is saved,
        // so a large allocation never copies the whole destination buffer.
        let backgrounds: Vec<_> = (0..area.width)
            .map(|x| buf[(area.x + x, label_y)].bg)
            .collect();
        whale.paint_frame(area, buf, theme, &terminal);
        for (x, background) in (0..area.width).zip(backgrounds) {
            buf[(area.x + x, label_y)].set_bg(background);
        }
        if !fits || !theme.paints_grounds() {
            return;
        }
        let x0 = area.x + (area.width - terminal.cols) / 2;
        let y0 = area.y + (area.height - terminal.rows - 1) / 2;
        for row in 0..terminal.rows {
            for col in 0..terminal.cols {
                let i = usize::from(row) * self.grid.cols + usize::from(col);
                if terminal.cells[i] == 0 {
                    continue;
                }
                if let Some(hex) = self.colors[i] {
                    let rgb = crate::color::rgb(hex);
                    let color = match (theme.depth(), rgb) {
                        (crate::color::ColorDepth::TrueColor, c) => c,
                        (
                            crate::color::ColorDepth::Ansi256,
                            ratatui::style::Color::Rgb(r, g, b),
                        ) => ratatui::style::Color::Indexed(crate::color::rgb_to_ansi256(r, g, b)),
                        _ => continue,
                    };
                    if let Some(cell) = buf.cell_mut((x0 + col, y0 + row)) {
                        let mut ink = color;
                        if minimum.is_finite()
                            && minimum > 0.0
                            && let Some((r, g, b)) = theme
                                .color(crate::Role::Foreground)
                                .and_then(crate::color::resolvable_rgb)
                        {
                            let foreground = ratatui::style::Color::Rgb(r, g, b);
                            for step in 0..=32 {
                                if crate::color::contrast_ratio(ink, cell.bg)
                                    .is_none_or(|ratio| ratio >= minimum.min(7.0))
                                {
                                    break;
                                }
                                let candidate =
                                    crate::color::blend(foreground, rgb, step as f32 / 32.0);
                                ink = match (theme.depth(), candidate) {
                                    (
                                        crate::color::ColorDepth::Ansi256,
                                        ratatui::style::Color::Rgb(r, g, b),
                                    ) => ratatui::style::Color::Indexed(
                                        crate::color::rgb_to_ansi256(r, g, b),
                                    ),
                                    _ => candidate,
                                };
                            }
                        }
                        cell.set_fg(ink);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
struct DotInk {
    role: usize,
    order: usize,
    rgb: [u8; 3],
}

fn valid_viewport(cols: usize, rows: usize, cell_aspect: f64) -> bool {
    (1..=160).contains(&cols)
        && (1..=80).contains(&rows)
        && cell_aspect.is_finite()
        && cell_aspect > 0.
}

fn role_index(role: Role) -> usize {
    match role {
        Role::Body => 0,
        Role::Tool => 1,
        Role::Accent => 2,
        Role::Pointer => 3,
        Role::Water => 4,
        Role::Hole | Role::Cutout => 5,
    }
}

fn dot_ink(role: Role, dark: bool, x: f64, y: f64, order: usize) -> Option<DotInk> {
    let hex = match ink::fill(role, dark)? {
        Fill::Solid(hex) => hex,
        Fill::Gradient([top, bottom]) => {
            let t = ((x + y + 100.) / 200.).clamp(0., 1.);
            let channel = |shift: u32| {
                let a = ((top >> shift) & 255_u32) as f64;
                let b = ((bottom >> shift) & 255_u32) as f64;
                (a + (b - a) * t).round() as u32
            };
            (channel(16) << 16) | (channel(8) << 8) | channel(0)
        }
    };
    Some(DotInk {
        role: role_index(role),
        order,
        rgb: [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8],
    })
}

fn rasterize_with_inks(
    parts: &Parts,
    cols: usize,
    rows: usize,
    cell_aspect: f64,
    dark: Option<bool>,
) -> ColoredGrid {
    if !valid_viewport(cols, rows, cell_aspect) {
        return ColoredGrid {
            grid: Grid {
                cols: 0,
                rows: 0,
                cells: Vec::new(),
            },
            colors: Vec::new(),
        };
    }
    let width = cols * 2;
    let height = rows * 4;
    let mut dots = vec![false; width * height];
    let mut inks = dark.map(|_| vec![None; width * height]);
    let scale = (width as f64 * cell_aspect * 2.).min(height as f64) / 124.;
    let sx = scale / (cell_aspect * 2.);
    let sy = scale;
    let (hw, hh) = (width as f64 / 2., height as f64 / 2.);
    for (order, shape) in parts.shapes.iter().enumerate() {
        if !shape.opacity.is_finite()
            || shape.opacity < 0.5
            || shape.role == Role::Hole
            || shape.role == Role::Cutout
        {
            continue;
        }
        let mut paths = vec![shape.path.clone()];
        paths.extend(holes_for(&shape.id, &parts.shapes, true));
        paths.retain(|path| {
            matches!(path.first(), Some(Cmd::M(_)))
                && path.iter().flat_map(Cmd::coords).all(|v| v.is_finite())
        });
        let edges: Vec<[f64; 4]> = paths
            .iter()
            .flat_map(segments)
            .map(|[x1, y1, x2, y2]| [hw + x1 * sx, hh + y1 * sy, hw + x2 * sx, hh + y2 * sy])
            .filter(|edge| edge.iter().all(|v| v.is_finite()))
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
                    if let (Some(inks), Some(dark)) = (&mut inks, dark) {
                        inks[y * width + x as usize] =
                            dot_ink(shape.role, dark, (x + 0.5 - hw) / sx, (cy - hh) / sy, order);
                    }
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
    let colors = if let Some(inks) = inks {
        let mut colors = vec![None; cols * rows];
        for row in 0..rows {
            for col in 0..cols {
                let mut counts = [0u32; 6];
                let mut latest = [0usize; 6];
                let mut sums = [[0u32; 3]; 6];
                for dy in 0..4 {
                    for dx in 0..2 {
                        if let Some(ink) = inks[(row * 4 + dy) * width + col * 2 + dx] {
                            counts[ink.role] += 1;
                            latest[ink.role] = latest[ink.role].max(ink.order);
                            for (sum, channel) in sums[ink.role].iter_mut().zip(ink.rgb) {
                                *sum += u32::from(channel);
                            }
                        }
                    }
                }
                let role = (0..6)
                    .max_by_key(|role| (counts[*role], latest[*role]))
                    .unwrap();
                if counts[role] > 0 {
                    let c = sums[role].map(|sum| (sum + counts[role] / 2) / counts[role]);
                    colors[row * cols + col] = Some((c[0] << 16) | (c[1] << 8) | c[2]);
                }
            }
        }
        colors
    } else {
        Vec::new()
    };
    ColoredGrid {
        grid: Grid { cols, rows, cells },
        colors,
    }
}

/// `frame()`: the terminal still at `cols × rows`, via the small rig.
pub fn braille(director: &Director, cols: usize, rows: usize) -> Grid {
    if !valid_viewport(cols, rows, 0.5) {
        return Grid {
            cols: 0,
            rows: 0,
            cells: Vec::new(),
        };
    }
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

/// A live terminal frame with the character's native light/dark palette.
pub fn colored_braille(director: &Director, cols: usize, rows: usize, dark: bool) -> ColoredGrid {
    if !valid_viewport(cols, rows, 0.5) {
        return rasterize_with_inks(&Parts::default(), cols, rows, 0.5, Some(dark));
    }
    let size = (cols * 2).min(rows * 4) as f64;
    let parts = scene(
        director,
        View {
            lod: 2,
            ..View::hero(size, 1.)
        },
    );
    rasterize_with_inks(&parts, cols, rows, 0.5, Some(dark))
}
