//! Port of `rig.js`: the mark is the rig. Profile contours are partitioned
//! from the official mark's cubic anchors; every pose keeps each named
//! part's command topology. Only move, cubic and close commands exist.

use super::data::{Pose, p, tables};
use super::math::{D2R, clamp, hypot, hypot2, lerp, round, smooth};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cmd {
    M([f64; 2]),
    C([f64; 6]),
    Z,
}

impl Cmd {
    pub fn coords(&self) -> &[f64] {
        match self {
            Cmd::M(c) => c,
            Cmd::C(c) => c,
            Cmd::Z => &[],
        }
    }
    pub fn coords_mut(&mut self) -> &mut [f64] {
        match self {
            Cmd::M(c) => c,
            Cmd::C(c) => c,
            Cmd::Z => &mut [],
        }
    }
    /// `c.slice(-2)`: the command's end point.
    pub fn end(&self) -> [f64; 2] {
        let c = self.coords();
        if c.is_empty() {
            return [0., 0.];
        }
        [c[c.len() - 2], c[c.len() - 1]]
    }
    /// The same command with every coordinate mapped; `i` is the index
    /// within the command's coordinates, so `i % 2` picks x or y.
    pub fn map(&self, mut f: impl FnMut(usize, f64) -> f64) -> Cmd {
        let mut out = *self;
        for (i, v) in out.coords_mut().iter_mut().enumerate() {
            *v = f(i, *v);
        }
        out
    }
    /// Rebuild a command of this kind from coordinates.
    fn with(&self, coords: &[f64]) -> Cmd {
        match self {
            Cmd::M(_) => Cmd::M([coords[0], coords[1]]),
            Cmd::C(_) => Cmd::C([
                coords[0], coords[1], coords[2], coords[3], coords[4], coords[5],
            ]),
            Cmd::Z => Cmd::Z,
        }
    }
}

pub type Path = Vec<Cmd>;

/// `parse()`: `M`, `C` and `Z` with plain decimal numbers.
pub fn parse(d: &str) -> Path {
    let bytes = d.as_bytes();
    let mut tokens: Vec<Result<u8, f64>> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if matches!(b, b'M' | b'C' | b'Z') {
            tokens.push(Ok(b));
            i += 1;
        } else if b == b'-' || b == b'.' || b.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let Ok(number) = d[start..i].parse::<f64>() else {
                return Vec::new();
            };
            if !number.is_finite() {
                return Vec::new();
            }
            tokens.push(Err(number));
        } else {
            i += 1;
        }
    }
    let mut path = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let Ok(op) = tokens[i] else {
            return Vec::new();
        };
        i += 1;
        let n = match op {
            b'M' => 2,
            b'C' => 6,
            _ => 0,
        };
        let Some(coords) = tokens.get(i..i + n) else {
            return Vec::new();
        };
        let Some(nums) = coords.iter().map(|t| t.err()).collect::<Option<Vec<_>>>() else {
            return Vec::new();
        };
        i += n;
        path.push(match op {
            b'M' => Cmd::M([nums[0], nums[1]]),
            b'C' => Cmd::C([nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]]),
            _ => Cmd::Z,
        });
    }
    path
}

pub fn ellipse(x: f64, y: f64, rx: f64, ry: f64) -> Path {
    let k = 0.55228475;
    vec![
        Cmd::M([x + rx, y]),
        Cmd::C([x + rx, y + ry * k, x + rx * k, y + ry, x, y + ry]),
        Cmd::C([x - rx * k, y + ry, x - rx, y + ry * k, x - rx, y]),
        Cmd::C([x - rx, y - ry * k, x - rx * k, y - ry, x, y - ry]),
        Cmd::C([x + rx * k, y - ry, x + rx, y - ry * k, x + rx, y]),
        Cmd::Z,
    ]
}

fn bezier(a: [f64; 2], c: &[f64; 6], t: f64) -> [f64; 2] {
    let q = 1. - t;
    [
        q * q * q * a[0] + 3. * q * q * t * c[0] + 3. * q * t * t * c[2] + t * t * t * c[4],
        q * q * q * a[1] + 3. * q * q * t * c[1] + 3. * q * t * t * c[3] + t * t * t * c[5],
    ]
}

fn points(path: &Path) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    let mut last = [0., 0.];
    for c in path {
        match c {
            Cmd::M(m) => {
                last = *m;
                out.push(last);
            }
            Cmd::C(c) => {
                for j in 1..=10 {
                    out.push(bezier(last, c, j as f64 / 10.));
                }
                last = [c[4], c[5]];
            }
            Cmd::Z => {}
        }
    }
    out
}

pub fn signed_area(path: &Path) -> f64 {
    let p = points(path);
    let mut sum = 0.;
    for (i, a) in p.iter().enumerate() {
        let b = p[(i + 1) % p.len()];
        sum += a[0] * b[1] - b[0] * a[1];
    }
    sum / 2.
}

