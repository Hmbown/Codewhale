/**
 * Operate starter — parallel scout with partial-failure synthesis.
 *
 * Dogfood source: docs/examples/dogfood-automatic/wf_a3_partial_failure_synthesis.workflow.js
 * Run: /workflow run workflows/operate_parallel_scout.workflow.js
 */
export default async function () {
  phase("Parallel scouts");
  const slots = await parallel([
    () =>
      task({
        description: "Healthy scout A",
        label: "scout-a",
        type: "explore",
        prompt: "Return the string READY_A. Read-only.",
      }),
    // Give this child an intentionally tiny budget so it starts, then fails
    // deterministically at the runtime boundary. A model refusal is still a
    // successful transport-level completion, while response-schema failures
    // intentionally abort the whole workflow so they remain loud.
    () =>
      task({
        description: "Deliberately failing scout B",
        label: "scout-b-fail",
        type: "explore",
        tokenBudget: 1,
        prompt:
          "Inspect Cargo.toml and return a detailed workspace summary. This child intentionally has a one-token budget so parallel() exercises a failed null slot.",
      }),
    () =>
      task({
        description: "Healthy scout C",
        label: "scout-c",
        type: "explore",
        prompt: "Return the string READY_C. Read-only.",
      }),
  ]);

  phase("Synthesize");
  const surviving = (slots || []).filter((s) => s != null);
  const summary = await task({
    description: "Synthesize from surviving parallel slots",
    label: "synthesizer",
    // Read-only synthesizer: "general" is write-capable and, without declared
    // write scope, would claim the workspace root (same as operate_read_audit).
    type: "review",
    prompt: [
      "Build one operator-facing summary from the surviving scout results.",
      "Explicitly note which parallel slot failed or returned null.",
      `slot_count=${(slots || []).length} surviving=${surviving.length}`,
      "slots_json:",
      JSON.stringify(slots),
    ].join("\n"),
  });

  return {
    scenario: "WF-A3",
    slots,
    surviving_count: surviving.length,
    summary,
  };
}
