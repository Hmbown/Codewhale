//! Session metrics: the shared accumulators behind the metrics line and
//! the detailed `turns · steps │ LLM · tools │ TTFT · avg tok/s │ cache │ in` ledger.
//!
//! Every number here is sourced from runtime evidence the engine already
//! emits — never from transcript timestamps or estimates:
//!
//! - **turns**: `Event::TurnStarted` count (`App::turn_counter`).
//! - **steps**: model calls (`Event::TurnUsage`) plus tool calls
//!   (`Event::ToolCallComplete`) — the agent's step count.
//! - **LLM**: sum of model-call wall time. Uses `TurnUsage::request_ms`
//!   (dispatch → usage receipt) when the engine measured dispatch, else the
//!   stream duration it always reports.
//! - **tools**: sum of tool wall time from `ToolCallStarted` → `ToolCallComplete`
//!   by tool id (the runtime's own clock, taken when the events drain).
//! - **TTFT avg**: mean of `TurnUsage::first_token_ms` over the model calls
//!   that reported one.
//! - **avg tok/s**: sum of provider-reported output tokens divided by the sum
//!   of measured request seconds (`request_ms`), across this loaded session's
//!   completed usage receipts. This is effective request throughput, including
//!   connection setup, time to first token, and pauses within the response;
//!   it is not a decoder-speed measurement or a live text-token estimate.
//!   Streaming and non-streaming calls use the same dispatch-to-receipt clock.
//!   Tool execution and idle time between calls are excluded. A transparent
//!   retry before any content uses the replacement request's clock; a billed
//!   response with usage is counted even if a later retry is needed. Calls
//!   without a positive measured request duration (including aggregate REPL
//!   child receipts) are excluded from both numerator and denominator. The
//!   normalized `Usage::output_tokens` receipt is canonical; separate reasoning
//!   counts are not added again and streamed estimates never enter this average.
//! - **cache**: provider-reported prompt-cache hit tokens over hit + miss
//!   (`SessionState::total_cache_hit_tokens` / `total_cache_miss_tokens`).
//! - **in**: provider-reported input tokens (`SessionState::total_input_tokens`).
//!
//! When a provider never reports a metric, or its evidence has not arrived
//! yet, the cell is omitted. Nothing here is estimated or captioned.
//!
//! The infoline and detailed ledger consume the same rate. The infoline keeps
//! the last measured session average while a request is in flight; it does not
//! divide a text estimate by a turn timer that also includes tools and waits.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use codewhale_localization::{Locale, MessageId, tr};

/// Runtime accumulators behind the strip. Lives on [`crate::tui::app::App`],
/// resets with the token breakdown when a session is loaded, so the numbers
/// describe this runtime session — the same scope as the token ledger.
#[derive(Debug, Clone, Default)]
pub struct SessionMetrics {
    /// Model calls that reported usage (`Event::TurnUsage`).
    pub model_calls: u64,
    /// Tool calls that completed (`Event::ToolCallComplete`).
    pub tool_calls: u64,
    /// Sum of model-call wall time.
    pub llm_time: Duration,
    /// Sum of tool wall time.
    pub tool_time: Duration,
    /// Sum of reported time-to-first-token values.
    ttft_total: Duration,
    /// How many model calls reported a time-to-first-token.
    ttft_samples: u64,
    /// Output tokens from calls that also reported a positive request duration.
    rate_output_tokens: u64,
    /// Dispatch-to-receipt time from exactly the same calls.
    rate_request_time: Duration,
    /// Tools currently running, keyed by tool id, with the instant their
    /// start event drained.
    tool_started: HashMap<String, Instant>,
}

impl SessionMetrics {
    /// Fold one model-call usage receipt into the accumulators.
    pub fn record_model_call(
        &mut self,
        output_tokens: u32,
        duration_ms: u64,
        first_token_ms: Option<u64>,
        request_ms: Option<u64>,
    ) {
        self.model_calls = self.model_calls.saturating_add(1);
        let call_ms = request_ms.unwrap_or(duration_ms);
        self.llm_time = self.llm_time.saturating_add(Duration::from_millis(call_ms));
        if let Some(ttft) = first_token_ms {
            self.ttft_total = self.ttft_total.saturating_add(Duration::from_millis(ttft));
            self.ttft_samples = self.ttft_samples.saturating_add(1);
        }
        if let Some(request_ms) = request_ms.filter(|millis| *millis > 0) {
            self.rate_output_tokens = self
                .rate_output_tokens
                .saturating_add(u64::from(output_tokens));
            self.rate_request_time = self
                .rate_request_time
                .saturating_add(Duration::from_millis(request_ms));
        }
    }

