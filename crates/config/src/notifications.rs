//! Shared notification configuration and typed, lossless leaf edits.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::{Path, PathBuf};

impl NotificationCondition {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "always" => Some(Self::Always),
            "unfocused" | "away" => Some(Self::Unfocused),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Unfocused => "unfocused",
            Self::Never => "never",
        }
    }
}

/// Stable configuration vocabulary shared by category and sound policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationEvent {
    TurnComplete,
    SubagentTerminal,
    ApprovalNeeded,
    InputNeeded,
    ElevationNeeded,
    ModelNotify,
}

impl NotificationEvent {
    pub const ALL: [Self; 6] = [
        Self::TurnComplete,
        Self::SubagentTerminal,
        Self::ApprovalNeeded,
        Self::InputNeeded,
        Self::ElevationNeeded,
        Self::ModelNotify,
    ];
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TurnComplete => "turn-complete",
            Self::SubagentTerminal => "subagent-terminal",
            Self::ApprovalNeeded => "approval-needed",
            Self::InputNeeded => "input-needed",
            Self::ElevationNeeded => "elevation-needed",
            Self::ModelNotify => "model-notify",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.as_str() == value)
    }
    pub const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSetting {
    Method,
    ThresholdSecs,
    IncludeSummary,
    Quiet,
    Sound,
    Condition,
    CompletionSound,
    SubagentCompletion,
    SoundFile,
    Event(NotificationEvent),
    EventSoundEnabled,
    EventSoundEvents,
    EventSoundMinIntervalMs,
    EventSoundQuiet,
}

impl NotificationSetting {
    pub const ALL: [Self; 19] = [
        Self::Method,
        Self::ThresholdSecs,
        Self::IncludeSummary,
        Self::Quiet,
        Self::Sound,
        Self::Condition,
        Self::CompletionSound,
        Self::SubagentCompletion,
        Self::SoundFile,
        Self::Event(NotificationEvent::TurnComplete),
        Self::Event(NotificationEvent::SubagentTerminal),
        Self::Event(NotificationEvent::ApprovalNeeded),
        Self::Event(NotificationEvent::InputNeeded),
        Self::Event(NotificationEvent::ElevationNeeded),
        Self::Event(NotificationEvent::ModelNotify),
        Self::EventSoundEnabled,
        Self::EventSoundEvents,
        Self::EventSoundMinIntervalMs,
        Self::EventSoundQuiet,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Method => "method",
            Self::ThresholdSecs => "threshold_secs",
            Self::IncludeSummary => "include_summary",
            Self::Quiet => "quiet",
            Self::Sound => "sound",
            Self::Condition => "condition",
            Self::CompletionSound => "completion_sound",
            Self::SubagentCompletion => "subagent_completion",
            Self::SoundFile => "sound_file",
            Self::Event(NotificationEvent::TurnComplete) => "events.turn-complete",
            Self::Event(NotificationEvent::SubagentTerminal) => "events.subagent-terminal",
            Self::Event(NotificationEvent::ApprovalNeeded) => "events.approval-needed",
            Self::Event(NotificationEvent::InputNeeded) => "events.input-needed",
            Self::Event(NotificationEvent::ElevationNeeded) => "events.elevation-needed",
            Self::Event(NotificationEvent::ModelNotify) => "events.model-notify",
            Self::EventSoundEnabled => "event_sound.enabled",
            Self::EventSoundEvents => "event_sound.events",
            Self::EventSoundMinIntervalMs => "event_sound.min_interval_ms",
            Self::EventSoundQuiet => "event_sound.quiet",
        }
    }
    pub fn parse(key: &str) -> Option<Self> {
        let key = key.trim().to_ascii_lowercase();
        let key = key.strip_prefix("notifications.").unwrap_or(&key);
        let key = match key {
            "threshold" => "threshold_secs",
            "summary" => "include_summary",
            other => other,
        };
        Self::ALL
            .into_iter()
            .find(|setting| setting.key().replace('-', "_") == key.replace('-', "_"))
    }
    pub fn required(key: &str) -> Result<Self> {
        Self::parse(key).context("unknown notification setting; use config get notifications or /config notifications status")
    }
    pub fn segments(self) -> Vec<&'static str> {
        std::iter::once("notifications")
            .chain(self.key().split('.'))
            .collect()
    }
    pub const fn choices(self) -> &'static str {
        match self {
            Self::Method => "auto, osc9, bel, kitty, ghostty, off",
            Self::Sound => "off, whale, bell, beep, file, legacy",
            Self::CompletionSound => "off, whale, bell, beep, file",
            Self::Condition => "always, unfocused, never",
            Self::SubagentCompletion => "always, final-only, off",
            Self::ThresholdSecs | Self::EventSoundMinIntervalMs => "0..=9223372036854775807",
            Self::SoundFile => "\"/path/call.wav\"",
            Self::EventSoundEvents => r#"["turn-complete", "approval-needed", ...]"#,
            _ => "on, off, true, false, yes, no",
        }
    }
    pub fn unset(self, path: &Path) -> Result<()> {
        crate::mutate_config_document(path, |doc| {
            crate::unset_config_document_value(doc, &self.segments())?;
            crate::unset_config_document_value(doc, &[&format!("notifications.{}", self.key())])?;
            Ok(())
        })
    }
}