struct Sample {
    l: f64,
    p: [f64; 2],
    d: [f64; 2],
}
struct Knot {
    l0: f64,
    l1: f64,
    h0: f64,
    h1: f64,
}
struct Measure {
    length: f64,
    knots: Vec<Knot>,
    samples: Vec<Sample>,
}

fn or_one(v: f64) -> f64 {
    if v == 0. || v.is_nan() { 1. } else { v }
}

fn measure(path: &Path) -> Measure {
    let (mut last, mut length) = ([0., 0.], 0.);
    let (mut knots, mut samples) = (Vec::new(), Vec::new());
    for c in path {
        match c {
            Cmd::M(m) => {
                last = *m;
                samples.push(Sample {
                    l: 0.,
                    p: last,
                    d: [1., 0.],
                });
            }
            Cmd::C(c) => {
                let a = last;
                let l0 = length;
                let mut prev = a;
                for j in 1..=30 {
                    let t = j as f64 / 30.;
                    let q = 1. - t;
                    let p = bezier(a, c, t);
                    let d = [
                        3. * q * q * (c[0] - a[0])
                            + 6. * q * t * (c[2] - c[0])
                            + 3. * t * t * (c[4] - c[2]),
                        3. * q * q * (c[1] - a[1])
                            + 6. * q * t * (c[3] - c[1])
                            + 3. * t * t * (c[5] - c[3]),
                    ];
                    length += hypot2(p[0] - prev[0], p[1] - prev[1]);
                    samples.push(Sample { l: length, p, d });
                    prev = p;
                }
                let seg = length - l0;
                knots.push(Knot {
                    l0,
                    l1: length,
                    h0: hypot2(c[0] - a[0], c[1] - a[1]) / or_one(seg),
                    h1: hypot2(c[2] - c[4], c[3] - c[5]) / or_one(seg),
                });
                last = [c[4], c[5]];
            }
            Cmd::Z => {}
        }
    }
    samples[0].d = samples[1].d;
    Measure {
        length,
        knots,
        samples,
    }
}

/// `match()`: resample `target` onto `source`'s command topology.
fn fit(source: &Path, target: &Path) -> Path {
    let src = measure(source);
    let dst = measure(target);
    let at = |u: f64| {
        let l = clamp(u, 0., 1.) * dst.length;
        let mut i = 1;
        while i < dst.samples.len() - 1 && dst.samples[i].l < l {
            i += 1;
        }
        let (a, b) = (&dst.samples[i - 1], &dst.samples[i]);
        let t = (l - a.l) / or_one(b.l - a.l);
        let d = [lerp(a.d[0], b.d[0], t), lerp(a.d[1], b.d[1], t)];
        let n = or_one(hypot(&d));
        (
            [lerp(a.p[0], b.p[0], t), lerp(a.p[1], b.p[1], t)],
            [d[0] / n, d[1] / n],
        )
    };
    let mut out = vec![Cmd::M(at(0.).0)];
    for k in &src.knots {
        let (ap, ad) = at(k.l0 / src.length);
        let (bp, bd) = at(k.l1 / src.length);
        let span = (k.l1 - k.l0) / src.length * dst.length;
        let h0 = span * k.h0.min(0.6);
        let h1 = span * k.h1.min(0.6);
        out.push(Cmd::C([
            ap[0] + ad[0] * h0,
            ap[1] + ad[1] * h0,
            bp[0] - bd[0] * h1,
            bp[1] - bd[1] * h1,
            bp[0],
            bp[1],
        ]));
    }
    out.push(Cmd::Z);
    out
}

fn orient_like(source: &Path, path: &Path) -> Path {
    if signed_area(source) * signed_area(path) >= 0. {
        return path.clone();
    }
    let start = path[0].end();
    let mut at = start;
    let mut segments: Vec<([f64; 2], [f64; 6])> = Vec::new();
    for c in path {
        if let Cmd::C(c) = c {
            segments.push((at, *c));
            at = [c[4], c[5]];
        }
    }
    if hypot2(at[0] - start[0], at[1] - start[1]) > 0.01 {
        segments.push((at, [at[0], at[1], start[0], start[1], start[0], start[1]]));
    }
    let mut out = vec![Cmd::M(start)];
    for (at, c) in segments.iter().rev() {
        out.push(Cmd::C([c[2], c[3], c[0], c[1], at[0], at[1]]));
    }
    out.push(Cmd::Z);
    out
}

/// Match anatomical landmarks section by section.
fn pieces(source: &Path, stops: &[usize], targets: &[Path]) -> Path {
    let mut begin = 0;
    let mut result: Path = Vec::new();
    for (i, &last) in stops.iter().enumerate() {
        let section: Path = if begin == 0 {
            source[..=last].to_vec()
        } else {
            let mut section = vec![Cmd::M(source[begin].end())];
            section.extend_from_slice(&source[begin + 1..=last]);
            section
        };
        let fitted = fit(&section, &targets[i]);
        let from = if i > 0 { 1 } else { 0 };
        result.extend_from_slice(&fitted[from..fitted.len() - 1]);
        begin = last;
    }
    result.push(Cmd::Z);
    result
}

