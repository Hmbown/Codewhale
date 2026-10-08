// Unit coverage for trajectory redaction: every text-entry argument, including
// run_actions steps and the merged clipboard tool, is replaced before writing.
import { test } from "node:test";
import assert from "node:assert/strict";
import { redactCall, REDACTED } from "../src/trajectory.mjs";

test("redactCall removes entered text from every text-entry tool", () => {
  for (const [tool, args, field] of [
    ["type", { text: "pw", press_enter: true }, "text"],
    ["set_value", { value: "pw", target: { type: "element", index: 1 } }, "value"],
    ["browser_type", { text: "pw", selector: "#p" }, "text"],
    ["clipboard", { action: "write", text: "pw" }, "text"],
    ["write_clipboard", { text: "pw" }, "text"],
  ]) {
    const r = redactCall(tool, args);
    assert.equal(r.redacted, true, tool);
    assert.equal(r.args[field], REDACTED, tool);
    assert.equal(args[field], "pw", "the live call's arguments are not mutated");
  }
  const steps = redactCall("run_actions", { steps: [{ tool: "left_click", arguments: { target: { type: "element", index: 2 } } }, { tool: "type", arguments: { text: "pw" } }] });
  assert.equal(steps.redacted, true);
  assert.equal(steps.args.steps[1].arguments.text, REDACTED);
  assert.deepEqual(steps.args.steps[0].arguments, { target: { type: "element", index: 2 } });
  const plain = redactCall("left_click", { target: { type: "coordinate", x: 1, y: 2 } });
  assert.equal(plain.redacted, false);
  assert.equal(redactCall("key", { text: "return" }).redacted, false, "key names are not entered text");
});