/// One validated live edit, also used for CLI overlays and targeted persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationConfigUpdate {
    Method(NotificationMethod),
    ThresholdSecs(u64),
    IncludeSummary(bool),
    Quiet(bool),
    Sound(Option<CompletionSound>),
    Condition(NotificationCondition),
    CompletionSound(CompletionSound),
    SubagentCompletion(SubagentCompletionNotification),
    SoundFile(PathBuf),
    Event(NotificationEvent, bool),
    EventSoundEnabled(bool),
    EventSoundEvents(Vec<String>),
    EventSoundMinIntervalMs(u64),
    EventSoundQuiet(bool),
}

impl NotificationConfigUpdate {
    pub fn parse(setting: NotificationSetting, raw: &str) -> Result<Self> {
        use NotificationSetting as K;
        let boolean = || match raw.trim().to_ascii_lowercase().as_str() {
            "true" | "on" | "yes" | "1" => Ok(true),
            "false" | "off" | "no" | "0" => Ok(false),
            _ => bail!("expected a notification boolean"),
        };
        let integer = || {
            let value: u64 = raw
                .trim()
                .parse()
                .context("expected a nonnegative notification integer")?;
            anyhow::ensure!(
                value <= i64::MAX as u64,
                "notification integer exceeds TOML range"
            );
            Ok(value)
        };
        Ok(match setting {
            K::Method => {
                Self::Method(NotificationMethod::parse(raw).context("invalid notification method")?)
            }
            K::ThresholdSecs => Self::ThresholdSecs(integer()?),
            K::IncludeSummary => Self::IncludeSummary(boolean()?),
            K::Quiet => Self::Quiet(boolean()?),
            K::Sound if raw.trim().eq_ignore_ascii_case("legacy") => Self::Sound(None),
            K::Sound => Self::Sound(Some(
                CompletionSound::parse(raw).context("invalid notification sound")?,
            )),
            K::CompletionSound => Self::CompletionSound(
                CompletionSound::parse(raw).context("invalid completion sound")?,
            ),
            K::Condition => Self::Condition(
                NotificationCondition::parse(raw).context("invalid notification condition")?,
            ),
            K::SubagentCompletion => Self::SubagentCompletion(
                SubagentCompletionNotification::parse(raw)
                    .context("invalid subagent notification policy")?,
            ),
            K::SoundFile => {
                let value = raw.trim();
                let value = if value.starts_with(['\'', '"']) {
                    let parsed: toml::Table = toml::from_str(&format!("value = {value}"))
                        .context("invalid quoted sound path")?;
                    parsed
                        .get("value")
                        .and_then(toml::Value::as_str)
                        .context("expected sound path string")?
                        .to_string()
                } else {
                    value.to_string()
                };
                anyhow::ensure!(
                    !value.is_empty()
                        && value.len() <= 4096
                        && !value.chars().any(char::is_control),
                    "invalid sound path"
                );
                Self::SoundFile(PathBuf::from(value))
            }
            K::Event(event) => Self::Event(event, boolean()?),
            K::EventSoundEnabled => Self::EventSoundEnabled(boolean()?),
            K::EventSoundQuiet => Self::EventSoundQuiet(boolean()?),
            K::EventSoundMinIntervalMs => Self::EventSoundMinIntervalMs(integer()?),
            K::EventSoundEvents => {
                anyhow::ensure!(raw.len() <= 1024, "notification event list is too large");
                let parsed: toml::Table = toml::from_str(&format!("events = {raw}"))
                    .context("expected TOML notification event array")?;
                let values = parsed
                    .get("events")
                    .and_then(toml::Value::as_array)
                    .context("expected notification event array")?;
                anyhow::ensure!(values.len() <= 6, "too many notification event names");
                let mut events = Vec::new();
                for value in values {
                    let event = value
                        .as_str()
                        .and_then(NotificationEvent::parse)
                        .context("unknown notification event")?;
                    if !events.iter().any(|value| value == event.as_str()) {
                        events.push(event.as_str().to_string());
                    }
                }
                Self::EventSoundEvents(events)
            }
        })
    }
    pub const fn setting(&self) -> NotificationSetting {
        use NotificationSetting as K;
        match self {
            Self::Method(_) => K::Method,
            Self::ThresholdSecs(_) => K::ThresholdSecs,
            Self::IncludeSummary(_) => K::IncludeSummary,
            Self::Quiet(_) => K::Quiet,
            Self::Sound(_) => K::Sound,
            Self::Condition(_) => K::Condition,
            Self::CompletionSound(_) => K::CompletionSound,
            Self::SubagentCompletion(_) => K::SubagentCompletion,
            Self::SoundFile(_) => K::SoundFile,
            Self::Event(event, _) => K::Event(*event),
            Self::EventSoundEnabled(_) => K::EventSoundEnabled,
            Self::EventSoundEvents(_) => K::EventSoundEvents,
            Self::EventSoundMinIntervalMs(_) => K::EventSoundMinIntervalMs,
            Self::EventSoundQuiet(_) => K::EventSoundQuiet,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::ThresholdSecs(v) | Self::EventSoundMinIntervalMs(v) => anyhow::ensure!(
                *v <= i64::MAX as u64,
                "notification integer exceeds TOML range"
            ),
            Self::SoundFile(path) => {
                let value = path.to_str().context("sound path must be UTF-8")?;
                anyhow::ensure!(
                    !value.is_empty()
                        && value.len() <= 4096
                        && !value.chars().any(char::is_control),
                    "invalid sound path"
                );
            }
            Self::EventSoundEvents(events) => {
                anyhow::ensure!(events.len() <= 6, "too many notification events");
                for (index, event) in events.iter().enumerate() {
                    anyhow::ensure!(
                        NotificationEvent::parse(event).is_some()
                            && !events[..index].contains(event),
                        "invalid or duplicate notification event"
                    );
                }
            }
            _ => (),
        }
        Ok(())
    }
    pub fn value(&self) -> Result<Option<toml::Value>> {
        self.validate()?;
        Ok(Some(match self {
            Self::Method(value) => value.as_str().into(),
            Self::Sound(Some(value)) | Self::CompletionSound(value) => value.as_str().into(),
            Self::Sound(None) => return Ok(None),
            Self::Condition(value) => value.as_str().into(),
            Self::SubagentCompletion(value) => value.as_str().into(),
            Self::ThresholdSecs(value) | Self::EventSoundMinIntervalMs(value) => {
                (*value as i64).into()
            }
            Self::IncludeSummary(value)
            | Self::Quiet(value)
            | Self::Event(_, value)
            | Self::EventSoundEnabled(value)
            | Self::EventSoundQuiet(value) => (*value).into(),
            Self::SoundFile(value) => value.to_string_lossy().into_owned().into(),
            Self::EventSoundEvents(values) => {
                toml::Value::Array(values.iter().cloned().map(toml::Value::String).collect())
            }
        }))
    }
    pub fn display(&self) -> String {
        self.value()
            .map(display_value)
            .unwrap_or_else(|_| "<invalid notification edit>".into())
    }
    pub fn persist(&self, path: &Path) -> Result<()> {
        self.persist_for_profile(path, None)
    }

