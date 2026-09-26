//! The workbar: live workflow progress under the composer.
//!
//! The transcript carries one line when a workflow starts and one when it
//! finishes (`history.rs`); everything in between lives here, under the
//! composer and its posture bar, one row per run between two rules:
//!
//! ```text
//! ────────────────────────────────────────────────────────────────────────────
//!  • Compare Cline with Codewhale  ████████░░░░░░░░░░░░  4/10 so far  2m14s  ↓1.2M  ⚠ 1 failed · 2 queued
//! ────────────────────────────────────────────────────────────────────────────
//! ```
//!
//! Row grammar: status mark · name · 20-cell bar · settled/total · elapsed ·
//! ↓tokens · chips. The total is what the run has admitted *so far* — a script
//! can keep spawning agents — so a running row says "so far" and the bar may
//! move backwards when the total grows. Chips are only ever true: a failure
//! count when an agent failed, "Large workflow" only past
//! [`LARGE_WORKFLOW_AGENTS`], and `· N queued` only when the runtime reports
//! follow-ups waiting on a busy agent of this run.
//!
//! Every state reads without colour (a distinct mark, and a word for
//! anything that needs you). Nothing animates, so reduced motion needs no
//! second path; ASCII-safe terminals get the backend's per-cell fallbacks.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::tui::ui_text::truncate_line_to_width;
use crate::tui::widgets::workflow_panel::{WorkflowPanel, WorkflowPanelLifecycle};
use codewhale_localization::{Locale, MessageId, tr};
use codewhale_palette::UiTheme;

/// Cells in one progress bar.
pub(crate) const BAR_CELLS: usize = 20;
/// Agents a run must have admitted before the workbar calls it large. At this
/// size a run is no longer something to watch row by row, and its token use
/// is the thing worth a glance.
pub(crate) const LARGE_WORKFLOW_AGENTS: usize = 25;
/// Most run rows painted at once; the rest fold into one `+N more` row.
pub(crate) const MAX_RUN_ROWS: usize = 6;
/// The name column never shrinks below this before other columns go.
const MIN_NAME_COLS: usize = 12;
/// Below this width the bar gives its columns to the name.
const BAR_MIN_WIDTH: usize = 72;
const GAP: &str = "  ";

/// One run as the workbar paints it: the run's own state plus the runtime's
/// count of follow-ups waiting on its busy agents.
pub(crate) struct WorkbarRun<'a> {
    pub panel: &'a WorkflowPanel,
    pub queued: usize,
}

/// Rows the workbar wants for `runs` runs, before the frame's budget: one
/// per run (folded past [`MAX_RUN_ROWS`]) between a rule above and a rule
/// below. No runs, no rows.
#[must_use]
pub(crate) fn desired_rows(runs: usize) -> u16 {
    let rows = if runs > MAX_RUN_ROWS {
        MAX_RUN_ROWS + 1
    } else {
        runs
    };
    let rules = if rows > 0 { 2 } else { 0 };
    u16::try_from(rows + rules).unwrap_or(u16::MAX)
}