pub const PART_IDS: [&str; 10] = [
    "body", "pouch", "pleat-1", "pleat-2", "jaw-band", "eye", "eyelid", "flipper", "flukes",
    "eye-far",
];

pub struct Rig {
    pub src: HashMap<&'static str, Path>,
    pub front: HashMap<&'static str, Path>,
    small_grid: HashMap<&'static str, Path>,
    small: Mutex<HashMap<u64, Arc<HashMap<&'static str, Path>>>>,
}

const MARK: &str = include_str!("../../assets/whale-motion/mark-data.js");

fn mark_contours() -> Vec<Path> {
    let start = MARK.find("root.WhaleMark=").expect("mark data") + "root.WhaleMark=".len();
    let end = MARK.rfind(";})").expect("mark data end");
    let value: serde_json::Value = serde_json::from_str(&MARK[start..end]).expect("mark json");
    value["contours"]
        .as_array()
        .unwrap()
        .iter()
        .map(|contour| {
            contour
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    let nums: Vec<f64> = c.as_array().unwrap()[1..]
                        .iter()
                        .map(|n| n.as_f64().unwrap())
                        .collect();
                    match c[0].as_str().unwrap() {
                        "M" => Cmd::M([nums[0], nums[1]]),
                        "C" => Cmd::C([nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]]),
                        _ => Cmd::Z,
                    }
                })
                .collect()
        })
        .collect()
}

fn index_at(path: &Path, x: f64, y: f64) -> usize {
    path.iter()
        .position(|c| !matches!(c, Cmd::Z) && c.end() == [x, y])
        .expect("landmark")
}