    /// Edit the existing notification owner. A profile with a notification
    /// table owns that whole table under the current Config merge semantics;
    /// otherwise the root table remains the owner. Never create an empty
    /// profile override that would reset inherited notification choices.
    pub fn persist_for_profile(&self, path: &Path, profile: Option<&str>) -> Result<()> {
        self.validate()?;
        crate::mutate_config_document(path, |doc| {
            let mut prefix = Vec::new();
            if let Some(profile) = profile {
                let table = doc
                    .get("profiles")
                    .and_then(|profiles| profiles.get(profile))
                    .and_then(toml_edit::Item::as_table_like)
                    .context("active profile is missing or malformed")?;
                if table.contains_key("notifications") {
                    prefix.extend(["profiles", profile]);
                }
            }
            let mut segments = prefix.clone();
            segments.extend(self.setting().segments());
            if let Some(value) = self.value()? {
                let value = match value {
                    toml::Value::String(v) => toml_edit::Value::from(v),
                    toml::Value::Integer(v) => v.into(),
                    toml::Value::Boolean(v) => v.into(),
                    toml::Value::Array(v) => toml_edit::Value::Array(
                        v.into_iter()
                            .map(|v| v.as_str().expect("validated string array").to_string())
                            .collect(),
                    ),
                    _ => unreachable!("notification leaf"),
                };
                crate::set_config_document_value(doc, &segments, value)?;
            } else {
                crate::unset_config_document_value(doc, &segments)?;
            }
            let old_literal = format!("notifications.{}", self.setting().key());
            prefix.push(&old_literal);
            crate::unset_config_document_value(doc, &prefix)?;
            Ok(())
        })
    }
}