    /// Note that a tool started; the matching completion closes the timer.
    pub fn record_tool_started(&mut self, tool_id: &str) {
        self.record_tool_started_at(tool_id, Instant::now());
    }

    fn record_tool_started_at(&mut self, tool_id: &str, at: Instant) {
        self.tool_started.insert(tool_id.to_string(), at);
    }

    /// Note that a tool completed. Counts the call even when its start was
    /// never seen (a replayed or foreign completion), but only accrues time
    /// when the runtime saw both edges.
    pub fn record_tool_completed(&mut self, tool_id: &str) {
        self.record_tool_completed_at(tool_id, Instant::now());
    }

    fn record_tool_completed_at(&mut self, tool_id: &str, at: Instant) {
        self.tool_calls = self.tool_calls.saturating_add(1);
        if let Some(started) = self.tool_started.remove(tool_id) {
            self.tool_time = self
                .tool_time
                .saturating_add(at.saturating_duration_since(started));
        }
    }

    /// Drop in-flight tool timers (turn interrupted or failed): a tool that
    /// never completed must not leak into the next turn's accounting.
    pub fn clear_in_flight(&mut self) {
        self.tool_started.clear();
    }

    /// Model calls plus tool calls.
    #[must_use]
    pub fn steps(&self) -> u64 {
        self.model_calls.saturating_add(self.tool_calls)
    }

    /// Mean time-to-first-token, when at least one call reported it.
    #[must_use]
    pub fn ttft_average(&self) -> Option<Duration> {
        if self.ttft_samples == 0 {
            return None;
        }
        Some(self.ttft_total / u32::try_from(self.ttft_samples).unwrap_or(u32::MAX))
    }

    /// Session-average output tokens per measured request second. See the
    /// module documentation for included time and receipt coverage.
    #[must_use]
    pub fn tokens_per_second(&self) -> Option<f64> {
        let secs = self.rate_request_time.as_secs_f64();
        if self.rate_output_tokens == 0 || !secs.is_finite() || secs <= 0.0 {
            return None;
        }
        Some(self.rate_output_tokens as f64 / secs)
    }
}

/// Everything the strip needs, decoupled from `App` so rendering can be
/// unit-tested without a full app.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MetricsSnapshot {
    pub turns: u64,
    pub steps: u64,
    pub llm_time: Duration,
    pub tool_time: Duration,
    pub ttft_avg: Option<Duration>,
    pub tokens_per_second: Option<f64>,
    /// `None` when no provider reported prompt-cache classes this session.
    pub cache_hit_percent: Option<u8>,
    pub input_tokens: u64,
}

impl MetricsSnapshot {
    /// True when there is nothing to say yet (fresh session).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.turns == 0 && self.steps == 0 && self.input_tokens == 0
    }
}

/// One rendered cell: a value with its localized short label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricCell {
    pub label: String,
    pub value: String,
    /// `label` first (`4 turns`) or value first (`LLM 11m46s`).
    pub value_first: bool,
}
/// Group priority, highest kept first. When the row is too narrow, groups
/// are dropped from the end of this list; inside a group the second cell
/// (steps, tools, tok/s) is dropped before the group itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricGroup {
    Input,
    Cache,
    Llm,
    Turns,
    Latency,
}

/// The DSH-style layout order, left to right.
const GROUP_ORDER: [MetricGroup; 5] = [
    MetricGroup::Turns,
    MetricGroup::Llm,
    MetricGroup::Latency,
    MetricGroup::Cache,
    MetricGroup::Input,
];

/// A group of one or two cells separated by ` · `.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricGroupCells {
    pub group: MetricGroup,
    pub cells: Vec<MetricCell>,
}