/// Paint the workbar into `area`: a rule, one row per run, a rule. A band
/// too short for the rules and every run row keeps the runs and drops the
/// rules: rules never hide a run.
pub(crate) fn render(
    area: ratatui::layout::Rect,
    buf: &mut ratatui::buffer::Buffer,
    runs: &[WorkbarRun<'_>],
    now_ms: u64,
    theme: &UiTheme,
    locale: Locale,
) {
    use ratatui::widgets::{Paragraph, Widget};
    if area.width == 0 || area.height == 0 || runs.is_empty() {
        return;
    }
    let run_rows = u16::try_from(runs.len().min(MAX_RUN_ROWS + 1)).unwrap_or(u16::MAX);
    let ruled = area.height >= run_rows.saturating_add(2);
    let row_area = if ruled {
        ratatui::layout::Rect {
            y: area.y + 1,
            height: area.height - 2,
            ..area
        }
    } else {
        area
    };
    if ruled {
        let rule = "─".repeat(usize::from(area.width));
        let style = Style::default().fg(theme.border);
        buf.set_string(area.x, area.y, &rule, style);
        buf.set_string(area.x, area.bottom() - 1, &rule, style);
    }
    let rows = lines(
        runs,
        row_area.width,
        usize::from(row_area.height),
        now_ms,
        theme,
        locale,
    );
    Paragraph::new(rows).render(row_area, buf);
}

/// Paint `runs` into at most `max_rows` lines of `width` columns. When the
/// runs do not all fit, the last row says how many are hidden and names the
/// key that lists them.
#[must_use]
pub(crate) fn lines(
    runs: &[WorkbarRun<'_>],
    width: u16,
    max_rows: usize,
    now_ms: u64,
    theme: &UiTheme,
    locale: Locale,
) -> Vec<Line<'static>> {
    let width = usize::from(width);
    if runs.is_empty() || max_rows == 0 || width < 8 {
        return Vec::new();
    }
    let shown = if runs.len() > max_rows {
        max_rows.saturating_sub(1)
    } else {
        runs.len()
    };
    let cells: Vec<RowCells> = runs[..shown]
        .iter()
        .map(|run| RowCells::new(run, now_ms, locale))
        .collect();
    let layout = Layout::fit(&cells, width.saturating_sub(2));
    let mut out: Vec<Line<'static>> = cells
        .iter()
        .map(|cells| cells.line(&layout, width, theme))
        .collect();
    let hidden = runs.len() - shown;
    if hidden > 0 {
        let arrow = if crate::tui::color_compat::ascii_safe_enabled() {
            "v"
        } else {
            "↓"
        };
        let text = format!(
            " {} · {arrow} {}",
            tr(locale, MessageId::WorkbarMoreRuns).replace("{count}", &hidden.to_string()),
            tr(locale, MessageId::FooterHintToManage),
        );
        out.push(Line::from(Span::styled(
            truncate_line_to_width(&text, width),
            Style::default().fg(theme.text_hint),
        )));
    }
    out
}

/// The text of one row, before layout.
struct RowCells {
    lifecycle: WorkflowPanelLifecycle,
    mark: &'static str,
    name: String,
    /// Filled cells of [`BAR_CELLS`].
    filled: usize,
    progress: String,
    elapsed: String,
    tokens: Option<String>,
    /// `(text, needs attention)`: attention chips paint in warning ink.
    chips: Vec<(String, bool)>,
}

impl RowCells {
    fn new(run: &WorkbarRun<'_>, now_ms: u64, locale: Locale) -> Self {
        let panel = run.panel;
        let (settled, total) = panel.done_total();
        let (failed, _) = panel.failure_cancel_counts();
        let lifecycle = panel.lifecycle;
        let filled = (settled * BAR_CELLS + total / 2)
            .checked_div(total)
            .unwrap_or(0)
            .min(BAR_CELLS);
        let progress = if total == 0 {
            tr(locale, MessageId::WorkflowNoTasksYet).into_owned()
        } else if lifecycle.is_running() {
            tr(locale, MessageId::WorkbarSoFar)
                .replace("{done}", &settled.to_string())
                .replace("{total}", &total.to_string())
        } else {
            format!("{settled}/{total}")
        };
        let end = panel.completed_at_ms.unwrap_or(now_ms);
        let elapsed = if panel.started_at_ms == 0 {
            String::new()
        } else {
            crate::elapsed::format_elapsed_secs(end.saturating_sub(panel.started_at_ms) / 1_000)
        };
        let tokens = panel.tokens_so_far().map(|tokens| {
            format!(
                "↓{}",
                crate::tui::footer_ui::format_token_count_compact(tokens)
            )
        });

        let mut chips = Vec::new();
        match lifecycle {
            WorkflowPanelLifecycle::Failed => {
                chips.push((tr(locale, MessageId::WorkflowLineFailed).into_owned(), true))
            }
            WorkflowPanelLifecycle::Cancelled => chips.push((
                tr(locale, MessageId::WorkflowLineStopped).into_owned(),
                false,
            )),
            WorkflowPanelLifecycle::Degraded => chips.push((
                tr(locale, MessageId::WorkflowLineFinishedWithGaps).into_owned(),
                true,
            )),
            _ if failed > 0 => chips.push((
                format!(
                    "⚠ {}",
                    tr(locale, MessageId::WorkflowCountFailed)
                        .replace("{count}", &failed.to_string())
                ),
                true,
            )),
            _ => {}
        }
        if total >= LARGE_WORKFLOW_AGENTS {
            chips.push((
                format!("⚠ {}", tr(locale, MessageId::WorkbarLargeWorkflow)),
                true,
            ));
        }
        if run.queued > 0 {
            chips.push((
                format!(
                    "· {}",
                    tr(locale, MessageId::AgentRailQueuedCount)
                        .replace("{count}", &run.queued.to_string())
                ),
                true,
            ));
        }
        let name = panel.label.split_whitespace().collect::<Vec<_>>().join(" ");
        Self {
            lifecycle,
            mark: lifecycle_mark(lifecycle),
            name: if name.is_empty() {
                "workflow".to_string()
            } else {
                name
            },
            filled,
            progress,
            elapsed,
            tokens,
            chips,
        }
    }

    fn line(&self, layout: &Layout, width: usize, theme: &UiTheme) -> Line<'static> {
        let state_ink = lifecycle_ink(self.lifecycle, theme);
        let quiet = Style::default().fg(theme.text_muted);
        let mut spans = vec![
            Span::raw(" "),
            Span::styled(format!("{} ", self.mark), Style::default().fg(state_ink)),
            Span::styled(
                pad(
                    &crate::tui::ui_text::semantic_truncate(&self.name, layout.name_cols),
                    layout.name_cols,
                ),
                Style::default()
                    .fg(theme.text_body)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        if layout.bar {
            spans.push(Span::raw(GAP));
            spans.push(Span::styled(
                "█".repeat(self.filled),
                Style::default().fg(state_ink),
            ));
            spans.push(Span::styled(
                "░".repeat(BAR_CELLS - self.filled),
                Style::default().fg(theme.text_hint),
            ));
        }
        spans.push(Span::raw(GAP));
        spans.push(Span::styled(
            pad(&self.progress, layout.progress_cols),
            quiet,
        ));
        if layout.elapsed_cols > 0 {
            spans.push(Span::raw(GAP));
            spans.push(Span::styled(pad(&self.elapsed, layout.elapsed_cols), quiet));
        }
        if layout.tokens_cols > 0 {
            spans.push(Span::raw(GAP));
            spans.push(Span::styled(
                pad(self.tokens.as_deref().unwrap_or(""), layout.tokens_cols),
                quiet,
            ));
        }
        for (index, (chip, attention)) in self.chips.iter().enumerate() {
            spans.push(Span::raw(if index == 0 { GAP } else { " " }));
            let ink = if *attention {
                theme.warning
            } else {
                theme.text_muted
            };
            spans.push(Span::styled(chip.clone(), Style::default().fg(ink)));
        }
        clip_line(spans, width)
    }
}

/// Column widths shared by every visible row, so the columns line up.
struct Layout {
    name_cols: usize,
    bar: bool,
    progress_cols: usize,
    elapsed_cols: usize,
    tokens_cols: usize,
}

impl Layout {
    /// Fit the shared columns into `width`. The name and the progress count
    /// always stay; the bar goes first, then tokens, then elapsed, so a narrow
    /// terminal keeps the facts and loses the picture.
    fn fit(rows: &[RowCells], width: usize) -> Self {
        let widest = |f: &dyn Fn(&RowCells) -> usize| rows.iter().map(f).max().unwrap_or(0);
        let name_want = widest(&|row| row.name.width());
        let chips_want = widest(&|row| {
            row.chips
                .iter()
                .map(|(chip, _)| chip.width() + 1)
                .sum::<usize>()
                .saturating_add(1)
        })
        .min(24);
        let mut layout = Self {
            name_cols: 0,
            bar: width >= BAR_MIN_WIDTH,
            progress_cols: widest(&|row| row.progress.width()),
            elapsed_cols: widest(&|row| row.elapsed.width()),
            tokens_cols: widest(&|row| row.tokens.as_deref().map_or(0, UnicodeWidthStr::width)),
        };
        loop {
            let fixed = 2 // mark + space
                + if layout.bar { BAR_CELLS + GAP.len() } else { 0 }
                + GAP.len() + layout.progress_cols
                + if layout.elapsed_cols > 0 { GAP.len() + layout.elapsed_cols } else { 0 }
                + if layout.tokens_cols > 0 { GAP.len() + layout.tokens_cols } else { 0 };
            let room = width.saturating_sub(fixed + chips_want);
            if room >= MIN_NAME_COLS.min(name_want) {
                layout.name_cols = name_want.min(room.max(MIN_NAME_COLS.min(name_want)));
                return layout;
            }
            if layout.bar {
                layout.bar = false;
            } else if layout.tokens_cols > 0 {
                layout.tokens_cols = 0;
            } else if layout.elapsed_cols > 0 {
                layout.elapsed_cols = 0;
            } else {
                layout.name_cols = width.saturating_sub(fixed).max(1).min(name_want.max(1));
                return layout;
            }
        }
    }
}

/// The run's mark: distinct shapes, so the state reads without colour.
fn lifecycle_mark(lifecycle: WorkflowPanelLifecycle) -> &'static str {
    match lifecycle {
        WorkflowPanelLifecycle::Pending => crate::tui::glyphs::AVAILABLE,
        WorkflowPanelLifecycle::Running => "•",
        WorkflowPanelLifecycle::Succeeded => crate::tui::glyphs::DONE,
        WorkflowPanelLifecycle::Degraded => crate::tui::glyphs::ATTENTION,
        WorkflowPanelLifecycle::Failed => crate::tui::glyphs::FAILED,
        WorkflowPanelLifecycle::Cancelled => "⊘",
    }
}

/// Running is working ink, not attention; only trouble spends warning/error.
fn lifecycle_ink(lifecycle: WorkflowPanelLifecycle, theme: &UiTheme) -> ratatui::style::Color {
    match lifecycle {
        WorkflowPanelLifecycle::Pending | WorkflowPanelLifecycle::Cancelled => theme.text_muted,
        WorkflowPanelLifecycle::Running => theme.accent_action,
        WorkflowPanelLifecycle::Succeeded => theme.success,
        WorkflowPanelLifecycle::Degraded => theme.warning,
        WorkflowPanelLifecycle::Failed => theme.error_fg,
    }
}

fn pad(text: &str, cols: usize) -> String {
    let width = text.width();
    if width >= cols {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(cols - width))
    }
}