/// Decode the existing raw config table without taking ownership of its unknown fields.
pub fn from_extras(
    extras: &std::collections::BTreeMap<String, toml::Value>,
) -> Result<NotificationsConfig> {
    let mut config: NotificationsConfig = extras
        .get("notifications")
        .cloned()
        .unwrap_or_else(|| toml::Value::Table(toml::Table::new()))
        .try_into()
        .context("invalid notification configuration")?;
    if config.condition.is_none() {
        config.condition = extras
            .get("tui")
            .and_then(|tui| tui.get("notification_condition"))
            .cloned()
            .map(toml::Value::try_into)
            .transpose()
            .context("invalid legacy notification condition")?;
    }
    config.condition = Some(config.condition.unwrap_or(NotificationCondition::Unfocused));
    Ok(config)
}

/// Apply a validated leaf to CLI's raw table; siblings and future keys survive.
pub fn in_namespace(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key == "notifications" || key.starts_with("notifications.")
}

pub fn edit_extras(
    extras: &mut std::collections::BTreeMap<String, toml::Value>,
    setting: NotificationSetting,
    value: Option<toml::Value>,
) -> Result<()> {
    if let Some(value) = &value {
        let parsed = match (setting, value) {
            (NotificationSetting::SoundFile, toml::Value::String(path)) => {
                let update = NotificationConfigUpdate::SoundFile(PathBuf::from(path));
                update.validate()?;
                update
            }
            _ => NotificationConfigUpdate::parse(setting, &display_value(Some(value.clone())))?,
        };
        anyhow::ensure!(
            parsed.value()?.as_ref() == Some(value),
            "notification leaf has the wrong type or noncanonical value"
        );
    }
    let mut updated = extras.clone();
    if value.is_some() || updated.contains_key("notifications") {
        let root = updated
            .entry("notifications".into())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        let mut table = root
            .as_table_mut()
            .context("notifications must be a TOML table")?;
        let segments = setting.key().split('.').collect::<Vec<_>>();
        let (key, parents) = segments.split_last().expect("notification key");
        let mut missing = false;
        for parent in parents {
            if value.is_none() && !table.contains_key(*parent) {
                missing = true;
                break;
            }
            table = table
                .entry((*parent).to_string())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .context("notification parent must be a TOML table")?;
        }
        if !missing {
            if let Some(value) = value {
                table.insert((*key).into(), value);
            } else {
                table.remove(*key);
            }
        }
    }
    updated.remove(&format!("notifications.{}", setting.key()));
    *extras = updated;
    Ok(())
}

fn display_value(value: Option<toml::Value>) -> String {
    match value {
        Some(toml::Value::String(value)) => value,
        Some(value) => value.to_string(),
        None => "legacy".into(),
    }
}

