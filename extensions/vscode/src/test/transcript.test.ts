import assert from "node:assert/strict";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { afterEach, beforeEach, describe, it } from "node:test";
import {
  extractFilePath,
  isInsideRoot,
  isRealPathInsideRoot,
  projectItem,
  statusForEvent,
} from "../transcript";
import type { ItemRecord } from "../api";

const agent = (over: Partial<ItemRecord> = {}): ItemRecord => ({
  id: "i1",
  kind: "agent_message",
  summary: "",
  ...over,
});

describe("projectItem", () => {
  it("renders the full detail body, not the truncated summary", () => {
    const stub = "Here is the plan" + "…".repeat(10);
    const view = projectItem(
      agent({ status: "completed", summary: stub, detail: "Here is the plan, in full, with steps." }),
      undefined,
    );
    assert.equal(view.summary, "Here is the plan, in full, with steps.");
    assert.ok(view.html?.includes("in full"));
    assert.ok(!view.html?.includes("…"));
  });

  it("falls back to summary when detail is absent on the wire", () => {
    const view = projectItem(agent({ status: "completed", summary: "short reply" }), undefined);
    assert.equal(view.summary, "short reply");
    assert.ok(view.html?.includes("short reply"));
  });

  it("keeps streamed text when a completion payload carries neither field", () => {
    const streaming = projectItem(agent({ status: "in_progress" }), {
      id: "i1",
      kind: "agent_message",
      summary: "partial",
      streamText: "partial",
      rev: 1,
    });
    assert.equal(streaming.streamText, "partial");
    const done = projectItem(agent({ status: "completed" }), streaming);
    assert.equal(done.summary, "partial");
    assert.equal(done.rev, streaming.rev + 1);
  });

  it("carries a parsed file path onto non-agent items", () => {
    const view = projectItem(
      {
        id: "t1",
        kind: "file_change",
        summary: "Edited a file",
        metadata: { tool_input: JSON.stringify({ file_path: "src/chat.ts" }) },
      },
      undefined,
    );
    assert.equal(view.filePath, "src/chat.ts");
  });

  it("merges detail and metadata forward from the existing view", () => {
    const first = projectItem(
      { id: "t1", kind: "tool_call", summary: "run", detail: "line one", metadata: { path: "a.ts" } },
      undefined,
    );
    const second = projectItem({ id: "t1", kind: "tool_call", summary: "run" }, first, "item.completed");
    assert.equal(second.detail, "line one");
    assert.equal(second.filePath, "a.ts");
  });
});

describe("extractFilePath", () => {
  it("reads the path out of a JSON tool_input string", () => {
    assert.equal(
      extractFilePath({ tool_input: '{"file_path":"/repo/src/main.rs","content":"x"}' }),
      "/repo/src/main.rs",
    );
  });

  it("reads an already-parsed tool_input object", () => {
    assert.equal(extractFilePath({ tool_input: { path: "docs/README.md" } }), "docs/README.md");
  });

  it("prefers a direct metadata path when present", () => {
    assert.equal(extractFilePath({ path: "direct.ts", tool_input: '{"file_path":"other.ts"}' }), "direct.ts");
  });

  it("survives malformed or missing tool_input", () => {
    assert.equal(extractFilePath(undefined), undefined);
    assert.equal(extractFilePath({}), undefined);
    assert.equal(extractFilePath({ tool_input: "not json {" }), undefined);
    assert.equal(extractFilePath({ tool_input: "[]" }), undefined);
    assert.equal(extractFilePath({ tool_input: '{"command":"ls"}' }), undefined);
    assert.equal(extractFilePath({ tool_input: '{"file_path":42}' }), undefined);
  });

  it("refuses paths carrying control characters", () => {
    assert.equal(extractFilePath({ path: "src/\u0000etc/passwd" }), undefined);
    assert.equal(extractFilePath({ tool_input: '{"file_path":"a\\u001bb.ts"}' }), undefined);
  });
});