pub fn rig() -> &'static Rig {
    static RIG: OnceLock<Rig> = OnceLock::new();
    RIG.get_or_init(|| {
        let contours = mark_contours();
        let (outer, mut pouch, eye, pleat1, pleat2) = (
            contours[0].clone(),
            contours[1].clone(),
            contours[2].clone(),
            contours[3].clone(),
            contours[4].clone(),
        );
        let mut body = outer[..125].to_vec();
        body.push(Cmd::C([215., 92., 218., 59., 215.77, 40.96]));
        body.push(Cmd::Z);
        let mut flipper = vec![Cmd::M([228.68, 388.82])];
        flipper.extend_from_slice(&outer[48..54]);
        flipper.push(Cmd::C([273., 445., 250., 405., 228.68, 388.82]));
        flipper.push(Cmd::Z);
        let mut flukes = vec![Cmd::M([203.32, 110.65])];
        flukes.extend_from_slice(&outer[125..outer.len() - 1]);
        flukes.push(Cmd::Z);
        let ji = index_at(&outer, 141.12, 447.47);
        let je = index_at(&outer, 496., 236.55);
        let pe = index_at(&pouch, 238.49, 455.19);
        let mut jaw = vec![Cmd::M([141.12, 447.47])];
        jaw.extend_from_slice(&outer[ji + 1..=je]);
        jaw.push(Cmd::C([497., 249., 486., 265., 483.42, 270.06]));
        jaw.extend_from_slice(&pouch[3..=pe]);
        jaw.push(Cmd::C([209., 463., 169., 458., 141.12, 447.47]));
        jaw.push(Cmd::Z);
        let front_body = parse("M244 70 C147 65 78 114 76 228 C75 279 76 303 94 342 C112 370 127 380 151 378 C172 450 250 475 286 465 C384 453 436 399 442 341 C442 320 440 310 431 310 C353 269 270 274 215 288 C176 300 138 314 121 294 C101 269 112 222 127 192 C153 145 203 127 244 128 C244 108 244 87 244 70 Z");
        let front_pouch = parse("M416 353 C405 398 362 431 288 443 C234 448 184 410 155 352 C204 368 241 374 281 374 C328 374 375 365 416 353 Z");
        let front_flukes = parse("M244 128 C270 130 274 154 302 151 C324 149 343 126 346 108 C321 107 314 94 302 100 C279 95 263 97 250 96 C265 86 282 67 285 52 C267 49 249 64 244 70 C244 96 244 116 244 128 Z");
        let front_fin = parse("M165 360 C147 376 139 397 124 404 C147 408 163 402 180 390 C183 378 174 367 165 360 Z");
        let front_jaw = parse("M155 352 C184 410 234 448 288 443 C362 431 405 398 416 353 C424 354 435 350 442 341 C436 399 384 453 286 465 C230 470 180 435 151 378 C150 368 152 359 155 352 Z");
        let front_pleat1 = parse("M330 384 C329 405 315 424 294 434 C314 430 336 405 330 384 Z");
        let front_pleat2 = parse("M247 386 C247 404 261 423 277 433 C257 426 242 403 247 386 Z");
        // SRC in the JavaScript key order; pouch is the mark contour itself.
        let mut src_list: Vec<(&'static str, Path)> = Vec::new();
        src_list.push(("body", body));
        // Jaw was built from the pouch before the seam pass mutates it.
        let jaw_band = jaw;
        pouch = pouch.clone();
        src_list.push(("pouch", pouch));
        src_list.push(("pleat-1", pleat1));
        src_list.push(("pleat-2", pleat2));
        src_list.push(("jaw-band", jaw_band));
        src_list.push(("eye", eye.clone()));
        src_list.push(("eyelid", ellipse(294., 337., 18., 15.)));
        src_list.push(("flipper", flipper));
        src_list.push(("flukes", flukes));
        src_list.push(("eye-far", eye));
        let target: HashMap<&str, Path> = HashMap::from([
            ("body", front_body),
            ("pouch", front_pouch),
            ("pleat-1", front_pleat1),
            ("pleat-2", front_pleat2),
            ("jaw-band", front_jaw),
            ("eye", ellipse(200., 330., 13., 8.)),
            ("eyelid", ellipse(200., 330., 18., 15.)),
            ("flipper", front_fin),
            ("flukes", front_flukes.clone()),
            ("eye-far", ellipse(365., 330., 13., 8.)),
        ]);
        // Close any open seam with a straight cubic.
        for (_, path) in src_list.iter_mut() {
            let first = path[0].end();
            let last = path[path.len() - 2].end();
            if hypot2(first[0] - last[0], first[1] - last[1]) > 0.01 {
                let at = path.len() - 1;
                path.insert(
                    at,
                    Cmd::C([
                        lerp(last[0], first[0], 1. / 3.),
                        lerp(last[1], first[1], 1. / 3.),
                        lerp(last[0], first[0], 2. / 3.),
                        lerp(last[1], first[1], 2. / 3.),
                        first[0],
                        first[1],
                    ]),
                );
            }
        }
        let mut src: HashMap<&'static str, Path> = src_list.into_iter().collect();
        let mut front: HashMap<&'static str, Path> = HashMap::new();
        for id in PART_IDS {
            front.insert(id, fit(&src[id], &orient_like(&src[id], &target[id])));
        }
        front.insert(
            "body",
            pieces(
                &src["body"],
                &[47, 53, 80, 124, 125],
                &[
                    parse("M244 70 C147 65 78 114 76 228 C75 279 98 325 128 335 C142 347 144 363 151 378"),
                    parse("M151 378 C164 410 187 434 210 449 C231 462 261 468 286 465"),
                    parse("M286 465 C384 453 436 399 442 341 C442 320 440 310 431 310"),
                    parse("M431 310 C353 269 270 274 215 288 C176 300 138 314 121 294 C101 269 112 222 127 192 C153 145 203 127 244 128"),
                    parse("M244 128 C244 108 244 87 244 70"),
                ],
            ),
        );
        // Keep the tapered throat and parallel pleats in the attentive turn.
        for id in ["pouch", "pleat-1", "pleat-2"] {
            let scaled = src[id]
                .iter()
                .map(|c| {
                    c.map(|i, v| {
                        if i % 2 == 1 {
                            355. + (v - 355.) * 0.65
                        } else {
                            300. + (v - 300.) * 0.9
                        }
                    })
                })
                .collect();
            front.insert(id, scaled);
        }
        front.insert(
            "flipper",
            pieces(
                &src["flipper"],
                &[4, 7],
                &[
                    parse("M165 360 C147 376 139 397 124 404"),
                    parse("M124 404 C147 408 163 402 180 390 C183 378 174 367 165 360"),
                ],
            ),
        );
        let flukes_src = &src["flukes"];
        let mut open_src = flukes_src[..flukes_src.len() - 2].to_vec();
        open_src.push(Cmd::Z);
        let mut open_target = front_flukes[..front_flukes.len() - 2].to_vec();
        open_target.push(Cmd::Z);
        let tail_open = fit(&open_src, &open_target);
        let mut front_flukes_fit = tail_open[..tail_open.len() - 1].to_vec();
        front_flukes_fit.extend_from_slice(&front_flukes[front_flukes.len() - 2..]);
        front.insert("flukes", front_flukes_fit);
        // Overlap only the internal closing seam.
        for path in [src.get_mut("flukes").unwrap(), front.get_mut("flukes").unwrap()] {
            let at = path.len() - 2;
            // `path.at(-2)[1]` and `[3]` in `['C', x1, y1, x2, y2, x, y]`:
            // the two control-point x coordinates.
            let c = path[at].coords_mut();
            c[0] -= 2.;
            c[2] -= 2.;
        }
        let small_grid = HashMap::from([
            ("body", parse("M10.5 3.5 C6 3.5 3 7 3 12 C3 16.5 5 18.5 8 18.5 C9.5 18.5 10.5 18 11 17 C10.5 18.5 9 19.5 8 19.5 C9 20.5 11 20.5 12.5 20 C17 20 21 16 21.5 11.5 C22 8.5 15 10.5 12 12 C6.5 15 5.5 13 6 10 C6 8 8 6.5 10 6.5 C10.5 5.5 10.5 4.5 10.5 3.5 Z")),
            ("flukes", parse("M10 6.5 C11.5 6.5 11.5 8 13.5 8 C15 8 16.5 7 17 6.5 C14 6 13 5 12 6 C13 5.5 14 4.5 14.5 4 C13 3.5 12 3.5 10.5 3.5 C10.5 4.5 10.5 5.5 10 6.5 Z")),
            ("pouch", parse("M20.5 12.5 C20 17 16 20 11.5 19.5 C15 18.5 15.5 13 20.5 12.5 Z")),
            ("flipper", parse("M11 17 C10.5 18.5 9 19.5 8 19.5 C9 20.5 11 20.5 12.5 20 C13 19.5 12 18 11 17 Z")),
            ("jaw-band", parse("M11 20 C17 20 21 16 22 12 C21 17 17 21 11 21 C10 21 10 20 11 20 Z")),
            ("eye", ellipse(14., 15., 0.65, 0.46)),
            ("eye-far", ellipse(14., 15., 0.65, 0.46)),
            ("eyelid", ellipse(14., 15., 1., 0.7)),
        ]);
        Rig {
            src,
            front,
            small_grid,
            small: Mutex::new(HashMap::new()),
        }
    })
}

