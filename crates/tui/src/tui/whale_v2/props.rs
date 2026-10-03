//! Port of `props.js`: flat physical props and particles, placed at rig
//! anchors so a held prop follows the whale under any transform.

use super::data::{Pose, p};
use super::math::{D2R, clamp};
use super::rig::{Cmd, Parts, Path, Point, Role, Shape, ellipse, parse};
use std::sync::OnceLock;

fn poly(pts: &[[f64; 2]]) -> Path {
    let mut a = vec![Cmd::M(pts[0])];
    for i in 1..=pts.len() {
        let p = pts[(i - 1) % pts.len()];
        let q = pts[i % pts.len()];
        a.push(Cmd::C([p[0], p[1], q[0], q[1], q[0], q[1]]));
    }
    a.push(Cmd::Z);
    a
}

pub fn transform(path: &Path, x: f64, y: f64, a: f64, s: f64) -> Path {
    let (c, sn) = (a.cos(), a.sin());
    path.iter()
        .map(|cmd| {
            let src = cmd.coords();
            let mut out = [0.; 6];
            let mut i = 0;
            while i < src.len() {
                out[i] = x + (src[i] * c - src[i + 1] * sn) * s;
                out[i + 1] = y + (src[i] * sn + src[i + 1] * c) * s;
                i += 2;
            }
            match cmd {
                Cmd::M(_) => Cmd::M([out[0], out[1]]),
                Cmd::C(_) => Cmd::C(out),
                Cmd::Z => Cmd::Z,
            }
        })
        .collect()
}

