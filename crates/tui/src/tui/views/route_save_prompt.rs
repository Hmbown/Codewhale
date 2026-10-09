//! Route-save decisions.
//!
//! A `/model` or `/provider` change is temporary by default. Nothing is
//! written until the user runs one of the explicit commands: `/fleet save`
//! updates the selected Fleet, `/fleet save-as` saves the route as a new
//! Fleet, and `/model save-default` remembers it as the startup default. No
//! key is intercepted, so a scripted or automated terminal is never
//! interrupted.

/// The explicit persistence choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSaveChoice {
    /// Rewrite the selected Fleet's operator route to the session route
    /// (`/fleet save`).
    UpdateFleet,
    /// Save the session route as a brand-new Fleet (user-global) and select it
    /// (`/fleet save-as`).
    SaveAsNewFleet,
    /// Remember the session route as the startup default
    /// (`/model save-default`).
    SaveAsDefault,
}