impl Rig {
    /// Optical-fit sources for a 16/24/32-cell icon; authored on a 24-cell
    /// grid, not downsampled from the hero.
    fn small_source(&self, size: f64) -> Arc<HashMap<&'static str, Path>> {
        let mut cache = self.small.lock().unwrap();
        if let Some(found) = cache.get(&size.to_bits()) {
            return found.clone();
        }
        let mut out = HashMap::new();
        for id in PART_IDS {
            let Some(grid) = self.small_grid.get(id) else {
                out.insert(id, self.src[id].clone());
                continue;
            };
            let fitted: Path = grid
                .iter()
                .map(|c| {
                    c.map(|_, v| {
                        let cell = round(v / 24. * size * 2.) / 2.;
                        (cell / size - 0.5) * 634.88 + 256.
                    })
                })
                .collect();
            out.insert(id, fit(&self.src[id], &orient_like(&self.src[id], &fitted)));
        }
        let out = Arc::new(out);
        // Arbitrary public optical sizes must not create an unbounded cache.
        // Eviction changes no evaluated geometry; existing readers own Arcs.
        if cache.len() >= 64 {
            cache.clear();
        }
        cache.insert(size.to_bits(), out.clone());
        out
    }
}

fn rotate(x: f64, y: f64, cx: f64, cy: f64, a: f64) -> [f64; 2] {
    let (c, s) = (a.cos(), a.sin());
    [
        cx + (x - cx) * c - (y - cy) * s,
        cy + (x - cx) * s + (y - cy) * c,
    ]
}

