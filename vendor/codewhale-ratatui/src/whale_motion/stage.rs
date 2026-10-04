//! The one session-scoped performance every whale surface reads.
//!
//! The Stage holds a single [`Director`] for the foreground session and a
//! single clock. Owner truth enters through [`Stage::observe`]; surfaces
//! call [`Stage::advance`] before they paint, which is idempotent within a
//! frame, so any number of surfaces share one clock instead of each
//! stepping its own. Nothing here classifies a tool, reads telemetry or
//! owns a status timer.

use super::acting::{Activity, Context, Director, Options, Presence, Span};
use super::data::Act;
use super::habitat::{Cove, Layer};
use super::rig::Parts;
use super::scene::{View, scene, scene_posed};
use serde_json::Value;
use std::time::{Duration, Instant};

/// Everything the owner reports that the performance may use.
#[derive(Clone, Debug, PartialEq)]
pub struct Inputs {
    pub presence: Presence,
    pub activity: Option<Activity>,
    pub context: Context,
}

/// A surface's paint caps (handoff `scheduling`): these are ceilings on one
/// shared clock, not extra timers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// Habitat and companion: 30 paints/s in action, 8 at rest.
    Hero,
    /// Rail head and toolbar mark: 15 paints/s in action, 2 at rest.
    Small,
    /// Terminal art: at most 6 paints/s in action, 2 at rest.
    Terminal,
}

/// Longest catch-up after a hidden or stalled interval. Anything older is
/// dropped: resume from current owner truth, never replay missed motion.
pub const MAX_CATCH_UP: Duration = Duration::from_secs(1);
/// Surfaces painting in the same display frame share the step.
const SAME_FRAME: Duration = Duration::from_millis(4);
const STEP: f64 = 1. / 30.;

pub struct Stage {
    session: Option<String>,
    director: Director,
    inputs: Option<Inputs>,
    clock: Option<Instant>,
    cove: Cove,
    visible: bool,
}

/// A large surface's frame: the cove behind, the performance, the near
/// water in front.
#[derive(Clone, Debug)]
pub struct CoveScene {
    pub behind: Vec<Layer>,
    pub parts: Parts,
    pub front: Vec<Layer>,
}

impl Default for Stage {
    fn default() -> Self {
        Self::new()
    }
}

impl Stage {
    pub fn new() -> Self {
        Self {
            session: None,
            director: Director::new(Options::default()),
            inputs: None,
            clock: None,
            cove: Cove::default(),
            visible: true,
        }
    }

    /// Apply owner truth for the foreground `session`. A different session
    /// replaces the Director outright: no pending clip, calf count or
    /// completed-turn identity carries across. Returns whether anything
    /// the performance uses changed.
    pub fn observe(&mut self, session: Option<&str>, inputs: Inputs, reduced: bool) -> bool {
        let mut changed = self.director.reduced != reduced;
        if self.session.as_deref() != session {
            changed = true;
            self.session = session.map(str::to_string);
            self.director = Director::new(Options {
                reduced,
                ..Options::default()
            });
            self.inputs = None;
            self.clock = None;
            self.cove.reset();
        }
        self.director.set_reduced(reduced);
        if self.inputs.as_ref() != Some(&inputs) {
            self.director.set(
                inputs.presence,
                inputs.activity.clone(),
                inputs.context.clone(),
            );
            self.inputs = Some(inputs);
            changed = true;
        }
        changed
    }

    /// Advance the shared clock to `now` in steps of at most 1/30 s.
    pub fn advance(&mut self, now: Instant) {
        if !self.visible {
            self.clock = None;
            return;
        }
        let Some(last) = self.clock else {
            self.clock = Some(now);
            return;
        };
        let elapsed = now.saturating_duration_since(last);
        if elapsed < SAME_FRAME {
            return;
        }
        self.clock = Some(now);
        let mut remaining = elapsed.min(MAX_CATCH_UP).as_secs_f64();
        while remaining > 1e-9 {
            let dt = remaining.min(STEP);
            self.director.step(dt);
            remaining -= dt;
        }
    }

    pub fn scene(&self, view: View) -> Parts {
        scene(&self.director, view)
    }

    /// Stop painting while hidden. Resuming starts from current owner truth:
    /// no hidden interval, stale entry clip or particle is replayed. The host
    /// continues to call `observe` when its authoritative inputs change.
    pub fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        self.clock = None;
        if visible {
            let reduced = self.director.reduced;
            // Reuse the canonical settle/resume boundary. Completed-turn and
            // onset identities survive; only transient motion is discarded.
            self.director.set_reduced(true);
            self.director.set_reduced(reduced);
            self.cove.reset();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// The whale in its cove, for large surfaces. The cove rides the
    /// Director's clock and only ever adjusts the resting gaze.
    pub fn cove_scene(&mut self, view: View, dark: bool) -> CoveScene {
        let overrides = self.cove.pose(&self.director);
        CoveScene {
            behind: self.cove.behind(&self.director, dark),
            parts: scene_posed(&self.director, view, &overrides),
            front: self.cove.front(&self.director, dark),
        }
    }

    /// Pointer over the cove, in design units (`−62…62`, positive Y down).
    pub fn cove_observe(&mut self, x: f64, y: f64) {
        self.cove.observe(x, y);
    }

    pub fn cove_leave(&mut self) {
        self.cove.leave();
    }

    /// A decorative tap on the water. It never reaches the Director's
    /// inputs, so it cannot change presence, acting or Engine state.
    pub fn cove_tap(&mut self, x: f64, y: f64) {
        self.cove.tap(x, y, &self.director);
    }

    pub fn cove(&self) -> &Cove {
        &self.cove
    }

    pub fn acting(&self) -> Act {
        self.director.acting
    }

    pub fn director(&self) -> &Director {
        &self.director
    }

    /// How long until this surface's next paint, or `None` when nothing
    /// should be scheduled (reduced motion paints only on input changes).
    pub fn cadence(&self, tier: Tier) -> Option<Duration> {
        if self.director.reduced || !self.visible {
            return None;
        }
        let resting = matches!(self.director.acting, Act::Rest | Act::Sleep)
            && !self.director.in_transition()
            && !self.cove.rippling(&self.director);
        let hz = match (tier, resting) {
            (Tier::Hero, false) => 30.,
            (Tier::Hero, true) => 8.,
            (Tier::Small, false) => 15.,
            (Tier::Small, true) => 2.,
            (Tier::Terminal, false) => 6.,
            (Tier::Terminal, true) => 2.,
        };
        Some(Duration::from_secs_f64(1. / hz))
    }
}

/// Read the owner's `activity` object exactly as served. `observed` must
/// already be the owner's strict verdict; nothing is derived from a tool
/// name, label, argument or caption.
pub fn activity(value: &Value, observed: bool) -> Activity {
    Activity {
        kind: value["kind"].as_str().map(str::to_string),
        observed,
        parallel: value["parallel"].as_f64(),
        active: value["active"]
            .as_array()
            .map(|spans| {
                spans
                    .iter()
                    .take(4)
                    .filter_map(|span| {
                        Some(Span {
                            kind: span["kind"].as_str()?.to_string(),
                            since_ms: span["sinceMs"].as_f64().filter(|n| n.is_finite())?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}