/// Separators used between cells and between groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Separators {
    pub cell: &'static str,
    pub group: &'static str,
}

impl Separators {
    /// Unicode: ` · ` inside a group, ` │ ` between groups.
    pub const UNICODE: Self = Self {
        cell: " · ",
        group: " │ ",
    };
    /// ASCII-safe: ` . ` and ` | `.
    pub const ASCII: Self = Self {
        cell: " . ",
        group: " | ",
    };

    #[must_use]
    pub fn for_ascii(ascii_safe: bool) -> Self {
        if ascii_safe {
            Self::ASCII
        } else {
            Self::UNICODE
        }
    }
}

/// Format a duration the way the strip does: `11m46s`, `1h02m`, `1.5s`, `320ms`.
#[must_use]
pub fn format_duration(duration: Duration) -> String {
    let ms = duration.as_millis();
    if ms == 0 {
        return "0s".to_string();
    }
    if ms < 1_000 {
        return format!("{ms}ms");
    }
    let secs = duration.as_secs();
    if secs < 60 {
        let tenths = (ms + 50) / 100;
        return format!("{}.{}s", tenths / 10, tenths % 10);
    }
    if secs < 3_600 {
        return format!("{}m{:02}s", secs / 60, secs % 60);
    }
    format!("{}h{:02}m", secs / 3_600, (secs % 3_600) / 60)
}

/// Format a token count: `842`, `12.3K`, `9.3M`, `1.2B`.
#[must_use]
pub fn format_tokens(tokens: u64) -> String {
    const UNITS: [(u64, &str); 3] = [(1_000_000_000, "B"), (1_000_000, "M"), (1_000, "K")];
    for (scale, suffix) in UNITS {
        if tokens >= scale {
            let scaled = tokens as f64 / scale as f64;
            return if scaled >= 100.0 {
                format!("{scaled:.0}{suffix}")
            } else {
                format!("{scaled:.1}{suffix}")
            };
        }
    }
    tokens.to_string()
}

/// Format an output rate: `120` or `7.5` (the label carries `tok/s`).
#[must_use]
pub fn format_rate(rate: f64) -> String {
    if rate < 10.0 {
        format!("{rate:.1}")
    } else {
        format!("{rate:.0}")
    }
}

/// Build the cells for every group that has something truthful to show.
///
/// A cell whose evidence has not arrived is omitted — never a placeholder:
/// `TTFT avg` / `tok/s` appear only once a model call reported them, `Cache
/// hit` only when a provider reported cache classes, `Input` only after the
/// first usage receipt. Turn cells are present once the session has started
/// (zero turns is a real count). Step cells wait for the first completed
/// model or tool call so `0 steps` cannot look like a stalled scoreboard.
#[must_use]
pub fn build_groups(snapshot: MetricsSnapshot, locale: Locale) -> Vec<MetricGroupCells> {
    let label = |id: MessageId| tr(locale, id).into_owned();
    let mut groups = Vec::new();
    for group in GROUP_ORDER {
        let cells = match group {
            MetricGroup::Turns => {
                if snapshot.turns == 0 && snapshot.steps == 0 {
                    continue;
                }
                let mut cells = Vec::new();
                if snapshot.turns > 0 {
                    cells.push(MetricCell {
                        label: label(if snapshot.turns == 1 {
                            MessageId::SessionMetricsTurn
                        } else {
                            MessageId::SessionMetricsTurns
                        }),
                        value: snapshot.turns.to_string(),
                        value_first: true,
                    });
                }
                if snapshot.steps > 0 {
                    cells.push(MetricCell {
                        label: label(if snapshot.steps == 1 {
                            MessageId::SessionMetricsStep
                        } else {
                            MessageId::SessionMetricsSteps
                        }),
                        value: snapshot.steps.to_string(),
                        value_first: true,
                    });
                }
                if cells.is_empty() {
                    continue;
                }
                cells
            }
            MetricGroup::Llm => {
                let mut cells = Vec::new();
                if !snapshot.llm_time.is_zero() {
                    cells.push(MetricCell {
                        label: label(MessageId::SessionMetricsLlm),
                        value: format_duration(snapshot.llm_time),
                        value_first: false,
                    });
                }
                if !snapshot.tool_time.is_zero() {
                    cells.push(MetricCell {
                        label: label(MessageId::SessionMetricsTools),
                        value: format_duration(snapshot.tool_time),
                        value_first: false,
                    });
                }
                if cells.is_empty() {
                    continue;
                }
                cells
            }
            MetricGroup::Latency => {
                let mut cells = Vec::new();
                if let Some(ttft) = snapshot.ttft_avg {
                    cells.push(MetricCell {
                        label: label(MessageId::SessionMetricsTtft),
                        value: format_duration(ttft),
                        value_first: false,
                    });
                }
                if let Some(rate) = snapshot.tokens_per_second {
                    cells.push(MetricCell {
                        label: label(MessageId::SessionMetricsTokensPerSecond),
                        value: format_rate(rate),
                        value_first: true,
                    });
                }
                if cells.is_empty() {
                    continue;
                }
                cells
            }
            MetricGroup::Cache => {
                let Some(pct) = snapshot.cache_hit_percent else {
                    continue;
                };
                vec![MetricCell {
                    label: label(MessageId::SessionMetricsCache),
                    value: format!("{pct}%"),
                    value_first: false,
                }]
            }
            MetricGroup::Input => {
                if snapshot.input_tokens == 0 {
                    continue;
                }
                vec![MetricCell {
                    label: label(MessageId::SessionMetricsInput),
                    value: format_tokens(snapshot.input_tokens),
                    value_first: false,
                }]
            }
        };
        groups.push(MetricGroupCells { group, cells });
    }
    groups
}

