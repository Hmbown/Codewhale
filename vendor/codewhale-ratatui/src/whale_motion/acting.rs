//! Port of `acting.js`: presence → acting state → pose over time.
//!
//! ```text
//! pose(t) = spring(base pose of the state)          state blends, follow-through
//!         + Σ one-shot clips (enter, exit, beats)    authored, exact frame timing
//!         + loop clip of the state                   the ongoing performance
//!         + ambient (breath, bob, blinks, puffs)     alive, never dancing
//!         + drag (flukes and fins trail the body)    secondary motion
//! ```
//!
//! Nothing here invents activity: every acting state comes from the owner's
//! presence, every work action from the owner's activity kind, every beat
//! from an observed owner onset. The Director owns no status timer and no
//! telemetry; it is a pure function of its inputs and the frames it steps.

use super::data::{Act, Ambient, Clip, Emit, N, Pose, PosterParticle, p, tables};
use super::math::{Rng, TAU, clamp, lerp};
use super::props::{Particle, ParticleKind};
use super::rig::{Anchors, MARK_DIRECTION, build};
use std::collections::{HashSet, VecDeque};

pub const FPS: f64 = 30.;

/// The authoritative owner presence. The first seven are today's GPUI
/// states; `Stuck` is the handoff's **proposed** eighth state, accepted here
/// for conformance only — the app never emits it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Offline,
    Idle,
    Listening,
    Thinking,
    Working,
    NeedsYou,
    Done,
    Stuck,
}

/// One observed foreground span onset reported by the owner.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub kind: String,
    pub since_ms: f64,
}

/// The owner's activity, exactly as reported. `observed` is true only when
/// the owner strictly said so; `parallel` keeps its raw number so a
/// non-integer count is refused rather than rounded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Activity {
    pub kind: Option<String>,
    pub observed: bool,
    pub parallel: Option<f64>,
    pub active: Vec<Span>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    /// `freshness === 'Live'`. Missing or stale freshness is plain work.
    pub live: bool,
    /// Stable turn identity; a completed one authorizes one Done flourish.
    pub turn_id: Option<String>,
    /// The explicit terminal status: completed, failed, error, canceled,
    /// interrupted. Never inferred.
    pub status: Option<String>,
    pub now_ms: Option<f64>,
    pub failed_at_ms: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Tool {
        kind: String,
    },
    Failed {
        kind: String,
        status: Option<String>,
    },
}