struct Library {
    page: Path,
    fold: Path,
    rules: [Path; 2],
    pad: Path,
    binding: Path,
    pad_lines: [Path; 4],
    pencil: Path,
    pencil_tip: Path,
    lens: Path,
    lens_handle: Path,
    wrench: Path,
    spyglass: Path,
    spyglass_end: Path,
    cursor: Path,
    links: [Path; 2],
    jet: Path,
    drop: Path,
    water_back: Path,
    water_front: Path,
    splashes: [Path; 2],
    particle_drop: Path,
    cloud: Path,
    bubble: Path,
}

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let splash = parse("M0 0 C-2 -8 -8 -12 -11 -15 C-5 -15 2 -8 3 0 C2 2 1 2 0 0 Z");
        let mirror = |side: f64| -> Path {
            splash
                .iter()
                .map(|c| c.map(|i, v| if i % 2 == 1 { v } else { -side * v }))
                .collect()
        };
        let link = |side: f64| -> Path {
            let mut path = ellipse(side * 2.3, side * 5.5, 4.5, 8.);
            path.extend(ellipse(side * 2.3, side * 5.5, 2.1, 5.3));
            path
        };
        let mut lens = ellipse(0., 0., 9., 9.);
        lens.extend(ellipse(0., 0., 5.8, 5.8));
        Library {
            page: parse("M-9 -13 C-5 -14 0 -14 4 -13 C6 -11 7 -9 9 -8 C9 -1 9 6 8 13 C3 12 -3 13 -9 14 C-8 5 -8 -4 -9 -13 Z"),
            fold: poly(&[[4., -13.], [4., -8.], [9., -8.]]),
            rules: [0., 1.].map(|i| {
                poly(&[
                    [-5., -4. + i * 6.],
                    [5., -4. + i * 6.],
                    [5., -2. + i * 6.],
                    [-5., -2. + i * 6.],
                ])
            }),
            pad: poly(&[[-11., -5.], [11., -5.], [11., 5.], [-11., 5.]]),
            binding: poly(&[[-8.5, -3.], [8.5, -3.], [8.5, -1.5], [-8.5, -1.5]]),
            pad_lines: [0., 1., 2., 3.].map(|i| {
                poly(&[
                    [-8. + i * 4., 0.],
                    [-6. + i * 4., 0.],
                    [-6. + i * 4., 1.2],
                    [-8. + i * 4., 1.2],
                ])
            }),
            pencil: poly(&[[-2., -5.], [2., -5.], [2., 6.], [0., 10.], [-2., 6.]]),
            pencil_tip: poly(&[[-1.3, 6.5], [1.3, 6.5], [0., 10.]]),
            lens,
            lens_handle: poly(&[[-1.7, 7.], [1.7, 7.], [1.7, 17.], [-1.7, 17.]]),
            wrench: poly(&[
                [-2., 9.],
                [-2., -8.],
                [-6., -11.],
                [-6., -18.],
                [-3., -21.],
                [-3., -14.],
                [3., -14.],
                [3., -21.],
                [6., -18.],
                [6., -11.],
                [2., -8.],
                [2., 9.],
                [5., 12.],
                [5., 17.],
                [2., 20.],
                [2., 14.],
                [-2., 14.],
                [-2., 20.],
                [-5., 17.],
                [-5., 12.],
            ]),
            spyglass: poly(&[
                [0., -2.5],
                [6., -2.5],
                [6., -3.5],
                [13., -3.5],
                [13., -4.5],
                [19., -4.5],
                [19., 4.5],
                [13., 4.5],
                [13., 3.5],
                [6., 3.5],
                [6., 2.5],
                [0., 2.5],
            ]),
            spyglass_end: poly(&[[17., -4.5], [20., -4.5], [20., 4.5], [17., 4.5]]),
            cursor: poly(&[[-8., -12.], [10., 1.6], [1.6, 2.8], [-2.2, 10.4]]),
            links: [link(-1.), link(1.)],
            jet: parse("M-2 0 C-3 -12 -9 -28 -17 -29 C-22 -29 -23 -24 -19 -21 C-25 -21 -27 -28 -23 -32 C-16 -39 -5 -32 0 -16 C5 -33 16 -38 23 -31 C27 -26 23 -20 19 -21 C23 -25 20 -29 16 -28 C8 -26 4 -12 2 0 C1 1 -1 1 -2 0 Z"),
            drop: parse("M0 -3 C3 0 3 3 0 3 C-3 3 -3 0 0 -3 Z"),
            water_back: parse("M-43 0 C-28 -7 -12 -4 0 -1 C14 2 28 -7 43 -2 C29 -1 15 7 0 3 C-15 -1 -28 -3 -43 0 Z"),
            water_front: parse("M-29 0 C-11 -2 8 4 28 0 C13 8 -11 4 -29 0 Z"),
            splashes: [mirror(-1.), mirror(1.)],
            particle_drop: parse("M0 -2 C1 -1 2 1 0 2 C-2 1 -1 -1 0 -2 Z"),
            cloud: parse("M-5 1 C-8 0 -7 -4 -4 -4 C-3 -8 2 -8 4 -5 C8 -5 9 0 5 2 C4 6 -3 6 -5 1 Z"),
            bubble: ellipse(0., 0., 1., 1.),
        }
    })
}

fn visible(v: f64) -> f64 {
    clamp(if v.is_nan() { 0. } else { v }, 0., 1.)
}