/// A rendered strip: the plain text (for tests, `/status`, and width math)
/// plus the cells that survived the budget, so the painter can style labels
/// and values differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedStrip {
    pub groups: Vec<MetricGroupCells>,
    pub separators: Separators,
}

impl RenderedStrip {
    /// Plain-text form: `4 turns · 108 steps │ LLM 11m46s · tools 1m52s │ …`.
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (index, group) in self.groups.iter().enumerate() {
            if index > 0 {
                out.push_str(self.separators.group);
            }
            for (cell_index, cell) in group.cells.iter().enumerate() {
                if cell_index > 0 {
                    out.push_str(self.separators.cell);
                }
                if cell.value_first {
                    out.push_str(&cell.value);
                    out.push(' ');
                    out.push_str(&cell.label);
                } else {
                    out.push_str(&cell.label);
                    out.push(' ');
                    out.push_str(&cell.value);
                }
            }
        }
        out
    }
}

/// Prompt-cache hit rates, each labelled by whose requests it covers (#6565).
///
/// `parent` is this conversation's own requests: the footer `cache N%` and it
/// never change meaning. `agents` covers sub-agent and other background
/// requests. `combined` weights both by their tokens. Each is `None` when its
/// requests reported no cache telemetry; no report is never 0%.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheRates {
    pub parent: Option<u8>,
    pub agents: Option<u8>,
    pub combined: Option<u8>,
}

fn hit_percent(hit: u64, miss: u64) -> Option<u8> {
    let total = hit.saturating_add(miss);
    (total > 0).then(|| u8::try_from((hit.saturating_mul(100) + total / 2) / total).unwrap_or(100))
}

#[must_use]
pub fn cache_rates(app: &crate::tui::app::App) -> CacheRates {
    let parent_hit = u64::from(app.session.displayed_total_cache_hit_tokens());
    let parent_miss = u64::from(app.session.displayed_total_cache_miss_tokens());
    let agents = app
        .session
        .subagent_cache_hit_tokens
        .zip(app.session.subagent_cache_miss_tokens);
    CacheRates {
        parent: hit_percent(parent_hit, parent_miss),
        agents: agents.and_then(|(hit, miss)| hit_percent(hit, miss)),
        combined: agents.and_then(|(hit, miss)| {
            hit_percent(
                parent_hit.saturating_add(hit),
                parent_miss.saturating_add(miss),
            )
        }),
    }
}

