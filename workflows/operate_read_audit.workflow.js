/**
 * Operate starter — read-only parallel audit (scouts → synthesizer).
 *
 * Dogfood source: docs/examples/dogfood-automatic/wf_a1_read_only_audit.workflow.js
 * Run: /workflow run workflows/operate_read_audit.workflow.js
 */
export default async function (args) {
  phase("Scout");
  const [crates, unsafeHits, unwrapHits] = await parallel([
    () =>
      task({
        description:
          "List top-level crates and one-line role for each under crates/.",
        label: "map crates",
        type: "explore",
        prompt:
          "Read Cargo.toml workspace members and crates/*/Cargo.toml. Return a short bullet list of crate names and purposes. Read-only.",
      }),
    () =>
      task({
        description: "Find unsafe blocks in Rust sources.",
        label: "scan unsafe",
        type: "explore",
        prompt:
          "Search for `unsafe` in crates/**/*.rs (exclude target). Summarize count and notable hot paths. Read-only; no edits.",
      }),
    () =>
      task({
        description: "Find unwrap/expect in hot paths.",
        label: "scan unwrap",
        type: "explore",
        prompt:
          "Search for `.unwrap(` and `.expect(` in crates/tui and crates/engine (if present). Note densest files. Read-only.",
      }),
  ]);

  phase("Synthesize");
  // Synthesizer must stay read-only: type "general" is write-capable, and
  // without declared writeRoots/exactFiles it would claim the workspace root.
  const summary = await task({
    description: "Synthesize audit findings for the operator.",
    label: "audit summary",
    type: "review",
    prompt: [
      "Synthesize a concise security/reliability audit from these scout results.",
      "Filter null/failed scouts. Group by severity. No file edits.",
      "",
      "crates:",
      String(crates ?? "(missing)"),
      "",
      "unsafe:",
      String(unsafeHits ?? "(missing)"),
      "",
      "unwrap:",
      String(unwrapHits ?? "(missing)"),
    ].join("\n"),
  });

  return {
    scenario: "WF-A1",
    goal: args?.goal ?? "read-only repo audit",
    summary,
  };
}