impl NotificationsConfig {
    pub fn apply_update(&mut self, update: NotificationConfigUpdate) -> Result<()> {
        update.validate()?;
        match update {
            NotificationConfigUpdate::Method(v) => self.method = v,
            NotificationConfigUpdate::ThresholdSecs(v) => self.threshold_secs = v,
            NotificationConfigUpdate::IncludeSummary(v) => self.include_summary = v,
            NotificationConfigUpdate::Quiet(v) => self.quiet = v,
            NotificationConfigUpdate::Sound(v) => self.sound = v,
            NotificationConfigUpdate::Condition(v) => self.condition = Some(v),
            NotificationConfigUpdate::CompletionSound(v) => self.completion_sound = v,
            NotificationConfigUpdate::SubagentCompletion(v) => self.subagent_completion = v,
            NotificationConfigUpdate::SoundFile(v) => self.sound_file = Some(v),
            NotificationConfigUpdate::Event(event, value) => *self.events.value_mut(event) = value,
            NotificationConfigUpdate::EventSoundEnabled(v) => self.event_sound.enabled = v,
            NotificationConfigUpdate::EventSoundEvents(v) => self.event_sound.events = v,
            NotificationConfigUpdate::EventSoundMinIntervalMs(v) => {
                self.event_sound.min_interval_ms = v
            }
            NotificationConfigUpdate::EventSoundQuiet(v) => self.event_sound.quiet = v,
        }
        Ok(())
    }
    pub fn display(&self, setting: NotificationSetting) -> String {
        use NotificationSetting as K;
        match setting {
            K::Method => self.method.as_str().into(),
            K::ThresholdSecs => self.threshold_secs.to_string(),
            K::IncludeSummary => self.include_summary.to_string(),
            K::Quiet => self.quiet.to_string(),
            K::Sound => self.sound.map_or("legacy", CompletionSound::as_str).into(),
            K::Condition => self
                .condition
                .unwrap_or(NotificationCondition::Unfocused)
                .as_str()
                .into(),
            K::CompletionSound => self.completion_sound.as_str().into(),
            K::SubagentCompletion => self.subagent_completion.as_str().into(),
            K::SoundFile => self
                .sound_file
                .as_ref()
                .map_or_else(String::new, |path| path.to_string_lossy().into_owned()),
            K::Event(event) => self.events.value(event).to_string(),
            K::EventSoundEnabled => self.event_sound.enabled.to_string(),
            K::EventSoundQuiet => self.event_sound.quiet.to_string(),
            K::EventSoundMinIntervalMs => self.event_sound.min_interval_ms.to_string(),
            K::EventSoundEvents => toml::Value::Array(
                self.event_sound
                    .events
                    .iter()
                    .cloned()
                    .map(toml::Value::String)
                    .collect(),
            )
            .to_string(),
        }
    }
}

impl NotificationEventsConfig {
    pub fn value(&self, event: NotificationEvent) -> bool {
        match event {
            NotificationEvent::TurnComplete => self.turn_complete,
            NotificationEvent::SubagentTerminal => self.subagent_terminal,
            NotificationEvent::ApprovalNeeded => self.approval_needed,
            NotificationEvent::InputNeeded => self.input_needed,
            NotificationEvent::ElevationNeeded => self.elevation_needed,
            NotificationEvent::ModelNotify => self.model_notify,
        }
    }
    fn value_mut(&mut self, event: NotificationEvent) -> &mut bool {
        match event {
            NotificationEvent::TurnComplete => &mut self.turn_complete,
            NotificationEvent::SubagentTerminal => &mut self.subagent_terminal,
            NotificationEvent::ApprovalNeeded => &mut self.approval_needed,
            NotificationEvent::InputNeeded => &mut self.input_needed,
            NotificationEvent::ElevationNeeded => &mut self.elevation_needed,
            NotificationEvent::ModelNotify => &mut self.model_notify,
        }
    }
}

/// High-level notification trigger override. See
/// The canonical notification condition; the old TUI field remains a read fallback.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationCondition {
    /// Allow configured operator notifications in the foreground; completed
    /// turns have no duration threshold.
    Always,
    /// Notify only while the terminal is genuinely in the background.
    Unfocused,
    /// Suppress all operator notifications.
    Never,
}

/// Notification delivery method (mirrors `tui::notifications::Method`).
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationMethod {
    /// Auto-detect: picks the best protocol for the current terminal
    /// (OSC 9, Kitty OSC 99, Ghostty OSC 777, or Bel).
    #[default]
    Auto,
    /// OSC 9 escape.
    Osc9,
    /// Plain BEL character.
    Bel,
    /// Kitty notification protocol (OSC 99).
    Kitty,
    /// Ghostty notification protocol (OSC 777).
    Ghostty,
    /// Disable notifications.
    Off,
}