impl CacheRates {
    /// `parent 82% · agents 64% · combined 75%` with the given words, or just
    /// `82%` when only the parent reported. `None` when nothing did.
    #[must_use]
    pub fn labelled(&self, parent: &str, agents: &str, combined: &str) -> Option<String> {
        match (self.parent, self.agents) {
            (Some(pct), None) => Some(format!("{pct}%")),
            (None, None) => None,
            _ => Some(
                [
                    (parent, self.parent),
                    (agents, self.agents),
                    (combined, self.combined),
                ]
                .into_iter()
                .filter_map(|(word, pct)| pct.map(|pct| format!("{word} {pct}%")))
                .collect::<Vec<_>>()
                .join(" · "),
            ),
        }
    }
}

/// Snapshot the live app state into the strip's inputs.
#[must_use]
pub fn snapshot_from_app(app: &crate::tui::app::App) -> MetricsSnapshot {
    // The footer rate is this conversation's own requests; sub-agent cache
    // is shown beside it, labelled, in PRICE and `/cache` (#6565).
    let cache_hit_percent = cache_rates(app).parent;
    MetricsSnapshot {
        turns: app.turn_counter,
        steps: app.session_metrics.steps(),
        llm_time: app.session_metrics.llm_time,
        tool_time: app.session_metrics.tool_time,
        ttft_avg: app.session_metrics.ttft_average(),
        tokens_per_second: app.session_metrics.tokens_per_second(),
        cache_hit_percent,
        input_tokens: u64::from(app.session.displayed_total_input_tokens()),
    }
}