/// `actingFor()`.
pub fn acting_for(presence: Presence, activity: Option<&Activity>, context: &Context) -> Act {
    match presence {
        Presence::Offline => Act::Sleep,
        Presence::Idle => Act::Rest,
        Presence::Listening => Act::Listen,
        Presence::Thinking => Act::Think,
        Presence::NeedsYou => Act::Needs,
        Presence::Done => Act::Done,
        Presence::Stuck => {
            let failed = matches!(context.status.as_deref(), Some("failed" | "error"));
            let age = match (context.now_ms, context.failed_at_ms) {
                (Some(now), Some(at)) => now - at,
                _ => f64::NAN,
            };
            if failed && (0. ..3000.).contains(&age) {
                Act::Hmm
            } else {
                Act::Rest
            }
        }
        Presence::Working => match activity {
            Some(activity) if activity.observed && context.live => activity
                .kind
                .as_deref()
                .and_then(|kind| tables().act_for_kind(kind))
                .unwrap_or(Act::Busy),
            _ => Act::Busy,
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipId {
    Enter(Act),
    Loop(Act),
    Beat(Act),
    Exit(Act),
    Wake,
    HmmBeat,
    Puff,
    Blink,
}

impl ClipId {
    pub fn clip(self) -> &'static Clip {
        let t = tables();
        match self {
            ClipId::Enter(act) => &t.act(act).enter,
            ClipId::Loop(act) => &t.act(act).loop_,
            ClipId::Beat(act) => t.act(act).beat.as_ref().expect("beat"),
            ClipId::Exit(act) => t.act(act).exit.as_ref().expect("exit"),
            ClipId::Wake => &t.wake,
            ClipId::HmmBeat => &t.hmm_beat,
            ClipId::Puff => &t.puff,
            ClipId::Blink => &t.blink,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub clip: ClipId,
    pub at: f64,
    pub owner: Act,
    pub exit: bool,
    pub wake: bool,
    pub beat: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Calf {
    pub value: f64,
    pub velocity: f64,
    pub target: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emission {
    pub kind: Emit,
    pub owner: Act,
    pub f: f64,
}

#[derive(Clone, Copy, Debug)]
struct AmbientClock {
    blink_at: f64,
    dart_at: f64,
    puff_at: f64,
    sleep_at: f64,
    look_at: f64,
    look: (f64, f64),
    cur: (f64, f64),
}

#[derive(Clone, Copy, Debug)]
struct Departure {
    at: f64,
    offset: Pose,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub seed: u32,
    pub reduced: bool,
    pub ambient: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            seed: 7,
            reduced: false,
            ambient: true,
        }
    }
}

/// One session's performance. Drop it when the foreground session changes:
/// no pending clip, calf count or completed-turn identity may carry over.
#[derive(Clone, Debug)]
pub struct Director {
    rng: Rng,
    /// Frame time at 30 fps, floating.
    pub f: f64,
    pub presence: Presence,
    pub activity: Option<Activity>,
    pub context: Context,
    pub acting: Act,
    pub reduced: bool,
    ambient_on: bool,
    x: Pose,
    v: Pose,
    pub shots: Vec<Shot>,
    loops: Vec<Shot>,
    pub particles: Vec<Particle>,
    pub lines: f64,
    pub events: VecDeque<(Event, f64)>,
    pub emissions: VecDeque<Emission>,
    amb: AmbientClock,
    pose_out: Pose,
    departure: Option<Departure>,
    changed_at: f64,
    from_target: Pose,
    delays: Pose,
    done_turns: HashSet<String>,
    spans: VecDeque<String>,
    span_set: HashSet<String>,
    pub calves: [Calf; 3],
    anchors: Option<Anchors>,
}

const HELD_PROPS: [usize; 9] = [
    p::page,
    p::pad,
    p::pencil,
    p::lens,
    p::wrench,
    p::glass,
    p::pod,
    p::cursor,
    p::link,
];

impl Director {
    pub fn new(options: Options) -> Self {
        let seed = if options.seed == 0 { 7 } else { options.seed };
        let mut director = Self {
            rng: Rng::new(seed),
            f: 0.,
            presence: Presence::Idle,
            activity: None,
            context: Context::default(),
            acting: Act::Rest,
            reduced: options.reduced,
            ambient_on: options.ambient,
            x: [0.; N],
            v: [0.; N],
            shots: Vec::new(),
            loops: vec![Shot {
                clip: ClipId::Loop(Act::Rest),
                at: 0.,
                owner: Act::Rest,
                exit: false,
                wake: false,
                beat: false,
            }],
            particles: Vec::new(),
            lines: 1.,
            events: VecDeque::new(),
            emissions: VecDeque::new(),
            amb: AmbientClock {
                blink_at: 40.,
                dart_at: 120.,
                puff_at: 360.,
                sleep_at: 0.,
                look_at: 0.,
                look: (0., 0.),
                cur: (0., 0.),
            },
            pose_out: [0.; N],
            departure: None,
            changed_at: 0.,
            from_target: [0.; N],
            delays: [0.; N],
            done_turns: HashSet::new(),
            spans: VecDeque::new(),
            span_set: HashSet::new(),
            calves: [Calf::default(); 3],
            anchors: None,
        };
        director.x = director.target_for(Act::Rest);
        director.pose_out = director.x;
        director
    }

    pub fn target_for(&self, act: Act) -> Pose {
        let mut pose = tables().defaults;
        for (param, value) in &tables().act(act).base {
            pose[*param] = *value;
        }
        pose[p::padLines] = if self.lines == 0. { 1. } else { self.lines };
        pose
    }

    /// `set(presence, activity, context)`: the only way owner truth enters.
    pub fn set(&mut self, presence: Presence, activity: Option<Activity>, context: Context) {
        self.presence = presence;
        self.activity = activity;
        self.context = context;
        let next = acting_for(presence, self.activity.as_ref(), &self.context);
        let done_key = (presence == Presence::Done
            && self.context.status.as_deref() == Some("completed"))
        .then(|| self.context.turn_id.clone())
        .flatten()
        .filter(|id| !id.is_empty());
        let celebrate = done_key.is_some_and(|key| self.done_turns.insert(key));
        self.go(next, celebrate, celebrate && next == Act::Done);
        let n = match (next, self.activity.as_ref().and_then(|a| a.parallel)) {
            (Act::Pod, Some(parallel)) if parallel.fract() == 0. && parallel > 0. => {
                parallel.min(3.) as usize
            }
            _ => 0,
        };
        let reduced = self.reduced;
        for (i, calf) in self.calves.iter_mut().enumerate() {
            calf.target = if i < n { 1. } else { 0. };
            if reduced {
                calf.value = calf.target;
                calf.velocity = 0.;
            }
        }
        // Observe every onset, including those hidden by owner priority:
        // never queue, never replay.
        let spans = match &self.activity {
            Some(activity) if activity.observed && self.context.live => {
                activity.active.iter().take(4).cloned().collect()
            }
            _ => Vec::new(),
        };
        for span in spans {
            if !span.since_ms.is_finite() {
                continue;
            }
            let since = if span.since_ms == 0. {
                0.
            } else {
                span.since_ms
            };
            let key = format!(
                "{}|{}|{}",
                self.context.turn_id.as_deref().unwrap_or(""),
                span.kind,
                since
            );
            if self.span_set.insert(key.clone()) {
                self.spans.push_back(key);
                self.event(Event::Tool { kind: span.kind });
            }
        }
        if self.span_set.len() > 512 {
            while self.spans.len() > 256 {
                let old = self.spans.pop_front().unwrap();
                self.span_set.remove(&old);
            }
        }
        if self.reduced {
            self.pose_out = self.poster();
        }
    }

    fn go(&mut self, next: Act, celebrate: bool, force: bool) {
        if next == self.acting && !force {
            return;
        }
        let prev = self.acting;
        let current = self.pose();
        // Freeze the visible offset, not the old clip's future movement or events.
        let mut offset = [0.; N];
        for i in 0..N {
            offset[i] = current[i] - self.x[i];
        }
        self.departure = Some(Departure { at: self.f, offset });
        let wake_dur = tables().wake.dur;
        let f = self.f;
        self.shots
            .retain(|s| s.wake && f - s.at < wake_dur && next != Act::Sleep);
        self.loops.clear();
        self.particles.clear();
        self.acting = next;
        self.changed_at = self.f;
        self.from_target = self.x;
        self.delays = [0.; N];
        self.delays[p::fin] = 3.;
        self.delays[p::finFar] = 4.;
        self.delays[p::fluke] = 4.;
        self.delays[p::curl] = if prev == Act::Sleep { 2. } else { 3. };
        let think = if next == Act::Think { 6. } else { 0. };
        self.delays[p::lookX] = think;
        self.delays[p::lookY] = think;
        self.delays[p::lid] = if next == Act::Sleep { 6. } else { 0. };
        self.delays[p::glassExt] = if next == Act::Browse { 8. } else { 0. };
        if next == Act::Write {
            self.lines = 1.;
        }
        if self.reduced {
            self.x = self.target_for(next);
            self.v = [0.; N];
            self.shots.clear();
            self.departure = None;
            self.pose_out = self.poster();
            return;
        }
        let shot = |clip, owner| Shot {
            clip,
            at: f,
            owner,
            exit: false,
            wake: false,
            beat: false,
        };
        if tables().act(prev).exit.is_some() {
            self.shots.push(Shot {
                exit: true,
                ..shot(ClipId::Exit(prev), prev)
            });
        }
        if prev == Act::Sleep && next != Act::Sleep {
            self.shots.push(Shot {
                wake: true,
                ..shot(ClipId::Wake, next)
            });
        }
        if next != Act::Done || celebrate {
            self.shots.push(shot(ClipId::Enter(next), next));
        }
        let enter = tables().act(next).enter.dur;
        self.loops = vec![Shot {
            at: f + (enter * 0.6).min(10.),
            ..shot(ClipId::Loop(next), next)
        }];
    }

    pub fn set_reduced(&mut self, value: bool) {
        if self.reduced == value {
            return;
        }
        self.reduced = value;
        self.shots.clear();
        self.loops.clear();
        self.particles.clear();
        self.departure = None;
        self.acting = acting_for(self.presence, self.activity.as_ref(), &self.context);
        self.x = self.target_for(self.acting);
        self.v = [0.; N];
        for calf in &mut self.calves {
            calf.value = calf.target;
            calf.velocity = 0.;
        }
        self.pose_out = self.poster();
        if !self.reduced {
            self.loops = vec![Shot {
                clip: ClipId::Loop(self.acting),
                at: self.f,
                owner: self.acting,
                exit: false,
                wake: false,
                beat: false,
            }];
        }
    }

    /// An owner event. A beat plays only for matching, fresh, observed work.
    pub fn event(&mut self, event: Event) -> bool {
        self.events.push_back((event.clone(), self.f));
        if self.events.len() > 40 {
            self.events.pop_front();
        }
        let observed = self.activity.as_ref().is_some_and(|a| a.observed);
        if self.presence != Presence::Working || !self.context.live || !observed {
            return false;
        }
        let kind = match &event {
            Event::Tool { kind } | Event::Failed { kind, .. } => kind,
        };
        if tables().act_for_kind(kind) != Some(self.acting) {
            return false;
        }
        let clip = match &event {
            Event::Tool { .. } => tables()
                .act(self.acting)
                .beat
                .is_some()
                .then_some(ClipId::Beat(self.acting)),
            Event::Failed { status, .. } => (matches!(status.as_deref(), Some("failed" | "error"))
                && !matches!(
                    self.acting,
                    Act::Rest
                        | Act::Sleep
                        | Act::Needs
                        | Act::Done
                        | Act::Hmm
                        | Act::Listen
                        | Act::Think
                ))
            .then_some(ClipId::HmmBeat),
        };
        let Some(clip) = clip else {
            return false;
        };
        if matches!(event, Event::Tool { .. }) && self.acting == Act::Write {
            self.lines = (self.lines + 1.).min(4.);
        }
        if self.reduced {
            self.pose_out = self.poster();
            return true;
        }
        self.shots.retain(|s| s.clip != clip);
        self.shots.push(Shot {
            clip,
            at: self.f,
            owner: self.acting,
            exit: false,
            wake: false,
            beat: true,
        });
        true
    }

    /// Advance one step of at most a tenth of a second.
    pub fn step(&mut self, dt: f64) {
        // Invalid host time must not poison every spring and contour.
        if !dt.is_finite() {
            return;
        }
        if self.reduced {
            self.pose_out = self.poster();
            return;
        }
        let t = tables();
        let dt = clamp(dt, 0., 1. / 10.);
        let df = dt * FPS;
        let f0 = self.f;
        self.f += df;
        let target = self.target_for(self.acting);
        let sub = (dt * 120.).ceil().max(1.) as usize;
        let h = dt / sub as f64;
        for _ in 0..sub {
            for (i, &target) in target.iter().enumerate() {
                let (w, z) = t.springs[i];
                let goal = if self.f - self.changed_at < self.delays[i] {
                    self.from_target[i]
                } else {
                    target
                };
                self.v[i] += (w * w * (goal - self.x[i]) - 2. * z * w * self.v[i]) * h;
                self.x[i] += self.v[i] * h;
            }
            for calf in &mut self.calves {
                calf.velocity += (144. * (calf.target - calf.value) - 24. * calf.velocity) * h;
                calf.value = clamp(calf.value + calf.velocity * h, 0., 1.);
            }
        }
        let mut out = [0.; N];
        if let Some(departure) = self.departure {
            let w = 1. - clamp((self.f - departure.at) / 8., 0., 1.);
            for (out, offset) in out.iter_mut().zip(departure.offset) {
                *out = offset * w;
            }
            if w == 0. {
                self.departure = None;
            }
        }
        self.anchors = Some(build(MARK_DIRECTION, &self.pose_out, 0, 24., 1.).anchors);
        let f = self.f;
        self.shots.retain(|s| f - s.at <= s.clip.clip().dur);
        let shots = self.shots.clone();
        for s in &shots {
            if f < s.at {
                continue;
            }
            let clip = s.clip.clip();
            clip.eval(f - s.at, &mut out, 1.);
            if !s.exit && (s.wake || s.owner == self.acting) {
                self.fire(clip, f0 - s.at, f - s.at, s.owner);
            }
        }
        let loops = self.loops.clone();
        for l in &loops {
            if f < l.at {
                continue;
            }
            let clip = l.clip.clip();
            let lf = (f - l.at) % clip.dur;
            let w = clamp((f - l.at) / 8., 0., 1.);
            clip.eval(lf, &mut out, w);
            let pf = (f0 - l.at) % clip.dur;
            self.fire(clip, if pf <= lf { pf } else { -1. }, lf, l.owner);
        }
        self.ambient(&mut out);
        out[p::fluke] += clamp(-self.v[p::y] * 2.2 - self.v[p::rot] * 0.6, -22., 22.);
        out[p::fin] += clamp(-self.v[p::y] * 1.2, -12., 12.);
        let mut pose = [0.; N];
        for i in 0..N {
            let desired = self.x[i] + out[i];
            let prior = self.pose_out[i];
            let delta = if i == p::wrenchSpin {
                angle_delta(desired, prior)
            } else {
                desired - prior
            };
            let rate = t.rates[i];
            pose[i] = prior + clamp(delta, -rate * df, rate * df);
        }
        for i in [p::lid, p::lidLow, p::mouth, p::yaw, p::pageFlip] {
            pose[i] = clamp(pose[i], 0., 1.);
        }
        for i in HELD_PROPS {
            pose[i] = clamp(pose[i], 0., 1.);
        }
        self.pose_out = pose;
        for particle in &mut self.particles {
            particle.age += df;
            particle.vy += particle.g * df;
            particle.vx *= particle.drag.powf(df);
            particle.vy *= particle.drag.powf(df);
            particle.x += particle.vx * df;
            particle.y += particle.vy * df;
        }
        let acting = self.acting;
        self.particles
            .retain(|particle| particle.age < particle.life && particle.owner == acting);
    }

    fn fire(&mut self, clip: &Clip, a: f64, b: f64, owner: Act) {
        if owner != self.acting {
            return;
        }
        for (frame, kind) in &clip.events {
            if *frame > a && *frame <= b {
                self.emit(*kind);
            }
        }
    }

    fn ambient(&mut self, out: &mut Pose) {
        if !self.ambient_on {
            return;
        }
        let a: Ambient = tables().act(self.acting).ambient;
        let f = self.f;
        // Breath: a slow inhale, a short hold, a longer exhale.
        let ph = (f % a.period) / a.period;
        let half_pi = std::f64::consts::PI / 2.;
        let b = if ph < 0.4 {
            (ph / 0.4 * half_pi).sin()
        } else if ph < 0.5 {
            1.
        } else {
            ((ph - 0.5) / 0.5 * half_pi).cos()
        };
        let breath = a.breath;
        out[p::squash] -= 0.016 * b * breath;
        out[p::y] = out[p::y] - 0.55 * b * breath + (f / 210. * TAU).sin() * 1.1 * a.bob;
        out[p::fluke] += 3.5 * ((f - 10.) / a.period * TAU).sin() * breath;
        out[p::fin] += 2.2 * ((f - 6.) / a.period * TAU).sin() * breath;
        if self.acting == Act::Rest {
            out[p::rot] += 2.3 * (f / 240. * TAU).sin();
            out[p::x] += 1.8 * (f / 330. * TAU).sin();
            out[p::y] += ((f + 33.) / 210. * TAU).sin();
            out[p::fluke] += 8. * ((f - 24.) / 180. * TAU).sin();
            out[p::head] += 1.5 * ((f - 12.) / 250. * TAU).sin();
        }
        let blink = |at: f64| Shot {
            clip: ClipId::Blink,
            at,
            owner: self.acting,
            exit: false,
            wake: false,
            beat: false,
        };
        if a.blink > 0. && f >= self.amb.blink_at {
            self.shots.push(blink(f));
            if self.rng.draw() < 0.2 {
                self.shots.push(blink(f + 9.));
            }
            self.amb.blink_at = f + (90. + self.rng.draw() * 150.) / a.blink;
        }
        if a.dart > 0. {
            if f >= self.amb.dart_at {
                let x = (self.rng.draw() - 0.5) * 0.9;
                let y = (self.rng.draw() - 0.5) * 0.5;
                self.amb.look = (x, y);
                self.amb.dart_at = f + 150. + self.rng.draw() * 240.;
                self.amb.look_at = f;
            }
            let t = clamp((f - self.amb.look_at) / 4., 0., 1.);
            self.amb.cur.0 = lerp(self.amb.cur.0, self.amb.look.0, t);
            self.amb.cur.1 = lerp(self.amb.cur.1, self.amb.look.1, t);
            out[p::lookX] += self.amb.cur.0;
            out[p::lookY] += self.amb.cur.1;
        }
        if a.puff > 0. && f >= self.amb.puff_at {
            self.shots.push(Shot {
                clip: ClipId::Puff,
                ..blink(f)
            });
            self.amb.puff_at = f + 600. + self.rng.draw() * 540.;
        }
        if a.sleepy > 0. && f >= self.amb.sleep_at {
            self.emit(Emit::Sleep);
            self.amb.sleep_at = f + 170. + self.rng.draw() * 140.;
        }
    }

    /// Particles are born at rig anchors, in world design units.
    fn emit(&mut self, kind: Emit) {
        let Some(a) = self.anchors else {
            return;
        };
        let owner = self.acting;
        self.emissions.push_back(Emission {
            kind,
            owner,
            f: self.f,
        });
        if self.emissions.len() > 100 {
            self.emissions.pop_front();
        }
        let r = &mut self.rng;
        let _seed = r.draw();
        let particle = |kind, x, y, vx, vy, radius, life| Particle {
            kind,
            x,
            y,
            vx,
            vy,
            g: 0.,
            drag: 0.98,
            r: radius,
            age: 0.,
            life,
            cloud: false,
            owner,
        };
        match kind {
            Emit::Spout { n, big } => {
                let source = if big { a.spout } else { a.blowhole };
                for _ in 0..n {
                    let ang =
                        -std::f64::consts::PI / 2. + (r.draw() - 0.5) * if big { 1.3 } else { 0.8 };
                    let sp = if big { 1.55 } else { 0.9 } * (0.75 + r.draw() * 0.5);
                    let x = source.x + if big { (r.draw() - 0.5) * 26. } else { 0. };
                    let y = source.y - if big { 24. + r.draw() * 3. } else { 1. };
                    let vx = ang.cos() * if big { 0.3 } else { sp };
                    let vy = if big { -0.35 } else { ang.sin() * sp };
                    self.particles.push(Particle {
                        g: if big { 0.055 } else { 0.075 },
                        drag: 0.985,
                        ..particle(
                            ParticleKind::Drop,
                            x,
                            y,
                            vx,
                            vy,
                            if big { 1.6 } else { 1.3 },
                            if big { 35. } else { 26. },
                        )
                    });
                }
                for _ in 0..if big { 3 } else { 2 } {
                    let x = a.blowhole.x + (r.draw() - 0.5) * 3.;
                    let y = a.blowhole.y - 5. - r.draw() * 4.;
                    let vx = (r.draw() - 0.5) * 0.1;
                    self.particles.push(Particle {
                        drag: 0.95,
                        ..particle(
                            ParticleKind::Mist,
                            x,
                            y,
                            vx,
                            -0.12,
                            if big { 4.5 } else { 3.2 },
                            24.,
                        )
                    });
                }
            }
            Emit::Thought => {
                let bx = a.blowhole.x + 2.;
                let by = a.blowhole.y - 3.;
                for (i, (delay, radius, cloud)) in
                    [(0., 1.3, false), (5., 2.1, false), (10., 5.2, true)]
                        .into_iter()
                        .enumerate()
                {
                    self.particles.push(Particle {
                        age: -delay,
                        drag: 0.99,
                        cloud,
                        ..particle(
                            ParticleKind::Thought,
                            bx + [0., 6., 14.][i],
                            by - i as f64 * 5.5,
                            0.06,
                            -0.05,
                            radius,
                            64. - delay,
                        )
                    });
                }
            }
            Emit::Speech => {
                let m = a.mouth;
                for i in 0..3 {
                    let x = m.x + 4. + r.draw() * 2.;
                    let vx = 0.26 + r.draw() * 0.15;
                    let vy = -0.3 - r.draw() * 0.2;
                    let radius = 1.7 + r.draw() * 1.6;
                    let _seed = r.draw();
                    self.particles.push(Particle {
                        age: -(i as f64) * 6.,
                        drag: 0.985,
                        ..particle(ParticleKind::Bubble, x, m.y - 1., vx, vy, radius, 50.)
                    });
                }
            }
            Emit::Blub => {
                self.particles.push(Particle {
                    drag: 0.995,
                    ..particle(
                        ParticleKind::Bubble,
                        a.blowhole.x,
                        a.blowhole.y - 2.,
                        0.02,
                        -0.16,
                        2.2,
                        64.,
                    )
                });
            }
            Emit::Sleep => {
                self.particles.push(Particle {
                    drag: 0.997,
                    ..particle(
                        ParticleKind::Bubble,
                        a.blowhole.x,
                        a.blowhole.y - 1.5,
                        0.03,
                        -0.1,
                        1.6,
                        90.,
                    )
                });
            }
        }
    }

    /// Reduced motion: the state's poster pose — no clock, no ambient, no
    /// transitions.
    pub fn poster(&self) -> Pose {
        let mut pose = self.target_for(self.acting);
        for (param, value) in &tables().act(self.acting).poster {
            pose[*param] = *value;
        }
        pose[p::padLines] = self.lines;
        pose
    }

    pub fn poster_particles(&self, anchors: &Anchors) -> Vec<Particle> {
        let owner = self.acting;
        let still = |kind, x, y, r, cloud| Particle {
            kind,
            x,
            y,
            vx: 0.,
            vy: 0.,
            g: 0.,
            drag: 0.98,
            r,
            age: 10.,
            life: 100.,
            cloud,
            owner,
        };
        let mut out = Vec::new();
        for kind in &tables().act(self.acting).poster_particles {
            match kind {
                PosterParticle::ThoughtCloud => {
                    let bx = anchors.blowhole.x + 2.;
                    let by = anchors.blowhole.y - 3.;
                    out.push(still(ParticleKind::Thought, bx, by, 1.3, false));
                    out.push(still(ParticleKind::Thought, bx + 6., by - 5.5, 2.1, false));
                    out.push(still(ParticleKind::Thought, bx + 14., by - 13., 5.2, true));
                }
                PosterParticle::Speech => {
                    let m = anchors.mouth;
                    out.push(still(ParticleKind::Bubble, m.x + 6., m.y - 5., 1.9, false));
                    out.push(still(ParticleKind::Bubble, m.x + 9., m.y - 10., 1.3, false));
                }
            }
        }
        out
    }

    pub fn pose(&self) -> Pose {
        self.pose_out
    }

    /// Whether anything is still moving beyond the settled loop: a clip, a
    /// departure fade, a particle or a calf in transit. Surfaces use it to
    /// choose their active or resting paint cap.
    pub fn in_transition(&self) -> bool {
        self.departure.is_some()
            || self.shots.iter().any(|s| s.clip != ClipId::Blink)
            || !self.particles.is_empty()
            || self
                .calves
                .iter()
                .any(|c| (c.value - c.target).abs() > 0.002 || c.velocity.abs() > 0.002)
    }
}

pub fn angle_delta(a: f64, b: f64) -> f64 {
    ((a - b + 180.) % 360. + 360.) % 360. - 180.
}