/// `shapes()`: every prop, in the reference's paint order.
pub fn shapes(q: &Pose, parts: &Parts, small_view: bool) -> Vec<Shape> {
    let lib = library();
    let boost = if small_view { 1.15 } else { 1. } * parts.unit_scale;
    let mut out = Vec::with_capacity(32);
    let mut add = |id: String, path: &Path, role: Role, x: f64, y: f64, a: f64, s: f64, v: f64| {
        out.push(Shape {
            id,
            path: transform(path, x, y, a, s),
            role,
            opacity: visible(v),
        });
    };
    let hand = parts.anchors.fin_tip;
    let angle = parts.anchors.fin_ang;
    let offset = |anchor: Point, x: f64, y: f64, a: f64| Point {
        x: anchor.x + (x * a.cos() - y * a.sin()) * boost,
        y: anchor.y + (x * a.sin() + y * a.cos()) * boost,
    };
    let pc = offset(hand, 22. + q[p::pageDX], -31. + q[p::pageDY], angle);
    let pa = angle - 0.08 + q[p::pageRot] * D2R;
    add(
        "prop-page".into(),
        &lib.page,
        Role::Tool,
        pc.x,
        pc.y,
        pa,
        boost,
        q[p::page],
    );
    add(
        "prop-page-fold".into(),
        &lib.fold,
        Role::Cutout,
        pc.x,
        pc.y,
        pa,
        boost,
        q[p::page],
    );
    for (i, rule) in lib.rules.iter().enumerate() {
        add(
            format!("prop-page-rule-{i}"),
            rule,
            Role::Cutout,
            pc.x,
            pc.y,
            pa,
            boost,
            q[p::page],
        );
    }
    add(
        "prop-page-turn".into(),
        &lib.page,
        Role::Tool,
        pc.x + 2. * boost,
        pc.y,
        pa,
        boost * (q[p::pageFlip] * std::f64::consts::PI / 2.).sin() * 0.75,
        q[p::page] * q[p::pageFlip],
    );
    let pad = offset(hand, 21., -16., angle);
    add(
        "prop-pad".into(),
        &lib.pad,
        Role::Tool,
        pad.x,
        pad.y,
        angle - 0.05,
        boost,
        q[p::pad],
    );
    add(
        "prop-pad-binding".into(),
        &lib.binding,
        Role::Cutout,
        pad.x,
        pad.y,
        angle - 0.05,
        boost,
        q[p::pad],
    );
    for (i, line) in lib.pad_lines.iter().enumerate() {
        add(
            format!("prop-pad-line-{i}"),
            line,
            Role::Cutout,
            pad.x,
            pad.y,
            angle - 0.05,
            boost,
            q[p::pad] * clamp(q[p::padLines] - i as f64, 0., 1.),
        );
    }
    let pen = offset(
        hand,
        16.4 + q[p::scribX] * 0.35,
        -23.3 + q[p::scribY] * 0.35,
        angle,
    );
    add(
        "prop-pencil".into(),
        &lib.pencil,
        Role::Accent,
        pen.x,
        pen.y,
        angle - 0.48,
        boost,
        q[p::pencil],
    );
    add(
        "prop-pencil-tip".into(),
        &lib.pencil_tip,
        Role::Tool,
        pen.x,
        pen.y,
        angle - 0.48,
        boost,
        q[p::pencil],
    );
    let la = angle + 0.55;
    let lc = offset(hand, 3., -42., la);
    add(
        "prop-lens".into(),
        &lib.lens,
        Role::Tool,
        lc.x,
        lc.y,
        la,
        boost,
        q[p::lens],
    );
    add(
        "prop-lens-handle".into(),
        &lib.lens_handle,
        Role::Tool,
        lc.x,
        lc.y,
        la,
        boost,
        q[p::lens],
    );
    let wc = offset(parts.anchors.tail, 40., -5. + q[p::wrenchY], angle);
    add(
        "prop-wrench".into(),
        &lib.wrench,
        Role::Tool,
        wc.x,
        wc.y,
        parts.anchors.tail_ang + 1.1 + q[p::wrenchSpin] * D2R,
        if small_view { 0.78 } else { 0.7 } * parts.unit_scale,
        q[p::wrench],
    );
    let ga = angle - 0.22;
    let gs = boost * (0.95 + 0.3 * clamp(q[p::glassExt], 0., 1.));
    let gc = offset(hand, 12., -33., ga);
    add(
        "prop-spyglass".into(),
        &lib.spyglass,
        Role::Accent,
        gc.x,
        gc.y,
        ga,
        gs,
        q[p::glass],
    );
    add(
        "prop-spyglass-end".into(),
        &lib.spyglass_end,
        Role::Tool,
        gc.x,
        gc.y,
        ga,
        gs,
        q[p::glass],
    );
    let cursor = offset(
        parts.anchors.spout,
        11. + q[p::cursorX],
        -31. + q[p::cursorY],
        angle,
    );
    add(
        "prop-cursor".into(),
        &lib.cursor,
        Role::Pointer,
        cursor.x,
        cursor.y,
        angle,
        boost,
        q[p::cursor],
    );
    let link = offset(parts.anchors.spout, 16., -34., angle);
    let link_angle = angle + (0.55 + q[p::linkTilt] * D2R);
    for (index, side) in [-1, 1].iter().enumerate() {
        add(
            format!("prop-link-{side}"),
            &lib.links[index],
            Role::Pointer,
            link.x,
            link.y,
            link_angle,
            boost,
            q[p::link],
        );
    }
    let spout = clamp(q[p::spout], 0., 1.);
    let surface = clamp(q[p::splash], 0., 1.);
    let origin = parts.anchors.spout;
    let unit = parts.unit_scale;
    add(
        "prop-spout".into(),
        &lib.jet,
        Role::Water,
        origin.x,
        origin.y,
        0.,
        (0.35 + 0.65 * spout) * unit,
        spout,
    );
    for side in [-1., 1.] {
        add(
            format!("prop-spout-drop-{}", side as i32),
            &lib.drop,
            Role::Water,
            origin.x + side * 25. * unit,
            origin.y - 35. * unit,
            -side * 0.35,
            unit * spout,
            spout,
        );
    }
    add(
        "prop-water-back".into(),
        &lib.water_back,
        Role::Water,
        0.,
        43.,
        0.,
        0.8 + 0.2 * surface,
        surface,
    );
    add(
        "prop-water-front".into(),
        &lib.water_front,
        Role::Water,
        0.,
        50.,
        0.,
        0.8 + 0.2 * surface,
        surface,
    );
    for (index, side) in [-1., 1.].iter().enumerate() {
        add(
            format!("prop-water-splash-{}", *side as i32),
            &lib.splashes[index],
            Role::Water,
            side * 35.,
            39.,
            0.,
            0.5 + 0.5 * surface,
            surface,
        );
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleKind {
    Drop,
    Mist,
    Thought,
    Bubble,
}

#[derive(Clone, Debug)]
pub struct Particle {
    pub kind: ParticleKind,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub g: f64,
    pub drag: f64,
    pub r: f64,
    pub age: f64,
    pub life: f64,
    pub cloud: bool,
    /// The acting state that emitted it; a particle never outlives it.
    pub owner: super::data::Act,
}

/// `particles()`: drops and bubbles as filled water shapes.
pub fn particles(list: &[Particle], small_view: bool) -> Vec<Shape> {
    let lib = library();
    let mut out = Vec::new();
    for (i, particle) in list.iter().enumerate() {
        if particle.age < 0.
            || particle.kind == ParticleKind::Mist
            || (small_view && particle.kind == ParticleKind::Bubble)
        {
            continue;
        }
        let fade = clamp(
            (particle.life - particle.age) / (particle.life * 0.2),
            0.,
            1.,
        );
        let r = particle.r * if small_view { 1.65 } else { 1. };
        let (mut x, mut y) = (particle.x, particle.y);
        if small_view && particle.kind == ParticleKind::Thought {
            x += 4.;
            y -= 6.;
        }
        if particle.kind == ParticleKind::Drop {
            out.push(Shape {
                id: format!("spout-{i}"),
                path: transform(&lib.particle_drop, x, y, 0., r),
                role: Role::Water,
                opacity: fade,
            });
        } else {
            let (path, scale) = if particle.cloud {
                (&lib.cloud, r / 6.)
            } else {
                (&lib.bubble, r)
            };
            out.push(Shape {
                id: format!("bubble-{i}"),
                path: transform(path, x, y, 0., scale),
                role: Role::Water,
                opacity: fade,
            });
        }
    }
    out
}