describe("isInsideRoot", () => {
  const root = path.resolve("/repo/project");

  it("accepts a workspace-relative path", () => {
    assert.equal(isInsideRoot(root, "src/chat.ts"), true);
    assert.equal(isInsideRoot(root, path.join(root, "src", "chat.ts")), true);
  });

  it("refuses traversal and outside-workspace paths", () => {
    assert.equal(isInsideRoot(root, "../secrets.env"), false);
    assert.equal(isInsideRoot(root, "src/../../secrets.env"), false);
    assert.equal(isInsideRoot(root, path.resolve("/etc/passwd")), false);
    assert.equal(isInsideRoot(root, root), false);
    assert.equal(isInsideRoot("", "src/chat.ts"), false);
  });
});

describe("isInsideRoot dot-prefixed names", () => {
  it("accepts a file whose name merely starts with two dots", () => {
    const root = path.resolve("/repo/project");
    assert.equal(isInsideRoot(root, "..notes.md"), true);
    assert.equal(isInsideRoot(root, "..hidden/file.ts"), true);
    assert.equal(isInsideRoot(root, "../project-sibling/file.ts"), false);
  });
});

describe("isRealPathInsideRoot", () => {
  let tmp: string;
  let root: string;
  let outside: string;

  beforeEach(() => {
    // realpath: on macOS the temp dir itself sits behind /var -> /private/var.
    tmp = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "cw-vscode-root-")));
    root = path.join(tmp, "workspace");
    outside = path.join(tmp, "outside");
    fs.mkdirSync(path.join(root, "src"), { recursive: true });
    fs.mkdirSync(outside);
    fs.writeFileSync(path.join(root, "src", "inside.ts"), "x");
    fs.writeFileSync(path.join(outside, "secret.env"), "x");
  });

  afterEach(() => {
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  it("accepts real files, not-yet-existing files and a root reached through a link", async () => {
    assert.equal(await isRealPathInsideRoot(root, "src/inside.ts"), true);
    assert.equal(await isRealPathInsideRoot(root, "src/not-created-yet.ts"), true);
    assert.equal(await isRealPathInsideRoot(root, path.join(root, "src", "inside.ts")), true);
    const viaLink = path.join(tmp, "workspace-link");
    fs.symlinkSync(root, viaLink);
    assert.equal(await isRealPathInsideRoot(viaLink, "src/inside.ts"), true);
  });

  it("refuses a link inside the workspace that points outside it", async () => {
    fs.symlinkSync(path.join(outside, "secret.env"), path.join(root, "src", "linked.env"));
    fs.symlinkSync(outside, path.join(root, "linked-dir"));
    // Lexically inside, really outside: the lexical check alone is fooled.
    assert.equal(isInsideRoot(root, "src/linked.env"), true);
    assert.equal(await isRealPathInsideRoot(root, "src/linked.env"), false);
    assert.equal(await isRealPathInsideRoot(root, "linked-dir/secret.env"), false);
    assert.equal(await isRealPathInsideRoot(root, path.join(root, "linked-dir", "secret.env")), false);
    // A file that does not exist yet behind a linked directory is outside too.
    assert.equal(await isRealPathInsideRoot(root, "linked-dir/new.ts"), false);
  });

  it("refuses a dangling link and a link that stays inside the workspace is fine", async () => {
    fs.symlinkSync(path.join(outside, "missing"), path.join(root, "dangling"));
    assert.equal(await isRealPathInsideRoot(root, "dangling"), false);
    fs.symlinkSync(path.join(root, "src", "inside.ts"), path.join(root, "alias.ts"));
    assert.equal(await isRealPathInsideRoot(root, "alias.ts"), true);
  });

  it("still refuses traversal, the root itself and an unresolvable root", async () => {
    assert.equal(await isRealPathInsideRoot(root, "../outside/secret.env"), false);
    assert.equal(await isRealPathInsideRoot(root, root), false);
    assert.equal(await isRealPathInsideRoot(path.join(tmp, "no-such-root"), "a.ts"), false);
    assert.equal(await isRealPathInsideRoot("", "a.ts"), false);
  });
});

describe("statusForEvent", () => {
  it("maps terminal events", () => {
    assert.equal(statusForEvent("item.completed"), "completed");
    assert.equal(statusForEvent("item.failed"), "failed");
    assert.equal(statusForEvent("item.interrupted"), "interrupted");
    assert.equal(statusForEvent("item.canceled"), "interrupted");
    assert.equal(statusForEvent("item.started"), "in_progress");
  });
});