impl NotificationMethod {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "osc9" | "osc-9" | "osc_9" => Some(Self::Osc9),
            "bel" | "bell" => Some(Self::Bel),
            "kitty" => Some(Self::Kitty),
            "ghostty" => Some(Self::Ghostty),
            "off" | "none" | "disable" | "disabled" => Some(Self::Off),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Osc9 => "osc9",
            Self::Bel => "bel",
            Self::Kitty => "kitty",
            Self::Ghostty => "ghostty",
            Self::Off => "off",
        }
    }

    #[must_use]
    pub fn names_hint() -> &'static str {
        "auto, osc9, bel, kitty, ghostty, off"
    }
}

fn default_threshold_secs() -> u64 {
    30
}

/// Completion sound options.
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CompletionSound {
    /// No sound on turn completion.
    #[default]
    Off,
    /// System notification beep. On Windows uses `MessageBeep`.
    Beep,
    /// Terminal BEL character (`\x07`).
    Bell,
    /// Play the bundled Codewhale whale call.
    Whale,
    /// Play a configured WAV sound file.
    File,
}

impl CompletionSound {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "disable" | "disabled" => Some(Self::Off),
            "beep" => Some(Self::Beep),
            "bell" | "bel" => Some(Self::Bell),
            "file" => Some(Self::File),
            "whale" => Some(Self::Whale),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Beep => "beep",
            Self::Bell => "bell",
            Self::File => "file",
            Self::Whale => "whale",
        }
    }

    #[must_use]
    pub fn names_hint() -> &'static str {
        "off, whale, bell, beep, file"
    }
}

/// Controls when per-subagent completion notifications fire during fleet /
/// workflow runs. Turn-completion notifications are unaffected.
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SubagentCompletionNotification {
    /// Notify on every subagent completion.
    Always,
    /// One notice for a batch of finished background work (sub-agents,
    /// background shells, durable tasks), sent once no agent, workflow run or
    /// durable task is still running. A running shell never holds it back.
    /// Default: stays quiet mid-run and fires once when the batch drains.
    #[default]
    FinalOnly,
    /// Never fire a subagent-completion notification.
    Off,
}

impl SubagentCompletionNotification {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "always" => Some(Self::Always),
            "final-only" | "finalonly" | "final" => Some(Self::FinalOnly),
            "off" | "none" | "never" | "disable" | "disabled" => Some(Self::Off),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::FinalOnly => "final-only",
            Self::Off => "off",
        }
    }

    #[must_use]
    pub fn names_hint() -> &'static str {
        "always, final-only, off"
    }
}

/// Operator notification configuration (native and terminal transports).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct NotificationsConfig {
    /// One sound choice for every enabled category. Absent preserves legacy sound settings.
    #[serde(default)]
    pub sound: Option<CompletionSound>,
    /// Canonical attention condition; absent falls back to the legacy TUI field.
    #[serde(default)]
    pub condition: Option<NotificationCondition>,
    /// Delivery method: `auto` | `osc9` | `kitty` | `ghostty` | `bel` |
    /// `off`. Default: `auto`.
    /// `auto` resolves to OSC 9 for iTerm.app / Ghostty / WezTerm / Cmux
    /// (detected via `$TERM_PROGRAM` then `$LC_TERMINAL`) and the native macOS
    /// transport where appropriate; unknown terminals fail closed to `off`.
    /// Audible BEL is explicit only. On Windows explicit BEL is routed through
    /// `MessageBeep(MB_OK)`.
    /// Use `method = "osc9"` explicitly when your terminal is OSC-9 capable
    /// but sets neither env var (e.g. Cmux without `LC_TERMINAL`).
    #[serde(default)]
    pub method: NotificationMethod,
    /// Only notify when the turn took at least this many seconds. Default: 30.
    #[serde(default = "default_threshold_secs")]
    pub threshold_secs: u64,
    /// Include a short summary (elapsed time + cost) in the notification body.
    /// Default: `false`.
    #[serde(default)]
    pub include_summary: bool,

    /// When to fire per-subagent completion notifications during fleet /
    /// workflow runs: `always` | `final-only` | `off`. Default: `final-only`
    /// (quiet mid-run, one notification when the batch drains). Set `off` to
    /// silence subagent notifications entirely.
    #[serde(default)]
    pub subagent_completion: SubagentCompletionNotification,

    /// Legacy completion cue, used only when `sound` is absent. Default: `"off"`.
    /// This is opt-in and follows the same foreground/quiet attention policy
    /// as desktop notifications.
    #[serde(default)]
    pub completion_sound: CompletionSound,

    /// Local WAV path for the File choice in canonical or legacy sound mode.
    #[serde(default)]
    pub sound_file: Option<PathBuf>,

    /// Opt-in per-event sound policy (`[notifications.event_sound]`).
    /// Disabled by default; canonical `sound` overrides its enable/list/quiet choices.
    #[serde(default)]
    pub event_sound: EventSoundConfig,

    /// Quiet mode: suppress every desktop notification (all categories, all
    /// delivery methods) and the paired `[notifications.event_sound]` cues,
    /// without editing `method`, `completion_sound`, or the per-category
    /// switches under `[notifications.events]`. Default: `false`.
    #[serde(default)]
    pub quiet: bool,

    /// Per-category desktop-notification switches
    /// (`[notifications.events]`). Every category defaults to enabled; set
    /// one to `false` to silence that event kind without touching the rest.
    #[serde(default)]
    pub events: NotificationEventsConfig,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            sound: None,
            condition: None,
            method: NotificationMethod::default(),
            threshold_secs: default_threshold_secs(),
            include_summary: false,
            subagent_completion: SubagentCompletionNotification::default(),
            completion_sound: CompletionSound::default(),
            sound_file: None,
            event_sound: EventSoundConfig::default(),
            quiet: false,
            events: NotificationEventsConfig::default(),
        }
    }
}