/// The complete, untrimmed strip text — what `/status` prints.
#[must_use]
pub fn full_text(snapshot: MetricsSnapshot, locale: Locale, ascii_safe: bool) -> String {
    RenderedStrip {
        groups: build_groups(snapshot, locale),
        separators: Separators::for_ascii(ascii_safe),
    }
    .text()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> MetricsSnapshot {
        MetricsSnapshot {
            turns: 4,
            steps: 108,
            llm_time: Duration::from_secs(11 * 60 + 46),
            tool_time: Duration::from_secs(60 + 52),
            ttft_avg: Some(Duration::from_millis(1_500)),
            tokens_per_second: Some(120.0),
            cache_hit_percent: Some(99),
            input_tokens: 9_300_000,
        }
    }

    #[test]
    fn durations_format_like_the_harness_strip() {
        assert_eq!(format_duration(Duration::ZERO), "0s");
        assert_eq!(format_duration(Duration::from_millis(320)), "320ms");
        assert_eq!(format_duration(Duration::from_millis(1_500)), "1.5s");
        assert_eq!(format_duration(Duration::from_millis(1_549)), "1.5s");
        assert_eq!(format_duration(Duration::from_secs(59)), "59.0s");
        assert_eq!(format_duration(Duration::from_secs(11 * 60 + 46)), "11m46s");
        assert_eq!(format_duration(Duration::from_secs(3_600 + 120)), "1h02m");
    }

    #[test]
    fn tokens_and_rates_format_compactly() {
        assert_eq!(format_tokens(842), "842");
        assert_eq!(format_tokens(12_345), "12.3K");
        assert_eq!(format_tokens(128_000), "128K");
        assert_eq!(format_tokens(9_300_000), "9.3M");
        assert_eq!(format_tokens(1_200_000_000), "1.2B");
        assert_eq!(format_rate(120.4), "120");
        assert_eq!(format_rate(7.46), "7.5");
    }

    #[test]
    fn full_strip_matches_the_reference_layout() {
        let text = full_text(sample(), Locale::En, false);
        assert_eq!(
            text,
            "4 turns · 108 steps │ LLM 11m46s · Tool call 1m52s │ TTFT avg 1.5s · 120 avg tok/s │ Cache hit 99% │ Input 9.3M"
        );
        let ascii = full_text(sample(), Locale::En, true);
        assert!(ascii.is_ascii(), "{ascii}");
        assert!(ascii.contains(" | LLM 11m46s . Tool call 1m52s | "));
    }

    #[test]
    fn idle_snapshot_paints_nothing() {
        let text = full_text(MetricsSnapshot::default(), Locale::En, false);
        assert_eq!(text, "");
    }

    #[test]
    fn absent_evidence_omits_the_cell_instead_of_a_placeholder() {
        let mut snapshot = sample();
        snapshot.cache_hit_percent = None;
        snapshot.ttft_avg = None;
        snapshot.tokens_per_second = None;
        snapshot.input_tokens = 0;
        let text = full_text(snapshot, Locale::En, false);
        assert_eq!(text, "4 turns · 108 steps │ LLM 11m46s · Tool call 1m52s");
        assert!(!text.contains('—'), "{text}");

        // A partially reported latency group keeps only the reported cell.
        snapshot.ttft_avg = Some(Duration::from_millis(900));
        let text = full_text(snapshot, Locale::En, false);
        assert!(text.ends_with("│ TTFT avg 900ms"), "{text}");
        snapshot.ttft_avg = None;
        snapshot.tokens_per_second = Some(88.0);
        let text = full_text(snapshot, Locale::En, false);
        assert!(text.ends_with("│ 88 avg tok/s"), "{text}");
    }

    #[test]
    fn singular_labels_for_one_turn_and_one_step() {
        let snapshot = MetricsSnapshot {
            turns: 1,
            steps: 1,
            ..MetricsSnapshot::default()
        };
        let text = full_text(snapshot, Locale::En, false);
        assert_eq!(text, "1 turn · 1 step", "{text}");
    }

    #[test]
    fn every_shipped_locale_has_short_labels() {
        for locale in Locale::shipped_complete() {
            let text = full_text(sample(), *locale, false);
            assert!(text.contains("4 "), "{}: {text}", locale.tag());
            assert!(text.contains("11m46s"), "{}: {text}", locale.tag());
            for group in build_groups(sample(), *locale) {
                for cell in group.cells {
                    assert!(
                        cell.label.chars().count() <= 12,
                        "{}: label `{}` is too long for the strip",
                        locale.tag(),
                        cell.label
                    );
                }
            }
        }
    }

    #[test]
    fn accumulators_derive_ttft_and_rate_from_reported_calls() {
        let mut metrics = SessionMetrics::default();
        // 100 output tokens over a 2 s stream, first token after 500 ms, whole
        // call 2.4 s including connection setup.
        metrics.record_model_call(100, 2_000, Some(500), Some(2_400));
        // A call that reported no first token (empty response) still counts
        // for LLM time but not for TTFT.
        metrics.record_model_call(20, 1_000, None, Some(1_100));
        assert_eq!(metrics.model_calls, 2);
        assert_eq!(metrics.llm_time, Duration::from_millis(3_500));
        assert_eq!(metrics.ttft_average(), Some(Duration::from_millis(500)));
        let rate = metrics.tokens_per_second().expect("rate");
        assert!((rate - 120.0 / 3.5).abs() < 1e-9, "{rate}");

        // Missing request_ms falls back to the stream duration.
        metrics.record_model_call(0, 700, None, None);
        assert_eq!(metrics.llm_time, Duration::from_millis(4_200));
        // A duration without individual request timing cannot enter the rate.
        assert!((metrics.tokens_per_second().unwrap() - 120.0 / 3.5).abs() < 1e-9);
    }

    #[test]
    fn request_average_covers_ttft_stream_pauses_tools_and_non_streaming_calls() {
        let mut metrics = SessionMetrics::default();
        let t0 = Instant::now();
        let connected = t0 + Duration::from_millis(200);
        let first_token = t0 + Duration::from_secs(1);
        let pause_started = t0 + Duration::from_secs(2);
        let pause_finished = pause_started + Duration::from_secs(1);
        let first_receipt = pause_finished + Duration::from_secs(2);
        let millis = |duration: Duration| u64::try_from(duration.as_millis()).unwrap();
        metrics.record_model_call(
            120,
            millis(first_receipt.duration_since(connected)),
            Some(millis(first_token.duration_since(t0))),
            Some(millis(first_receipt.duration_since(t0))),
        );
        assert_eq!(metrics.tokens_per_second(), Some(24.0));

        // Thirty seconds of tool work and ten seconds idle are not model time.
        metrics.record_tool_started_at("build", first_receipt);
        let tool_finished = first_receipt + Duration::from_secs(30);
        metrics.record_tool_completed_at("build", tool_finished);
        let second_dispatch = tool_finished + Duration::from_secs(10);
        assert_eq!(metrics.tokens_per_second(), Some(24.0));
        let second_receipt = second_dispatch + Duration::from_secs(3);
        metrics.record_model_call(
            60,
            2_800,
            Some(500),
            Some(millis(second_receipt.duration_since(second_dispatch))),
        );

        // A buffered/non-streaming call has a real request clock even though
        // its adapter reports no stream duration or first-content timestamp.
        let third_dispatch = second_receipt + Duration::from_secs(20);
        let third_receipt = third_dispatch + Duration::from_secs(2);
        metrics.record_model_call(
            80,
            0,
            None,
            Some(millis(third_receipt.duration_since(third_dispatch))),
        );
        assert_eq!(metrics.tokens_per_second(), Some(26.0)); // 260 / (5 + 3 + 2)
        assert_eq!(metrics.ttft_average(), Some(Duration::from_millis(750)));
        assert_eq!(metrics.tool_time, Duration::from_secs(30));

        // Aggregate child or legacy receipts and zero-duration cache receipts
        // cannot contribute tokens without their matching request denominator.
        metrics.record_model_call(1_000, 9_000, None, None);
        metrics.record_model_call(300, 0, None, Some(0));
        assert_eq!(metrics.tokens_per_second(), Some(26.0));
        // A measured, empty response consumes time and produces zero output.
        metrics.record_model_call(0, 1_000, None, Some(1_000));
        assert!((metrics.tokens_per_second().unwrap() - 260.0 / 11.0).abs() < 1e-9);
    }

    #[test]
    fn tool_time_needs_both_edges_and_in_flight_timers_are_dropped() {
        let mut metrics = SessionMetrics::default();
        let t0 = Instant::now();
        metrics.record_tool_started_at("a", t0);
        metrics.record_tool_completed_at("a", t0 + Duration::from_millis(1_500));
        // Completion without a seen start counts the call, not the time.
        metrics.record_tool_completed_at("ghost", t0 + Duration::from_secs(9));
        assert_eq!(metrics.tool_calls, 2);
        assert_eq!(metrics.tool_time, Duration::from_millis(1_500));
        assert_eq!(metrics.steps(), 2);

        metrics.record_tool_started_at("b", t0);
        metrics.clear_in_flight();
        metrics.record_tool_completed_at("b", t0 + Duration::from_secs(5));
        assert_eq!(metrics.tool_time, Duration::from_millis(1_500));
    }

    #[test]
    fn sub_agent_cache_is_shown_beside_the_parent_rate_never_folded_into_it() {
        // #6565: the footer rate keeps meaning this conversation's requests;
        // agents and the token-weighted combination are labelled beside it.
        let mut app = crate::tui::app::App::new(
            crate::test_support::test_tui_options(std::path::PathBuf::from(".")),
            &crate::config::Config::default(),
        );
        assert_eq!(cache_rates(&app), CacheRates::default());
        assert_eq!(
            cache_rates(&app).labelled("parent", "agents", "combined"),
            None
        );

        app.session.total_cache_hit_tokens = 800;
        app.session.total_cache_miss_tokens = 200;
        assert_eq!(snapshot_from_app(&app).cache_hit_percent, Some(80));
        assert_eq!(
            cache_rates(&app)
                .labelled("parent", "agents", "combined")
                .as_deref(),
            Some("80%")
        );

        app.session.subagent_cache_hit_tokens = Some(200);
        app.session.subagent_cache_miss_tokens = Some(800);
        let rates = cache_rates(&app);
        assert_eq!(
            rates,
            CacheRates {
                parent: Some(80),
                agents: Some(20),
                combined: Some(50),
            }
        );
        assert_eq!(
            rates.labelled("parent", "agents", "combined").as_deref(),
            Some("parent 80% · agents 20% · combined 50%")
        );
        // The footer figure did not move.
        assert_eq!(snapshot_from_app(&app).cache_hit_percent, Some(80));

        // A loaded session starts the scope over, like the parent totals.
        app.session.reset_token_breakdown();
        assert_eq!(app.session.subagent_cache_hit_tokens, None);
    }
}
