//! Pure session metrics rendering; host observation and painting remain outside.
use crate::config_policy::StatusMetrics as MetricsSnapshot;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct MetricLabels {
    pub cache: String,
    pub input: String,
    pub llm: String,
    pub step: String,
    pub steps: String,
    pub tokens_per_second: String,
    pub tools: String,
    pub ttft: String,
    pub turn: String,
    pub turns: String,
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
pub fn build_groups(snapshot: MetricsSnapshot, labels: &MetricLabels) -> Vec<MetricGroupCells> {
    let label = |value: &String| value.clone();
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
                            &labels.turn
                        } else {
                            &labels.turns
                        }),
                        value: snapshot.turns.to_string(),
                        value_first: true,
                    });
                }
                if snapshot.steps > 0 {
                    cells.push(MetricCell {
                        label: label(if snapshot.steps == 1 {
                            &labels.step
                        } else {
                            &labels.steps
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
                        label: label(&labels.llm),
                        value: format_duration(snapshot.llm_time),
                        value_first: false,
                    });
                }
                if !snapshot.tool_time.is_zero() {
                    cells.push(MetricCell {
                        label: label(&labels.tools),
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
                        label: label(&labels.ttft),
                        value: format_duration(ttft),
                        value_first: false,
                    });
                }
                if let Some(rate) = snapshot.tokens_per_second {
                    cells.push(MetricCell {
                        label: label(&labels.tokens_per_second),
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
                    label: label(&labels.cache),
                    value: format!("{pct}%"),
                    value_first: false,
                }]
            }
            MetricGroup::Input => {
                if snapshot.input_tokens == 0 {
                    continue;
                }
                vec![MetricCell {
                    label: label(&labels.input),
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