/// Clip a styled row at `width` columns without splitting a wide glyph.
fn clip_line(spans: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let mut used = 0usize;
    let mut out = Vec::with_capacity(spans.len());
    for span in spans {
        let span_width = span.content.width();
        if used + span_width <= width {
            used += span_width;
            out.push(span);
            continue;
        }
        let room = width.saturating_sub(used);
        if room > 0 {
            let text = truncate_line_to_width(&span.content, room);
            out.push(Span::styled(text, span.style));
        }
        break;
    }
    Line::from(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::widgets::workflow_panel::{WorkflowPanelEvent, WorkflowRowStatus};
    use ratatui::{Terminal, backend::TestBackend, widgets::Paragraph};

    const NOW: u64 = 1_000_000;

    fn run(label: &str, started: u64, agents: usize, settled: usize) -> WorkflowPanel {
        let mut panel = WorkflowPanel::new(format!("run-{label}"), label, started);
        panel.apply_event(WorkflowPanelEvent::PhaseStarted {
            title: "Survey".to_string(),
            at_ms: started,
        });
        for index in 0..agents {
            let value = serde_json::json!({
                "type": "task_started",
                "task_id": format!("{label}-{index}"),
                "workflow_task_label": format!("agent-{index}"),
                "at_ms": started,
            });
            panel.apply_event(WorkflowPanelEvent::from_json_value(&value).expect("task_started"));
        }
        for index in 0..settled {
            panel.apply_event(WorkflowPanelEvent::TaskCompleted {
                task_id: format!("{label}-{index}"),
                status: WorkflowRowStatus::Succeeded,
                usage: None,
                at_ms: started + 1_000,
            });
        }
        panel
    }

    fn render(runs: &[WorkbarRun<'_>], width: u16, rows: u16) -> String {
        let theme = codewhale_palette::UI_THEME;
        let lines = lines(runs, width, usize::from(rows), NOW, &theme, Locale::En);
        let mut terminal = Terminal::new(TestBackend::new(width, rows)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget(Paragraph::new(lines), frame.area()))
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        (0..rows)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn running_row_shows_bar_growing_total_elapsed_and_tokens() {
        let mut panel = run("Compare Cline with Codewhale", NOW - 134_000, 10, 4);
        panel.budget_spent = 1_234_567;
        let snapshot = render(
            &[WorkbarRun {
                panel: &panel,
                queued: 0,
            }],
            100,
            1,
        );
        assert_eq!(
            snapshot,
            " • Compare Cline with Codewhale  ████████░░░░░░░░░░░░  4/10 so far  2m 14s  ↓1.2M"
        );
    }

    #[test]
    fn settled_rows_read_without_colour_and_drop_so_far() {
        let mut done = run("audit", NOW - 60_000, 3, 3);
        done.apply_event(WorkflowPanelEvent::RunCompleted {
            status: WorkflowPanelLifecycle::Succeeded,
            error: None,
            at_ms: NOW - 1_000,
        });
        let mut failed = run("migrate", NOW - 60_000, 2, 0);
        failed.apply_event(WorkflowPanelEvent::RunCompleted {
            status: WorkflowPanelLifecycle::Failed,
            error: Some("script error".to_string()),
            at_ms: NOW - 1_000,
        });
        let mut gaps = run("review", NOW - 60_000, 2, 1);
        gaps.apply_event(WorkflowPanelEvent::RunCompleted {
            status: WorkflowPanelLifecycle::Degraded,
            error: None,
            at_ms: NOW - 1_000,
        });
        let snapshot = render(
            &[
                WorkbarRun {
                    panel: &done,
                    queued: 0,
                },
                WorkbarRun {
                    panel: &failed,
                    queued: 0,
                },
                WorkbarRun {
                    panel: &gaps,
                    queued: 0,
                },
            ],
            80,
            3,
        );
        assert_eq!(
            snapshot,
            [
                " ✓ audit    ████████████████████  3/3  59s",
                " ✕ migrate  ░░░░░░░░░░░░░░░░░░░░  0/2  59s  failed",
                " ◆ review   ██████████░░░░░░░░░░  1/2  59s  finished with gaps",
            ]
            .join("\n")
        );
    }

    #[test]
    fn chips_are_only_ever_true() {
        // Small and healthy: no chip at all.
        let small = run("small", NOW - 5_000, 3, 1);
        let plain = render(
            &[WorkbarRun {
                panel: &small,
                queued: 0,
            }],
            100,
            1,
        );
        assert!(!plain.contains('⚠') && !plain.contains("queued"), "{plain}");

        // Large only at the threshold, a failure count only after a failure,
        // queued only when the runtime reports waiting follow-ups.
        let mut large = run("large", NOW - 5_000, LARGE_WORKFLOW_AGENTS, 0);
        large.apply_event(WorkflowPanelEvent::TaskCompleted {
            task_id: "large-0".to_string(),
            status: WorkflowRowStatus::Failed,
            usage: None,
            at_ms: NOW,
        });
        let busy = render(
            &[WorkbarRun {
                panel: &large,
                queued: 2,
            }],
            120,
            1,
        );
        assert!(busy.contains("⚠ 1 failed"), "{busy}");
        assert!(busy.contains("⚠ Large workflow"), "{busy}");
        assert!(busy.ends_with("· 2 queued"), "{busy}");
        let under = run("under", NOW - 5_000, LARGE_WORKFLOW_AGENTS - 1, 0);
        let under = render(
            &[WorkbarRun {
                panel: &under,
                queued: 0,
            }],
            120,
            1,
        );
        assert!(!under.contains("Large"), "{under}");
    }

    #[test]
    fn narrow_rows_shed_the_bar_before_the_facts() {
        let panel = run(
            "Compare Cline with Codewhale on five axes",
            NOW - 10_000,
            8,
            2,
        );
        let narrow = render(
            &[WorkbarRun {
                panel: &panel,
                queued: 0,
            }],
            60,
            1,
        );
        assert!(!narrow.contains('█') && !narrow.contains('░'), "{narrow}");
        assert!(narrow.contains("2/8 so far"), "{narrow}");
        assert!(narrow.width() <= 60, "{narrow}");
        let tiny = render(
            &[WorkbarRun {
                panel: &panel,
                queued: 0,
            }],
            30,
            1,
        );
        assert!(tiny.contains("2/8 so far"), "{tiny}");
    }

    #[test]
    fn many_runs_fold_into_a_more_row_that_names_the_key() {
        let panels: Vec<WorkflowPanel> = (0..12)
            .map(|index| run(&format!("wf-{index:02}"), NOW - 10_000, 8, index % 8))
            .collect();
        let runs: Vec<WorkbarRun<'_>> = panels
            .iter()
            .map(|panel| WorkbarRun { panel, queued: 0 })
            .collect();
        // Six run rows, the fold row, and the two rules.
        assert_eq!(desired_rows(runs.len()), MAX_RUN_ROWS as u16 + 3);
        let snapshot = render(&runs, 100, MAX_RUN_ROWS as u16 + 1);
        let rows: Vec<&str> = snapshot.lines().collect();
        assert_eq!(rows.len(), MAX_RUN_ROWS + 1);
        assert_eq!(rows[MAX_RUN_ROWS], " +6 more · ↓ to manage");
    }

    fn settled(
        label: &str,
        status: WorkflowPanelLifecycle,
        agents: usize,
        done: usize,
    ) -> WorkflowPanel {
        let mut panel = run(label, NOW - 60_000, agents, done);
        panel.apply_event(WorkflowPanelEvent::RunCompleted {
            status,
            error: None,
            at_ms: NOW - 1_000,
        });
        panel
    }

    fn render_band(runs: &[WorkbarRun<'_>], width: u16, height: u16) -> ratatui::buffer::Buffer {
        let area = ratatui::layout::Rect::new(0, 0, width, height);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| {
                super::render(
                    area,
                    frame.buffer_mut(),
                    runs,
                    NOW,
                    &codewhale_palette::UI_THEME,
                    Locale::En,
                );
            })
            .expect("draw");
        terminal.backend().buffer().clone()
    }

    fn buffer_rows(buf: &ratatui::buffer::Buffer) -> Vec<String> {
        let area = buf.area;
        (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn the_band_is_one_row_per_run_between_two_rules() {
        let live = run("Audit the parser", NOW - 30_000, 4, 1);
        let done = settled("Port fixtures", WorkflowPanelLifecycle::Succeeded, 2, 2);
        let runs = [
            WorkbarRun {
                panel: &live,
                queued: 3,
            },
            WorkbarRun {
                panel: &done,
                queued: 0,
            },
        ];
        assert_eq!(desired_rows(runs.len()), 4);
        assert_eq!(desired_rows(0), 0);
        let rows = buffer_rows(&render_band(&runs, 90, desired_rows(runs.len())));
        let rule = "─".repeat(90);
        assert_eq!(
            rows,
            vec![
                rule.clone(),
                " • Audit the parser  █████░░░░░░░░░░░░░░░  1/4 so far  30s  · 3 queued"
                    .to_string(),
                " ✓ Port fixtures     ████████████████████  2/2         59s".to_string(),
                rule,
            ]
        );

        // Too short for rules and a run: the run keeps the row.
        let rows = buffer_rows(&render_band(&runs[..1], 90, 2));
        assert!(rows[0].contains("Audit the parser"), "{rows:?}");
        assert!(!rows.iter().any(|row| row.starts_with('─')), "{rows:?}");

        // Room for the rules but not every run: the runs win, never a band of
        // rules around a "+2 more" with no run visible.
        let rows = buffer_rows(&render_band(&runs, 90, 3));
        assert!(rows[0].contains("Audit the parser"), "{rows:?}");
        assert!(rows[1].contains("Port fixtures"), "{rows:?}");
        assert!(!rows.iter().any(|row| row.starts_with('─')), "{rows:?}");
    }

    /// NO_COLOR / 16-colour / ASCII terminals: every state still reads. Under
    /// monochrome the text is unchanged and each state keeps its own mark;
    /// at 16 colours the state inks stay apart; ASCII-safe marks stay apart
    /// except failed/stopped, which their words tell apart.
    #[test]
    fn states_read_on_no_color_sixteen_colour_and_ascii_terminals() {
        use crate::tui::color_compat::{adapt_cell_colors, adapt_cell_symbol_for_ascii};
        use codewhale_palette::{ColorDepth, PaletteMode, ThemeId};
        let theme = codewhale_palette::UI_THEME;
        let running = run("running", NOW - 10_000, 4, 1);
        let ok = settled("ok", WorkflowPanelLifecycle::Succeeded, 2, 2);
        let gaps = settled("gaps", WorkflowPanelLifecycle::Degraded, 2, 1);
        let failed = settled("failed", WorkflowPanelLifecycle::Failed, 2, 0);
        let stopped = settled("stopped", WorkflowPanelLifecycle::Cancelled, 2, 0);
        let panels = [&running, &ok, &gaps, &failed, &stopped];
        let runs: Vec<WorkbarRun<'_>> = panels
            .iter()
            .map(|panel| WorkbarRun { panel, queued: 0 })
            .collect();
        let source = render_band(&runs, 100, desired_rows(runs.len()));
        let text = buffer_rows(&source);
        let marks: Vec<String> = text[1..=5]
            .iter()
            .map(|row| row.chars().nth(1).expect("mark").to_string())
            .collect();
        for (index, mark) in marks.iter().enumerate() {
            assert!(
                !marks[index + 1..].contains(mark),
                "state marks collide: {marks:?}"
            );
        }
        assert!(text[3].contains("finished with gaps"), "{text:?}");
        assert!(text[4].contains("failed"), "{text:?}");
        assert!(text[5].contains("stopped"), "{text:?}");

        let adapted = |depth: ColorDepth| {
            let mut buf = source.clone();
            for cell in buf.content.iter_mut() {
                adapt_cell_colors(cell, depth, PaletteMode::Dark, ThemeId::Whale, &theme, None);
            }
            buf
        };
        let mono = adapted(ColorDepth::Monochrome);
        assert_eq!(buffer_rows(&mono), text, "monochrome changes no text");
        assert!(
            mono.content.iter().all(|cell| {
                cell.fg == ratatui::style::Color::Reset && cell.bg == ratatui::style::Color::Reset
            }),
            "monochrome paints no colour"
        );
        let ansi16 = adapted(ColorDepth::Ansi16);
        let mark_ink: Vec<ratatui::style::Color> = (1..=5).map(|y| ansi16[(1, y)].fg).collect();
        for (index, ink) in mark_ink.iter().enumerate() {
            assert!(
                !mark_ink[index + 1..].contains(ink),
                "16-colour state inks collide: {mark_ink:?}"
            );
        }

        let mut ascii = source.clone();
        for cell in ascii.content.iter_mut() {
            adapt_cell_symbol_for_ascii(cell);
        }
        let ascii_rows = buffer_rows(&ascii);
        assert!(
            ascii_rows.iter().all(|row| row.is_ascii()),
            "{ascii_rows:?}"
        );
        let ascii_marks: Vec<char> = ascii_rows[1..=4]
            .iter()
            .map(|row| row.chars().nth(1).expect("mark"))
            .collect();
        for (index, mark) in ascii_marks.iter().enumerate() {
            assert!(
                !ascii_marks[index + 1..].contains(mark),
                "ASCII marks collide: {ascii_marks:?}"
            );
        }
    }

    #[test]
    fn seventy_five_agents_across_ten_runs_render_in_one_pass() {
        let panels: Vec<WorkflowPanel> = (0..10)
            .map(|index| run(&format!("wf-{index}"), NOW - 10_000, 8, 3))
            .collect();
        let runs: Vec<WorkbarRun<'_>> = panels
            .iter()
            .map(|panel| WorkbarRun { panel, queued: 0 })
            .collect();
        let started = std::time::Instant::now();
        for _ in 0..100 {
            let _ = lines(
                &runs,
                120,
                MAX_RUN_ROWS + 1,
                NOW,
                &codewhale_palette::UI_THEME,
                Locale::En,
            );
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "100 frames of 10 runs × 8 agents took {:?}",
            started.elapsed()
        );
    }
}
