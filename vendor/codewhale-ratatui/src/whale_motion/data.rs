//! The authored performance tables, read from the vendored handoff export
//! (`vendor/whale-character-v2/handoff/animation.json`) so the Rust Director
//! and the JavaScript oracle share one copy of every pose, clip, spring and
//! rate. The three small clips the export does not carry (the in-work hmm
//! beat, the resting puff and the blink) are transcribed from `acting.js`.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Every animated parameter, in the export's order. The pose is a fixed
/// array indexed by this list.
pub const PARAMS: [&str; 48] = [
    "x",
    "y",
    "rot",
    "scale",
    "squash",
    "tilt",
    "yaw",
    "curl",
    "arch",
    "head",
    "fluke",
    "flukeSpread",
    "fin",
    "finFar",
    "lookX",
    "lookY",
    "lid",
    "lidLow",
    "eyeScale",
    "brow",
    "browTilt",
    "mouth",
    "smile",
    "mouthSide",
    "page",
    "pageFlip",
    "pageRot",
    "pageDX",
    "pageDY",
    "pad",
    "padLines",
    "pencil",
    "scribX",
    "scribY",
    "lens",
    "wrench",
    "wrenchSpin",
    "wrenchY",
    "glass",
    "glassExt",
    "pod",
    "spout",
    "splash",
    "cursor",
    "cursorX",
    "cursorY",
    "link",
    "linkTilt",
];
pub const N: usize = PARAMS.len();
pub type Pose = [f64; N];

/// Named indices into a [`Pose`].
#[allow(non_upper_case_globals, dead_code)]
pub mod p {
    pub const x: usize = 0;
    pub const y: usize = 1;
    pub const rot: usize = 2;
    pub const scale: usize = 3;
    pub const squash: usize = 4;
    pub const tilt: usize = 5;
    pub const yaw: usize = 6;
    pub const curl: usize = 7;
    pub const arch: usize = 8;
    pub const head: usize = 9;
    pub const fluke: usize = 10;
    pub const flukeSpread: usize = 11;
    pub const fin: usize = 12;
    pub const finFar: usize = 13;
    pub const lookX: usize = 14;
    pub const lookY: usize = 15;
    pub const lid: usize = 16;
    pub const lidLow: usize = 17;
    pub const eyeScale: usize = 18;
    pub const brow: usize = 19;
    pub const browTilt: usize = 20;
    pub const mouth: usize = 21;
    pub const smile: usize = 22;
    pub const mouthSide: usize = 23;
    pub const page: usize = 24;
    pub const pageFlip: usize = 25;
    pub const pageRot: usize = 26;
    pub const pageDX: usize = 27;
    pub const pageDY: usize = 28;
    pub const pad: usize = 29;
    pub const padLines: usize = 30;
    pub const pencil: usize = 31;
    pub const scribX: usize = 32;
    pub const scribY: usize = 33;
    pub const lens: usize = 34;
    pub const wrench: usize = 35;
    pub const wrenchSpin: usize = 36;
    pub const wrenchY: usize = 37;
    pub const glass: usize = 38;
    pub const glassExt: usize = 39;
    pub const pod: usize = 40;
    pub const spout: usize = 41;
    pub const splash: usize = 42;
    pub const cursor: usize = 43;
    pub const cursorX: usize = 44;
    pub const cursorY: usize = 45;
    pub const link: usize = 46;
    pub const linkTilt: usize = 47;
}

pub fn param_index(name: &str) -> Option<usize> {
    PARAMS.iter().position(|p| *p == name)
}

/// The 17 acting states. One-to-one with the export's `acts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Act {
    Rest,
    Listen,
    Think,
    Busy,
    Read,
    Search,
    Write,
    Run,
    Browse,
    Talk,
    Pod,
    Needs,
    Done,
    Hmm,
    Computer,
    Connect,
    Sleep,
}

