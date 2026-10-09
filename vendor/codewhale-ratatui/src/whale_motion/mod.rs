//! Codewhale's canonical Whale v2 performance, shared by every terminal surface.
//!
//! The native core is adapted from `codewhale-app/src/whale` at
//! `bc1044887b4ef973e444b6f8f83f9389daffee2a`; see
//! `assets/whale-motion/PROVENANCE.md`. One host-owned [`Director`] accepts
//! authoritative owner inputs. Springs, authored clips, particles and the rig
//! produce the live pose; widgets only paint it. Drop the Director when the
//! foreground identity changes, or use [`Stage`] to manage that boundary.
//!
//! [`braille`] retains the canonical one-ink packed geometry. [`colored_braille`]
//! adds the native blue ombre and prop inks to those same dots; a terminal cell
//! can carry one foreground ink, so its majority visible shape wins, with later
//! paint order breaking ties. The caller supplies localized state words.
//! Reduced motion settles on the authored poster and schedules no animation.

pub mod acting;
pub mod data;
pub mod habitat;
pub mod ink;
pub mod math;
pub(crate) mod pixels;
pub mod props;
pub mod rig;
pub mod scene;
pub mod stage;

pub use acting::{Activity, Context, Director, Event, Options, Presence, Span, acting_for};
pub use data::Act;
pub use habitat::{Cove, Layer};
pub use rig::{Cmd, Parts, Path, Role, Shape, holes_for};
pub use scene::{
    ColoredGrid, Grid, View, braille, colored_braille, rasterize, rasterize_colored, scene,
    scene_posed,
};
pub use stage::{CoveScene, Inputs, Stage, Tier};

#[cfg(test)]
mod tests;
