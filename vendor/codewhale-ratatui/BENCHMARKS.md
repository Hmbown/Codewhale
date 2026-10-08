# Rendering benchmarks

```sh
cargo bench --locked --bench render
cargo bench --locked --bench render -- --quick
```

The dependency-free harness measures actual Ratatui cell buffers in release
mode with the native Underwater palette. It warms each scenario, runs 31
samples of 20 paints, and reports median and p95 **batch-average** microseconds
per operation. `--quick` uses five samples of two paints for a smoke run.

| Scenario | Timed work |
| --- | --- |
| Native composer | Reset and paint a prepared 112×5 composer with CJK, combining text and a ZWJ emoji |
| Native workbar | Reset and paint a prepared 112×5 dock with ten task rows |
| Session lists | Reset and paint a prepared 112×38 list with ten, 100 or 10,000 source rows; the latter two have the same visible row count |
| Ocean background | Clone a populated 112×38 native shell with messages, composer and workbar, then apply its first depth-column pass |
| Native work view | Reset, construct and draw the current showcase Work fixture |

Prepared fixtures, their source data and buffers are allocated before timing.
Buffer resets are included; the ocean measurement explicitly includes cloning
its populated input. Outputs are passed to `black_box`. No terminal writes,
event polling, provider work, image export or startup detection is timed.

These are local rendering measurements, not end-to-end latency or a comparison
with competing libraries. Batch averages smooth scheduler noise; their p95 is
not an individual-frame tail-latency claim. Record the machine, compiler,
commit and background workload when comparing runs. Use the same fixtures and
settings before and after an optimization, then confirm identical cell buffers.
CI compiles this harness and uses deterministic behavior/geometry checks;
wall-clock timing thresholds would be unreliable on shared runners.

## Sample quality-pass measurements

Measured on October 1, 2026, Apple M4 Max (arm64), Rust 1.99.0, release mode.
Other workspace builds were active, so treat these as local samples rather
than portable budgets. The same harness and fixtures ran before and after
reusing the contrast verdict for consecutive same-color ocean text cells.

| Scenario | Before median / p95 µs | After median / p95 µs |
| --- | ---: | ---: |
| Composer, 112×5 | 10.927 / 15.873 | 10.512 / 10.906 |
| Workbar, 112×5 | 18.375 / 20.142 | 18.554 / 19.340 |
| Sessions, 100 rows | 77.123 / 82.310 | 75.079 / 79.292 |
| Sessions, 10,000 rows | 75.502 / 80.723 | 74.871 / 85.604 |
| Ocean plus fixture clone | 90.796 / 95.590 | 30.181 / 31.135 |
| Native Work, construction and paint | 145.077 / 156.183 | 119.348 / 128.233 |

The ocean sample's median fell about 67%; the full Work sample fell about
18%. Geometry, profile and per-cell contrast checks preserve the rendered
result. The other rows provide context for run-to-run noise; their small
differences are not claimed as optimizations. The 100- and 10,000-row lists
paint the same number of visible rows, illustrating bounded viewport work.