impl Act {
    pub const ALL: [Act; 17] = [
        Act::Rest,
        Act::Listen,
        Act::Think,
        Act::Busy,
        Act::Read,
        Act::Search,
        Act::Write,
        Act::Run,
        Act::Browse,
        Act::Talk,
        Act::Pod,
        Act::Needs,
        Act::Done,
        Act::Hmm,
        Act::Computer,
        Act::Connect,
        Act::Sleep,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Act::Rest => "rest",
            Act::Listen => "listen",
            Act::Think => "think",
            Act::Busy => "busy",
            Act::Read => "read",
            Act::Search => "search",
            Act::Write => "write",
            Act::Run => "run",
            Act::Browse => "browse",
            Act::Talk => "talk",
            Act::Pod => "pod",
            Act::Needs => "needs",
            Act::Done => "done",
            Act::Hmm => "hmm",
            Act::Computer => "computer",
            Act::Connect => "connect",
            Act::Sleep => "sleep",
        }
    }
    pub fn from_id(id: &str) -> Option<Act> {
        Act::ALL.into_iter().find(|act| act.id() == id)
    }
    fn index(self) -> usize {
        Act::ALL.iter().position(|a| *a == self).unwrap()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ease {
    Linear,
    In,
    Out,
    InOut,
    Back,
    Step,
}

impl Ease {
    fn parse(value: Option<&str>) -> Ease {
        match value {
            Some("l") => Ease::Linear,
            Some("i") => Ease::In,
            Some("o") => Ease::Out,
            Some("b") => Ease::Back,
            Some("s") => Ease::Step,
            _ => Ease::InOut,
        }
    }
    pub fn apply(self, t: f64) -> f64 {
        match self {
            Ease::Linear => t,
            Ease::In => t * t * t,
            Ease::Out => 1. - (1. - t).powf(3.),
            Ease::InOut => {
                if t < 0.5 {
                    4. * t * t * t
                } else {
                    1. - (-2. * t + 2.).powf(3.) / 2.
                }
            }
            Ease::Back => {
                let c1 = 1.70158;
                let c3 = c1 + 1.;
                1. + c3 * (t - 1.).powf(3.) + c1 * (t - 1.).powf(2.)
            }
            Ease::Step => {
                if t < 1. {
                    0.
                } else {
                    1.
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub frame: f64,
    pub value: f64,
    pub ease: Ease,
}

/// A particle emission authored on a clip frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Emit {
    Spout { n: u32, big: bool },
    Thought,
    Speech,
    Blub,
    Sleep,
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub dur: f64,
    /// Parameter index and its keys, in authored order.
    pub tracks: Vec<(usize, Vec<Key>)>,
    pub events: Vec<(f64, Emit)>,
}

impl Clip {
    /// `track()` from acting.js: keys are additive offsets, omitted easing is
    /// cubic in/out.
    pub fn track(keys: &[Key], f: f64) -> f64 {
        if keys.is_empty() || !f.is_finite() {
            return 0.;
        }
        if f <= keys[0].frame {
            return keys[0].value;
        }
        for i in 1..keys.len() {
            let k1 = keys[i];
            if f <= k1.frame {
                let k0 = keys[i - 1];
                let t = if k1.frame == k0.frame {
                    1.
                } else {
                    (f - k0.frame) / (k1.frame - k0.frame)
                };
                return k0.value + (k1.value - k0.value) * k1.ease.apply(t);
            }
        }
        keys[keys.len() - 1].value
    }
    pub fn eval(&self, f: f64, out: &mut Pose, weight: f64) {
        if !f.is_finite() || !weight.is_finite() {
            return;
        }
        for (param, keys) in &self.tracks {
            if let Some(value) = out.get_mut(*param) {
                *value += Self::track(keys, f) * weight;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ambient {
    pub breath: f64,
    pub bob: f64,
    pub blink: f64,
    pub dart: f64,
    pub puff: f64,
    pub sleepy: f64,
    pub period: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PosterParticle {
    ThoughtCloud,
    Speech,
}

#[derive(Clone, Debug)]
pub struct ActData {
    /// Absolute pose: defaults, then the act's base.
    pub base: Vec<(usize, f64)>,
    pub enter: Clip,
    pub loop_: Clip,
    pub beat: Option<Clip>,
    pub exit: Option<Clip>,
    pub ambient: Ambient,
    pub poster: Vec<(usize, f64)>,
    pub poster_particles: Vec<PosterParticle>,
}

pub struct Tables {
    pub defaults: Pose,
    acts: Vec<ActData>,
    pub springs: [(f64, f64); N],
    pub rates: [f64; N],
    pub wake: Clip,
    pub hmm_beat: Clip,
    pub puff: Clip,
    pub blink: Clip,
    kind_to_act: HashMap<String, Act>,
    /// The owner's canonical kinds the export maps, for coverage checks.
    pub owner_kinds: Vec<String>,
}

impl Tables {
    pub fn act(&self, act: Act) -> &ActData {
        &self.acts[act.index()]
    }
    /// `KIND_TO_ACT`: the owner's canonical kind to its action. Unknown or
    /// future kinds have no action and fall back to plain work.
    pub fn act_for_kind(&self, kind: &str) -> Option<Act> {
        self.kind_to_act.get(kind).copied()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Export {
    fps: f64,
    parameters: Vec<String>,
    defaults: HashMap<String, f64>,
    acts: HashMap<String, Value>,
    springs: HashMap<String, (f64, f64)>,
    rate_limits: HashMap<String, f64>,
    wake: Value,
    owner_kind_to_action: HashMap<String, String>,
}

fn clip(value: &Value) -> Clip {
    let dur = value["dur"].as_f64().expect("clip duration");
    let mut tracks = Vec::new();
    if let Some(keys) = value["keys"].as_object() {
        for (name, list) in keys {
            let param = param_index(name).unwrap_or_else(|| panic!("unknown parameter {name}"));
            let keys = list
                .as_array()
                .expect("keys")
                .iter()
                .map(|key| Key {
                    frame: key[0].as_f64().expect("key frame"),
                    value: key[1].as_f64().expect("key value"),
                    ease: Ease::parse(key.get(2).and_then(Value::as_str)),
                })
                .collect();
            tracks.push((param, keys));
        }
    }
    let events = value["events"]
        .as_array()
        .map(|events| {
            events
                .iter()
                .map(|event| {
                    let frame = event[0].as_f64().expect("event frame");
                    let kind = match event[1].as_str().expect("event kind") {
                        "spout" => Emit::Spout {
                            n: event[2]["n"].as_u64().unwrap_or(4) as u32,
                            big: event[2]["big"].as_bool().unwrap_or(false),
                        },
                        "thought" => Emit::Thought,
                        "speech" => Emit::Speech,
                        "blub" => Emit::Blub,
                        "sleep" => Emit::Sleep,
                        other => panic!("unknown event {other}"),
                    };
                    (frame, kind)
                })
                .collect()
        })
        .unwrap_or_default();
    Clip {
        dur,
        tracks,
        events,
    }
}

fn pairs(value: &Value) -> Vec<(usize, f64)> {
    value
        .as_object()
        .map(|object| {
            object
                .iter()
                .filter_map(|(name, value)| Some((param_index(name)?, value.as_f64()?)))
                .collect()
        })
        .unwrap_or_default()
}

/// One authored key: `[frame, value, ease?]`.
type Authored<'a> = (f64, f64, Option<&'a str>);

/// A clip written the way acting.js writes it.
fn authored(dur: f64, tracks: &[(&str, &[Authored])], events: Vec<(f64, Emit)>) -> Clip {
    Clip {
        dur,
        tracks: tracks
            .iter()
            .map(|(name, keys)| {
                (
                    param_index(name).unwrap(),
                    keys.iter()
                        .map(|(frame, value, ease)| Key {
                            frame: *frame,
                            value: *value,
                            ease: Ease::parse(*ease),
                        })
                        .collect(),
                )
            })
            .collect(),
        events,
    }
}

const EXPORT: &str = include_str!("../../assets/whale-motion/animation.json");

pub fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let export: Export = serde_json::from_str(EXPORT).expect("whale animation export");
        assert_eq!(export.fps, 30.);
        assert_eq!(export.parameters, PARAMS.map(str::to_string));
        let mut defaults = [0.; N];
        for (index, name) in PARAMS.iter().enumerate() {
            defaults[index] = export.defaults[*name];
        }
        let acts = Act::ALL
            .iter()
            .map(|act| {
                let value = &export.acts[act.id()];
                let ambient = &value["ambient"];
                let number = |key: &str| ambient[key].as_f64().unwrap_or(0.);
                let poster_particles = value["poster"]["particles"]
                    .as_array()
                    .map(|list| {
                        list.iter()
                            .filter_map(|entry| match entry[0].as_str()? {
                                "thought-cloud" => Some(PosterParticle::ThoughtCloud),
                                "speech" => Some(PosterParticle::Speech),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                ActData {
                    base: pairs(&value["base"]),
                    enter: clip(&value["enter"]),
                    loop_: clip(&value["loop"]),
                    beat: value.get("beat").map(clip),
                    exit: value.get("exit").map(clip),
                    ambient: Ambient {
                        breath: number("breath"),
                        bob: number("bob"),
                        blink: number("blink"),
                        dart: number("dart"),
                        puff: number("puff"),
                        sleepy: number("sleepy"),
                        period: number("period"),
                    },
                    poster: pairs(&value["poster"]),
                    poster_particles,
                }
            })
            .collect();
        let fallback = export.springs["_"];
        let springs = PARAMS.map(|name| export.springs.get(name).copied().unwrap_or(fallback));
        let rate = export.rate_limits["_"];
        let rates = PARAMS.map(|name| export.rate_limits.get(name).copied().unwrap_or(rate));
        let kind_to_act = export
            .owner_kind_to_action
            .iter()
            .map(|(kind, act)| (kind.clone(), Act::from_id(act).expect("mapped action")))
            .collect();
        let mut owner_kinds: Vec<String> = export.owner_kind_to_action.keys().cloned().collect();
        owner_kinds.sort();
        // acting.js HMM_BEAT, PUFF and BLINK.
        let hmm_beat = authored(
            12.,
            &[
                (
                    "head",
                    &[
                        (0., 0., None),
                        (4., -4., Some("o")),
                        (8., -4., None),
                        (12., 0., None),
                    ],
                ),
                (
                    "tilt",
                    &[
                        (0., 0., None),
                        (5., -6., None),
                        (8., -6., None),
                        (12., 0., None),
                    ],
                ),
                (
                    "mouthSide",
                    &[
                        (0., 0., None),
                        (5., 1., None),
                        (8., 1., None),
                        (12., 0., None),
                    ],
                ),
            ],
            Vec::new(),
        );
        let puff = authored(
            34.,
            &[
                (
                    "y",
                    &[
                        (0., 0., None),
                        (6., 0.7, None),
                        (10., -0.6, Some("o")),
                        (34., 0., Some("io")),
                    ],
                ),
                (
                    "squash",
                    &[
                        (0., 0., None),
                        (6., 0.03, None),
                        (10., -0.02, None),
                        (20., 0., None),
                    ],
                ),
            ],
            vec![(9., Emit::Spout { n: 3, big: false })],
        );
        let blink = authored(
            7.,
            &[(
                "lid",
                &[
                    (0., 0., None),
                    (2., 1., Some("i")),
                    (3., 1., None),
                    (7., 0., Some("o")),
                ],
            )],
            Vec::new(),
        );
        Tables {
            defaults,
            acts,
            springs,
            rates,
            wake: clip(&export.wake),
            hmm_beat,
            puff,
            blink,
            kind_to_act,
            owner_kinds,
        }
    })
}