fn default_notification_event_enabled() -> bool {
    true
}

/// Per-category desktop-notification switches (`[notifications.events]`).
///
/// Categories mirror the closed set of notification kinds in
/// `tui::notification_payload::NotificationKind`. Each defaults to `true`;
/// a disabled category is suppressed across every delivery mechanism
/// (OSC 9, Kitty OSC 99, Ghostty OSC 777, BEL, macOS Notification Center).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct NotificationEventsConfig {
    /// An agent turn finished successfully. Default: `true`.
    #[serde(default = "default_notification_event_enabled")]
    pub turn_complete: bool,
    /// A sub-agent reached a terminal status. Default: `true`.
    #[serde(default = "default_notification_event_enabled")]
    pub subagent_terminal: bool,
    /// A tool call is blocked waiting for approval. Default: `true`.
    #[serde(default = "default_notification_event_enabled")]
    pub approval_needed: bool,
    /// The agent asked a question and is blocked on the answer.
    /// Default: `true`.
    #[serde(default = "default_notification_event_enabled")]
    pub input_needed: bool,
    /// The sandbox denied an operation and the user must decide.
    /// Default: `true`.
    #[serde(default = "default_notification_event_enabled")]
    pub elevation_needed: bool,
    /// The model called the `notify` tool. Default: `true`.
    #[serde(default = "default_notification_event_enabled")]
    pub model_notify: bool,
}

impl Default for NotificationEventsConfig {
    fn default() -> Self {
        Self {
            turn_complete: true,
            subagent_terminal: true,
            approval_needed: true,
            input_needed: true,
            elevation_needed: true,
            model_notify: true,
        }
    }
}

fn default_event_sound_events() -> Vec<String> {
    vec!["turn-complete".to_string(), "approval-needed".to_string()]
}

fn default_event_sound_min_interval_ms() -> u64 {
    2000
}

/// Opt-in, deterministic per-event sound policy (#4817). Terminal-bell
/// level only: cues are BEL (`\x07`) bytes, a platform-safe no-op on
/// terminals that ignore them. Off by default.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct EventSoundConfig {
    /// Master switch. Default: `false` (nothing is emitted unless opted in).
    #[serde(default)]
    pub enabled: bool,
    /// Allow-list of event names, kebab-case (`"turn-complete"`,
    /// `"subagent-terminal"`, `"approval-needed"`, `"input-needed"`,
    /// `"elevation-needed"`, `"model-notify"`). Unknown names are ignored.
    /// Default: `["turn-complete", "approval-needed"]`.
    #[serde(default = "default_event_sound_events")]
    pub events: Vec<String>,
    /// Minimum milliseconds between two plays of the same event. Default: 2000.
    #[serde(default = "default_event_sound_min_interval_ms")]
    pub min_interval_ms: u64,
    /// Quiet mode: suppress all event sounds without editing the allow-list.
    /// Default: `false`.
    #[serde(default)]
    pub quiet: bool,
}

impl Default for EventSoundConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            events: default_event_sound_events(),
            min_interval_ms: default_event_sound_min_interval_ms(),
            quiet: false,
        }
    }
}
