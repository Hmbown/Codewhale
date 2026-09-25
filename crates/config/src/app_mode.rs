//! The TUI's user-facing operating mode. Lives in codewhale-config so
//! settings, receipts, and other crates can name it without depending on
//! the TUI; the TUI adds the localized picker strings through an extension
//! trait.

/// Supported application modes for the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Agent,
    Plan,
    Operate,
}

impl AppMode {
    /// Productive keyboard cycle: Plan -> Act -> Operate -> Plan.
    ///
    /// Operate joins the visible cycle as the always-on fleet operation:
    /// a lead plans slices, then workers execute against an optional burn rate.
    pub const CYCLE: [Self; 3] = [Self::Plan, Self::Agent, Self::Operate];

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "agent" | "act" | "work" | "auto" | "1" => Some(Self::Agent),
            "plan" | "2" => Some(Self::Plan),
            "operate" | "operation" | "ops" | "3" => Some(Self::Operate),
            // Invisible one-way permission shorthand only — never a visible
            // mode. These spellings resolve to Act; the bypass posture they
            // imply is carried by the permission surface (settings load,
            // CLI/runtime wire), not by a mode.
            other if Self::is_legacy_bypass_alias(other) => Some(Self::Agent),
            _ => None,
        }
    }

    /// Legacy mode spellings (`yolo`, `4`, `bypass`, `bypass-permissions`,
    /// `bypasspermissions`) that carried the Full Access posture. [`Self::parse`]
    /// folds them to Act; callers that must preserve the permission meaning
    /// (the `/mode` command, the runtime policy wire, persisted thread records)
    /// ask this one predicate instead of keeping their own copy of the list.
    #[must_use]
    pub fn is_legacy_bypass_alias(value: &str) -> bool {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "yolo" | "4" | "bypass" | "bypass-permissions" | "bypasspermissions"
        )
    }

    #[must_use]
    pub fn from_setting(value: &str) -> Self {
        // Unreleased Multitask never shipped; normalize leftover settings to Operate.
        match value.trim().to_ascii_lowercase().as_str() {
            "multitask" | "multi" | "5" => Self::Operate,
            other => Self::parse(other).unwrap_or(Self::Agent),
        }
    }

    #[must_use]
    pub fn as_setting(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Plan => "plan",
            Self::Operate => "operate",
        }
    }

    /// Short label used in the UI footer.
    pub fn label(self) -> &'static str {
        match self {
            AppMode::Agent => "ACT",
            AppMode::Plan => "PLAN",
            AppMode::Operate => "OPERATE",
        }
    }

    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            AppMode::Agent => "Act",
            AppMode::Plan => "Plan",
            AppMode::Operate => "Operate",
        }
    }

    #[must_use]
    pub fn number(self) -> char {
        match self {
            AppMode::Agent => '1',
            AppMode::Plan => '2',
            AppMode::Operate => '3',
        }
    }

    #[must_use]
    pub fn uses_agent_baseline(self) -> bool {
        matches!(self, Self::Agent | Self::Operate)
    }

    /// Operate gets a higher parallel launch floor so background fan-out is
    /// not throttled to a single slot when config is low.
    #[must_use]
    pub fn mode_delegation_launch_floor(self) -> usize {
        match self {
            Self::Operate => 4,
            _ => 1,
        }
    }

    /// Description shown in help or onboarding text.
    pub fn description(self) -> &'static str {
        match self {
            AppMode::Agent => "Act mode - direct work in the current session with tools",
            AppMode::Plan => "Plan mode - research and design before implementing",
            AppMode::Operate => {
                "Operate mode - always-on fleet operation: lead plans, optional $/time burn rate, workers follow the plan"
            }
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        let Some(index) = Self::CYCLE.iter().position(|mode| *mode == self) else {
            return Self::Agent;
        };
        Self::CYCLE[(index + 1) % Self::CYCLE.len()]
    }

    #[must_use]
    pub fn previous(self) -> Self {
        let Some(index) = Self::CYCLE.iter().position(|mode| *mode == self) else {
            return Self::Agent;
        };
        Self::CYCLE[(index + Self::CYCLE.len() - 1) % Self::CYCLE.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_mode_helpers_centralize_parse_labels_and_cycle_order() {
        assert_eq!(AppMode::parse("agent"), Some(AppMode::Agent));
        assert_eq!(AppMode::parse("act"), Some(AppMode::Agent));
        assert_eq!(AppMode::parse("work"), Some(AppMode::Agent));
        assert_eq!(AppMode::parse("2"), Some(AppMode::Plan));
        assert_eq!(AppMode::parse("auto"), Some(AppMode::Agent));
        assert_eq!(AppMode::parse("3"), Some(AppMode::Operate));
        assert_eq!(AppMode::parse("operate"), Some(AppMode::Operate));
        // Legacy YOLO spellings resolve to Act; the bypass posture they imply
        // travels on the permission surface, not on a mode.
        assert_eq!(AppMode::parse("YOLO"), Some(AppMode::Agent));
        assert_eq!(AppMode::parse("4"), Some(AppMode::Agent));
        assert_eq!(AppMode::parse("bypass"), Some(AppMode::Agent));
        assert_eq!(AppMode::parse("bypass-permissions"), Some(AppMode::Agent));
        for alias in [
            "yolo",
            " YOLO ",
            "4",
            "bypass",
            "bypass-permissions",
            "BypassPermissions",
        ] {
            assert!(AppMode::is_legacy_bypass_alias(alias), "{alias}");
        }
        for mode in ["agent", "act", "plan", "operate", "1", "full-access", ""] {
            assert!(!AppMode::is_legacy_bypass_alias(mode), "{mode}");
        }
        assert_eq!(AppMode::parse("multitask"), None);
        assert_eq!(AppMode::parse("5"), None);
        assert_eq!(AppMode::parse("fast"), None);
        assert_eq!(AppMode::from_setting("multitask"), AppMode::Operate);
        assert_eq!(AppMode::from_setting("5"), AppMode::Operate);

        assert_eq!(AppMode::Agent.as_setting(), "agent");
        assert_eq!(AppMode::Plan.display_name(), "Plan");
        assert_eq!(AppMode::Agent.number(), '1');
        assert_eq!(AppMode::Operate.number(), '3');
        assert_eq!(
            AppMode::CYCLE,
            [AppMode::Plan, AppMode::Agent, AppMode::Operate]
        );

        assert_eq!(AppMode::Plan.next(), AppMode::Agent);
        assert_eq!(AppMode::Agent.next(), AppMode::Operate);
        assert_eq!(AppMode::Operate.next(), AppMode::Plan);
        assert_eq!(AppMode::Plan.previous(), AppMode::Operate);
        assert_eq!(AppMode::Agent.previous(), AppMode::Plan);
        assert_eq!(AppMode::Operate.previous(), AppMode::Agent);
    }

    /// The durable form of a mode is the `as_setting()` string persisted into
    /// settings and session records — `AppMode` derives no `Serialize`, so the
    /// round-trip that has to hold is string -> mode -> string.
    #[test]
    fn setting_strings_round_trip_for_every_mode() {
        for mode in AppMode::CYCLE {
            let setting = mode.as_setting();
            assert_eq!(
                AppMode::from_setting(setting),
                mode,
                "from_setting({setting})"
            );
            assert_eq!(AppMode::parse(setting), Some(mode), "parse({setting})");
        }

        assert_eq!(AppMode::Agent.as_setting(), "agent");
        assert_eq!(AppMode::Plan.as_setting(), "plan");
        assert_eq!(AppMode::Operate.as_setting(), "operate");
    }

    /// `from_setting` is the de-facto default: an absent, empty, or unreadable
    /// stored value must land on Act rather than panicking or picking Operate.
    #[test]
    fn from_setting_falls_back_to_act_for_unknown_values() {
        assert_eq!(AppMode::from_setting(""), AppMode::Agent);
        assert_eq!(AppMode::from_setting("   "), AppMode::Agent);
        assert_eq!(AppMode::from_setting("nonsense"), AppMode::Agent);
        assert_eq!(AppMode::from_setting("OPERATE"), AppMode::Operate);
    }

    #[test]
    fn labels_and_descriptions_cover_every_mode() {
        assert_eq!(AppMode::Agent.label(), "ACT");
        assert_eq!(AppMode::Plan.label(), "PLAN");
        assert_eq!(AppMode::Operate.label(), "OPERATE");

        assert_eq!(AppMode::Agent.display_name(), "Act");
        assert_eq!(AppMode::Plan.display_name(), "Plan");
        assert_eq!(AppMode::Operate.display_name(), "Operate");

        for mode in AppMode::CYCLE {
            assert!(!mode.description().is_empty());
        }
    }

    #[test]
    fn operate_shares_the_agent_baseline_and_raises_the_launch_floor() {
        assert!(AppMode::Agent.uses_agent_baseline());
        assert!(AppMode::Operate.uses_agent_baseline());
        assert!(!AppMode::Plan.uses_agent_baseline());

        assert_eq!(AppMode::Operate.mode_delegation_launch_floor(), 4);
        assert_eq!(AppMode::Agent.mode_delegation_launch_floor(), 1);
        assert_eq!(AppMode::Plan.mode_delegation_launch_floor(), 1);
    }
}