/// Morph a blade in its shoulder-to-tip frame.
fn morph_blade(a: &Path, b: &Path, t: f64) -> Path {
    let aa = a[0].end();
    let bb = b[0].end();
    let at = a[4].end();
    let bt = b[4].end();
    let mut angle = (bt[1] - bb[1]).atan2(bt[0] - bb[0]) - (at[1] - aa[1]).atan2(at[0] - aa[0]);
    angle = angle.sin().atan2(angle.cos());
    a.iter()
        .enumerate()
        .map(|(i, c)| {
            let src = c.coords();
            let dst = b[i].coords();
            let mut out = Vec::with_capacity(src.len());
            let mut j = 0;
            while j < src.len() {
                let target = rotate(dst[j] - bb[0], dst[j + 1] - bb[1], 0., 0., -angle);
                let p = rotate(
                    lerp(src[j] - aa[0], target[0], t),
                    lerp(src[j + 1] - aa[1], target[1], t),
                    0.,
                    0.,
                    angle * t,
                );
                out.push(p[0] + lerp(aa[0], bb[0], t));
                out.push(p[1] + lerp(aa[1], bb[1], t));
                j += 2;
            }
            c.with(&out)
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Direction {
    pub uncurl: f64,
    pub front_width: f64,
}

/// `01 / Emblem`, the product direction.
pub const MARK_DIRECTION: Direction = Direction {
    uncurl: 0.,
    front_width: 1.,
};
/// `02 / Cruise` and `03 / Open C`, the reference's alternate studies.
pub const CRUISE_DIRECTION: Direction = Direction {
    uncurl: 0.56,
    front_width: 1.08,
};
pub const OPEN_DIRECTION: Direction = Direction {
    uncurl: 0.25,
    front_width: 0.94,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Body,
    Hole,
    Cutout,
    Tool,
    Accent,
    Pointer,
    Water,
}

#[derive(Clone, Debug)]
pub struct Shape {
    pub id: String,
    pub path: Path,
    pub role: Role,
    pub opacity: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Anchors {
    pub blowhole: Point,
    pub spout: Point,
    pub mouth: Point,
    pub chin: Point,
    pub eye: Point,
    pub fin_tip: Point,
    pub tail: Point,
    pub tail_ang: f64,
    pub fin_ang: f64,
    pub head_ang: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Parts {
    pub shapes: Vec<Shape>,
    pub anchors: Anchors,
    pub small: bool,
    pub unit_scale: f64,
}

/// The rig's own defaults (`POSE`) with props at zero — what a partial pose
/// such as a calf's resolves to.
pub fn rig_pose() -> Pose {
    tables().defaults
}

/// `build()`: the named contour parts and anchors for one pose.
pub fn build(dir: Direction, pose: &Pose, lod: u8, size: f64, dpr: f64) -> Parts {
    if !size.is_finite()
        || size <= 0.
        || !dpr.is_finite()
        || dpr <= 0.
        || size > 16_384.
        || dpr > 16.
        || !dir.uncurl.is_finite()
        || !dir.front_width.is_finite()
        || dir.front_width <= 0.
        || pose.iter().any(|v| !v.is_finite())
    {
        return Parts::default();
    }
    let rig = rig();
    let q = pose;
    let yaw = clamp(q[p::yaw], 0., 1.);
    let head_yaw = smooth(0., 0.82, yaw) * 0.18;
    let tail_yaw = smooth(0.24, 1., yaw) * 0.12;
    let uncurl = dir.uncurl * (1. - clamp(q[p::curl], 0., 1.)) + (-q[p::curl]).max(0.) * 0.9
        - q[p::curl].max(0.) * 0.08;
    let small = lod >= 2;
    let tool_weight = clamp(q[p::page] + q[p::pencil] + q[p::lens] + q[p::glass], 0., 1.);
    let targets = [
        (p::page, 392., 222., 132.),
        (p::pencil, 394., 218., 118.),
        (p::lens, 380., 240., 168.),
        (p::glass, 401., 255., 122.),
    ];
    let mut tx = 335. * (1. - tool_weight);
    let mut ty = 386. * (1. - tool_weight);
    let mut total = 0.;
    for (key, x, y, neutral) in targets {
        let weight = q[key];
        total += weight;
        tx += (x + (q[p::fin] - neutral) * 0.38) * weight;
        ty += (y + (q[p::fin] - neutral) * 0.18) * weight;
    }
    if total > 1. {
        tx /= total;
        ty /= total;
    }
    let lift = smooth(30., 98., q[p::fin]);
    let attention = smooth(0.65, 1., yaw) * (1. - tool_weight);
    let tw = tool_weight;
    let held_fin = pieces(
        &rig.src["flipper"],
        &[4, 7],
        &[
            vec![
                Cmd::M([230., 398.]),
                Cmd::C([
                    238.,
                    408.,
                    lerp(252., 250., tw),
                    lerp(427., 425., tw),
                    lerp(262., 255., tw),
                    lerp(438., 435., tw),
                ]),
                Cmd::C([
                    lerp(290., 240., tw),
                    lerp(416., 350., tw),
                    tx - lerp(10., 35., tw),
                    ty + lerp(18., 5., tw),
                    tx,
                    ty,
                ]),
            ],
            vec![
                Cmd::M([tx, ty]),
                Cmd::C([tx - 66., ty + 20., 230., 357., 230., 398.]),
            ],
        ],
    );
    let small_src = small.then(|| rig.small_source(size));
    let source = |id: &str| -> &Path {
        match &small_src {
            Some(s) => &s[id],
            None => &rig.src[id],
        }
    };
    let profile_fin = morph_blade(source("flipper"), &held_fin, lift);
    let posed_fin = morph_blade(&profile_fin, &rig.front["flipper"], head_yaw);
    let transform = |id: &str, x: f64, y: f64| -> [f64; 2] {
        let (mut x, mut y) = (x, y);
        let w = (1. - smooth(165., 295., y)) * smooth(315., 135., x);
        x -= uncurl * 48. * w;
        y += uncurl * 60. * w;
        if id == "flukes" {
            let hinge = lerp(220., 246., tail_yaw);
            let weight = smooth(hinge, hinge + 75., x);
            [x, y] = rotate(x, y, hinge, 110., (q[p::fluke] - 6.) * 0.48 * D2R * weight);
        }
        if y > 285. {
            let jaw_weight = smooth(320., 470., y);
            y += q[p::mouth] * 22. * jaw_weight;
            if id == "pouch" {
                y += (q[p::smile] - 0.4) * 7. * ((x - 230.) / 260. * std::f64::consts::PI).sin();
                y += q[p::mouthSide] * 10. * smooth(280., 480., x) * (1. - smooth(400., 460., y));
            }
        }
        let head_weight = smooth(185., 470., x) * smooth(150., 300., y);
        [x, y] = rotate(x, y, 290., 305., -q[p::head] * 0.34 * D2R * head_weight);
        x = (x - 256.) / 5.12;
        y = (y - 256.) / 5.12;
        x *= q[p::scale] * (1. + q[p::squash] * 0.5);
        y *= q[p::scale] * (1. - q[p::squash]);
        [x, y] = rotate(x, y, 0., 0., (q[p::rot] + q[p::tilt] * 0.28) * D2R);
        if small {
            x *= 0.9;
            y *= 0.9;
        }
        [x + q[p::x], y + q[p::y]]
    };
    let mut shapes: Vec<Shape> = Vec::with_capacity(12);
    for id in [
        "flukes", "body", "jaw-band", "pouch", "pleat-1", "pleat-2", "flipper", "eye", "eye-far",
        "eyelid",
    ] {
        let a = source(id);
        let b = &rig.front[id];
        let t = if id == "flukes" { tail_yaw } else { head_yaw };
        let mut opacity = 1.;
        if id == "flipper" {
            opacity = lift * (1. - attention);
        }
        if id == "jaw-band" {
            opacity = 0.;
        }
        if id.starts_with("pleat") && small {
            opacity = 0.;
        }
        if id.starts_with("eye") && small && 26. / 512. * size * dpr < 2. {
            opacity = 0.;
        }
        if id == "eyelid" || id == "eye-far" {
            opacity = 0.;
        }
        let path: Path = a
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if matches!(c, Cmd::Z) {
                    return Cmd::Z;
                }
                let src = c.coords();
                let dst = b[i].coords();
                let mut out = Vec::with_capacity(src.len());
                let mut j = 0;
                while j < src.len() {
                    let blend = if id == "body" {
                        lerp(head_yaw, tail_yaw, 1. - smooth(140., 260., src[j + 1]))
                    } else {
                        t
                    };
                    let mut x = lerp(src[j], dst[j], blend);
                    let mut y = lerp(src[j + 1], dst[j + 1], blend);
                    if id == "body" {
                        x -= 8.
                            * (yaw * std::f64::consts::PI).sin()
                            * smooth(90., 0., hypot2(src[j] - 160., src[j + 1] - 442.));
                    }
                    if id == "flipper" {
                        let fin = posed_fin[i].coords();
                        x = fin[j];
                        y = fin[j + 1];
                    }
                    if yaw > 0. {
                        x = 256. + (x - 256.) * lerp(1., dir.front_width, head_yaw);
                    }
                    if id == "eye" || id == "eye-far" {
                        let cx = if id == "eye" {
                            lerp(294., 200., head_yaw)
                        } else {
                            lerp(294., 365., head_yaw)
                        };
                        let cy = lerp(337., 330., head_yaw);
                        let visible = if id == "eye-far" { 0. } else { 1. };
                        x = cx + (x - cx) * q[p::eyeScale] * visible;
                        y = cy
                            + (y - cy)
                                * q[p::eyeScale]
                                * (1. - q[p::lid] * 0.97 - q[p::lidLow] * 0.64)
                                * visible;
                        y += q[p::lidLow] * 3. * (((x - cx) / 13.).powf(2.) - 1.);
                        x += q[p::lookX] * 2.;
                        y += q[p::lookY] * 2.;
                    }
                    let xy = transform(id, x, y);
                    let grid = 124. / (size * dpr);
                    if small {
                        out.push(round(xy[0] / grid * 2.) / 2. * grid);
                        out.push(round(xy[1] / grid * 2.) / 2. * grid);
                    } else {
                        out.extend_from_slice(&xy);
                    }
                    j += 2;
                }
                c.with(&out)
            })
            .collect();
        let role = if matches!(id, "pouch" | "eye" | "eye-far") {
            Role::Hole
        } else {
            Role::Body
        };
        shapes.push(Shape {
            id: id.to_string(),
            path,
            role,
            opacity,
        });
    }
    // The legacy named lid geometry; closing the eye narrows its aperture.
    let cy = lerp(337., 328., head_yaw);
    let lid_path: Path = rig.src["eyelid"]
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if matches!(c, Cmd::Z) {
                return Cmd::Z;
            }
            let src = c.coords();
            let dst = rig.front["eyelid"][i].coords();
            let mut out = Vec::new();
            let mut j = 0;
            while j < src.len() {
                let x = lerp(src[j], dst[j], head_yaw);
                let y = lerp(src[j + 1], dst[j + 1], head_yaw);
                out.extend_from_slice(&transform(
                    "eye",
                    x,
                    cy - 13. + (y - cy + 15.) * q[p::lid] * 0.40,
                ));
                j += 2;
            }
            c.with(&out)
        })
        .collect();
    let lid = shapes.iter_mut().find(|s| s.id == "eyelid").unwrap();
    lid.opacity = 0.;
    lid.path = lid_path;
    let anchor = |x: f64, y: f64| {
        let z = transform("body", x, y);
        Point { x: z[0], y: z[1] }
    };
    let tip = shapes.iter().find(|s| s.id == "flipper").unwrap().path[4].end();
    // The mark's own flipper separation, carried up with the arm.
    let left = [[230., 398.], [230., 357.], [tx - 66., ty + 20.], [tx, ty]];
    let mix = |a: [f64; 2], b: [f64; 2], t: f64| [lerp(a[0], b[0], t), lerp(a[1], b[1], t)];
    let q1 = mix(left[0], left[1], 0.42);
    let mid = mix(left[1], left[2], 0.42);
    let q2 = mix(q1, mid, 0.42);
    let q3 = mix(q2, mix(mid, mix(left[2], left[3], 0.42), 0.42), 0.42);
    let point = |pt: [f64; 2], dx: f64, dy: f64| transform("body", pt[0] + dx, pt[1] + dy);
    let cat = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| [a[0], a[1], b[0], b[1], c[0], c[1]];
    let start = point(left[0], -6., -4.);
    let gap = Shape {
        id: "flipper-gap".into(),
        path: vec![
            Cmd::M(start),
            Cmd::C(cat(
                point(q1, -7., -2.),
                point(q2, -5., -1.),
                point(q3, 0., 0.),
            )),
            Cmd::C(cat(
                point(q2, 0., 0.),
                point(q1, 0., 0.),
                point(left[0], -6., -4.),
            )),
            Cmd::Z,
        ],
        role: Role::Hole,
        opacity: lift * (1. - head_yaw) * (1. - attention),
    };
    let at = shapes.iter().position(|s| s.id == "flipper").unwrap();
    shapes.insert(at, gap);
    let tail = transform(
        "flukes",
        lerp(289., 296., tail_yaw),
        lerp(113., 121., tail_yaw),
    );
    let anchors = Anchors {
        blowhole: anchor(lerp(325., 278., head_yaw), lerp(234., 278., head_yaw)),
        spout: anchor(410., 222.),
        mouth: anchor(lerp(480., 426., head_yaw), lerp(300., 353., head_yaw)),
        chin: anchor(lerp(397., 285., head_yaw), lerp(376., 425., head_yaw)),
        eye: anchor(lerp(294., 200., head_yaw), lerp(337., 330., head_yaw)),
        fin_tip: Point {
            x: tip[0],
            y: tip[1],
        },
        tail: Point {
            x: tail[0],
            y: tail[1],
        },
        tail_ang: (q[p::rot] + q[p::tilt] * 0.28 + (q[p::fluke] - 6.) * 0.35) * D2R,
        fin_ang: (q[p::rot] + q[p::tilt] * 0.28) * D2R,
        head_ang: (q[p::rot] + q[p::tilt] * 0.28 - q[p::head] * 0.34) * D2R,
    };
    Parts {
        shapes,
        anchors,
        small,
        unit_scale: q[p::scale] * if small { 0.9 } else { 1. },
    }
}

/// `holesFor()`: the compound holes painted with a shape. In one ink the
/// body subtracts its apertures; designated prop details are always holes.
pub fn holes_for(id: &str, shapes: &[Shape], mono: bool) -> Vec<Path> {
    let prefix = id.strip_suffix("body").unwrap_or("");
    shapes
        .iter()
        .filter(|h| {
            (mono
                && id.ends_with("body")
                && h.role == Role::Hole
                && [
                    format!("{prefix}pouch"),
                    format!("{prefix}eye"),
                    format!("{prefix}eye-far"),
                ]
                .contains(&h.id))
                || (mono && h.id == "flipper-gap" && id == "body")
                || (h.role == Role::Cutout
                    && (id == "prop-page" || id == "prop-pad")
                    && h.id.starts_with(&format!("{id}-")))
        })
        .filter_map(|h| {
            if !h.opacity.is_finite() {
                return None;
            }
            let origin = h.path.first()?.end();
            let w = h.opacity;
            Some(
                h.path
                    .iter()
                    .map(|c| c.map(|i, v| lerp(origin[i % 2], v, w)))
                    .collect(),
            )
        })
        .collect()
}

/// `pathString()`: the geometry receipt's exact number format.
pub fn path_string(path: &Path) -> String {
    path.iter()
        .map(|c| {
            let op = match c {
                Cmd::M(_) => "M",
                Cmd::C(_) => "C",
                Cmd::Z => "Z",
            };
            let nums: Vec<String> = c.coords().iter().map(|v| super::math::fixed3(*v)).collect();
            format!("{op}{}", nums.join(" "))
        })
        .collect::<Vec<_>>()
        .join(" ")
}
