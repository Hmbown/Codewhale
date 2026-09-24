// Automated coverage for the dashboard's target-resolution rail (#4397).
//
// These functions are the only thing standing between "the user clicked a row"
// and "a reply or an approval was POSTed somewhere". Every branch here is a
// fail-closed decision, so every branch is asserted: a saved session is never
// replied to, a stale selection never sends, and an approval that is not in the
// live thread's own approval set is never answered.
//
// Run by `npm test` in `web/` — see `web/vitest.config.ts`, whose `include`
// reaches this file deliberately so the rail cannot regress without CI saying
// so. The module is import-safe outside a browser: `app.mjs` only calls
// `startBrowserClient()` when `document` exists.

import { describe, expect, it } from "vitest";

import {
  NO_TARGET,
  boundedDiffLines,
  canReply,
  collectProviderModelPages,
  completedElapsedSuffix,
  diffLineKind,
  formatElapsedMs,
  refusalMessage,
  receiptPresentation,
  resolveApprovalTarget,
  resolveReplyTarget,
  runningElapsedSuffix,
  sessionTarget,
  streamCursor,
  threadTarget,
  workflowReceiptPresentation,
} from "./app.mjs";

describe("runningElapsedSuffix", () => {
  const at = (secsAgo) => new Date(Date.now() - secsAgo * 1000).toISOString();

  it("suppresses the badge for tools that resolve quickly (< 3s)", () => {
    expect(runningElapsedSuffix({ kind: "tool_call", status: "in_progress", started_at: at(1) })).toBe("");
    expect(runningElapsedSuffix({ kind: "tool_call", status: "in_progress", started_at: at(2) })).toBe("");
  });

  it("ticks the seconds once a tool has been in flight for 3s or more", () => {
    expect(runningElapsedSuffix({ kind: "tool_call", status: "in_progress", started_at: at(3) })).toBe(" (3s)");
    expect(runningElapsedSuffix({ kind: "tool_call", status: "in_progress", started_at: at(12) })).toBe(" (12s)");
  });

  it("never badges something that is not in flight", () => {
    expect(runningElapsedSuffix({ kind: "tool_call", status: "completed", started_at: at(30) })).toBe("");
    expect(runningElapsedSuffix({ kind: "status", status: "in_progress", started_at: at(30) })).toBe("");
    expect(runningElapsedSuffix({ kind: "tool_call", status: "in_progress" })).toBe("");
    expect(runningElapsedSuffix(null)).toBe("");
  });
});

describe("inline diff (inline_diffs=full, bounded to 14 lines)", () => {
  const diff = ["--- a/x.txt", "+++ b/x.txt", "@@ -1,3 +1,3 @@", " keep", "-old", "+new"].join("\n");

  it("keeps a short diff whole and reports nothing omitted", () => {
    const out = boundedDiffLines(diff);
    expect(out.omitted).toBe(0);
    expect(out.lines.length).toBe(6);
  });

  it("bounds a long diff and says how many rows were left out", () => {
    const long = ["--- a/x", "+++ b/x", ...Array.from({ length: 50 }, (_, i) => `+line ${i}`)].join("\n");
    const out = boundedDiffLines(long);
    expect(out.lines.length).toBe(14);
    expect(out.omitted).toBe(38);
  });

  it("classifies rows for the red/green rendering", () => {
    expect(diffLineKind("+++ b/x")).toBe("meta");
    expect(diffLineKind("--- a/x")).toBe("meta");
    expect(diffLineKind("@@ -1 +1 @@")).toBe("hunk");
    expect(diffLineKind("+added")).toBe("add");
    expect(diffLineKind("-removed")).toBe("del");
    expect(diffLineKind(" context")).toBe("");
    expect(diffLineKind("")).toBe("");
  });

  it("treats an empty diff as empty", () => {
    expect(boundedDiffLines("").lines.length).toBe(0);
    expect(boundedDiffLines(null).omitted).toBe(0);
  });
});

describe("completedElapsedSuffix / formatElapsedMs (CLI's `reasoning done · 521ms`)", () => {
  const item = (ms) => ({
    kind: "agent_reasoning",
    status: "completed",
    started_at: new Date(Date.now() - ms).toISOString(),
    ended_at: new Date().toISOString(),
  });

  it("formats sub-second as ms and longer as seconds", () => {
    expect(formatElapsedMs(521)).toBe("521ms");
    expect(formatElapsedMs(0)).toBe("0ms");
    expect(formatElapsedMs(1500)).toBe("2s");
    expect(formatElapsedMs(65000)).toBe("1m 5s");
  });

  it("only badges completed items, and only with both timestamps", () => {
    expect(completedElapsedSuffix(item(500))).toBe(" · 500ms");
    expect(completedElapsedSuffix({ ...item(500), status: "in_progress" })).toBe("");
    expect(completedElapsedSuffix({ kind: "agent_reasoning", status: "completed" })).toBe("");
    expect(completedElapsedSuffix(null)).toBe("");
  });
});

describe("collectProviderModelPages", () => {
  it("loads a 600-model catalog through every opaque page", async () => {
    const all = Array.from({ length: 600 }, (_, index) => ({
      id: `openrouter/model-${String(index).padStart(3, "0")}`,
      image_input: "unknown",
    }));
    const cursors = new Map([
      ["", { start: 0, nextCursor: "page-2" }],
      ["page-2", { start: 250, nextCursor: "page-3" }],
      ["page-3", { start: 500, nextCursor: "" }],
    ]);
    const paths = [];
    const models = await collectProviderModelPages("openrouter", async (path) => {
      paths.push(path);
      const url = new URL(path, "http://runtime.local");
      const cursor = url.searchParams.get("cursor") || "";
      const page = cursors.get(cursor);
      expect(url.searchParams.get("limit")).toBe("250");
      expect(page).toBeDefined();
      return {
        provider: "openrouter",
        models: all.slice(page.start, page.start + 250),
        total: all.length,
        ...(page.nextCursor ? { nextCursor: page.nextCursor } : {}),
      };
    });

    expect(models).toEqual(all);
    expect(paths).toHaveLength(3);
  });

  it("rejects truncated pages, malformed rows and changing totals", async () => {
    for (const response of [
      { provider: "openrouter", models: [{ id: "one" }], total: 2 },
      { provider: "openrouter", models: null, total: 0 },
      { provider: "another-provider", models: [], total: 0 },
    ]) {
      await expect(collectProviderModelPages("openrouter", async () => response)).rejects.toThrow();
    }
    let count = 0;
    await expect(collectProviderModelPages("openrouter", async () => ({
      provider: "openrouter",
      models: [{ id: String(count) }],
      total: ++count === 1 ? 2 : 3,
      nextCursor: "next",
    }))).rejects.toThrow("catalog changed");
  });

  it("rejects a repeated or non-progressing cursor", async () => {
    await expect(
      collectProviderModelPages("openrouter", async () => ({
        provider: "openrouter",
        models: [{ id: "model-a", image_input: "unknown" }],
        total: 2,
        nextCursor: "same-page",
      })),
    ).rejects.toThrow("引擎返回的模型游标没有前进。");
  });
});

describe("receiptPresentation", () => {
  it("keeps a failed MCP transport compact while preserving the raw receipt", () => {
    const raw = "Failed to connect MCP server 'github': Stdio transport closed MCP server stderr (last 1 line): Docker is not running";
    expect(receiptPresentation({ kind: "status", status: "completed", summary: raw })).toEqual({
      label: "MCP · 不可用",
      summary: "github 连不上",
      raw,
      failed: true,
      variant: "mcp",
      meta: "",
    });
  });

  it("does not rewrite ordinary receipts", () => {
    expect(receiptPresentation({ kind: "tool_result", status: "completed", summary: "3 tests passed" })).toEqual({
      label: "工具 · 完成",
      summary: "3 tests passed",
      raw: "3 tests passed",
      failed: false,
      variant: "generic",
      meta: "",
    });
  });

  // AsBudy（2026-09-18 老板：「CLI 的工具卡是按类型分开渲染的，直接做了吧」）：
  // 形态分类 + 徽标（耗时/非零退出码）—— 样式在 asbudy-my.js 注入，看 data-variant。
  it("AsBudy: 按工具类型分形态（exec/file/explore/status），并给徽标", () => {
    const exec = receiptPresentation({
      kind: "tool_call",
      status: "completed",
      summary: "bash: out",
      metadata: { tool_name: "bash", tool_input: JSON.stringify({ command: "ls" }), duration_ms: 1234, exit_code: 0 },
    });
    expect(exec.variant).toBe("exec");
    expect(exec.meta).toBe("1.2s");

    const bad = receiptPresentation({
      kind: "tool_call", status: "failed", summary: "bash failed: x",
      metadata: { tool_name: "bash", tool_input: JSON.stringify({ command: "node x.js" }), duration_ms: 800, exit_code: 1 },
    });
    expect(bad.meta).toBe("800ms · 退出码 1");

    expect(receiptPresentation({
      kind: "file_change", status: "completed", summary: "write ok",
      metadata: { tool_name: "write", tool_input: JSON.stringify({ path: "public/index.html" }) },
    }).variant).toBe("file");

    expect(receiptPresentation({
      kind: "tool_call", status: "completed", summary: "read: x",
      metadata: { tool_name: "read", tool_input: JSON.stringify({ path: "AGENTS.md" }) },
    }).variant).toBe("explore");

    expect(receiptPresentation({ kind: "status", status: "completed", summary: "Continuing — tool results" }).variant).toBe("status");
  });

  // AsBudy（2026-09-18 老板：「全程没有任何中文、没有进度反馈」）：
  // 回执说人话 —— 标签中文 + 摘要讲「做了什么」；原文仍留在 raw 里可展开。
  // 引擎 metadata 里本来就带着 tool_name / tool_input（以前完全没用上）。
  it("AsBudy: 用命令/路径说清做了什么，原文仍可展开", () => {
    const raw = "bash: total 36\ndrwxr-x---+ 8 cus-yanyijin ...";
    const presented = receiptPresentation({
      kind: "tool_call",
      status: "completed",
      summary: raw,
      metadata: { tool_name: "bash", tool_input: JSON.stringify({ command: "cd /app && ls -la" }) },
    });
    // AsBudy 第 47 轮：工具卡按族分类（bash = 执行族 ⇒「运行」）
    expect(presented.label).toBe("运行 · 完成");
    expect(presented.summary).toBe("执行命令：cd /app && ls -la");
    expect(presented.raw).toContain("drwxr-x");  // 目录行可能带 ACL 标记（drwxr-x---+）
    expect(presented.failed).toBe(false);
  });

  it("AsBudy: 失败时先说做了什么，再缀「未完成」", () => {
    const presented = receiptPresentation({
      kind: "file_change",
      status: "failed",
      summary: "write failed: Failed to authorize tool execution: Auto-Review guardian denied...",
      metadata: { tool_name: "write", tool_input: JSON.stringify({ path: "/opt/app/public/index.html" }) },
    });
    // AsBudy 第 47 轮：write/apply_patch 归「修补」族
    expect(presented.label).toBe("修补 · 未完成");
    expect(presented.summary).toBe("写入 public/index.html —— 未完成");
    expect(presented.failed).toBe(true);
  });

  it("AsBudy: 引擎的英文状态句译成中文，认不出的句子退回原文", () => {
    expect(receiptPresentation({ kind: "status", status: "completed", summary: "Auto-Review checking 'bash'" }).summary)
      .toBe("已检查这一步（bash）");
    expect(receiptPresentation({ kind: "status", status: "completed", summary: "Continuing — tool results" }).summary)
      .toBe("拿到结果，继续");
    expect(receiptPresentation({ kind: "status", status: "completed", summary: "Some brand new engine sentence" }).summary)
      .toBe("Some brand new engine sentence");
  });
});

describe("workflowReceiptPresentation", () => {
  it("summarises a single rejected dispatch and keeps the raw receipt", () => {
    const raw = '{"status":"degraded","dispatch_failure_count":1}';
    expect(
      workflowReceiptPresentation(
        { summary: "workflow: check", status: "completed" },
        raw,
        "---"
      ),
    ).toEqual({
      label: "工作流 · 需要处理",
      summary: "有 1 个任务派发被拒绝了",
      raw: "---",
      failed: true,
      variant: "plan",
    });
  });

  it("summarises multiple rejected dispatches in the plural", () => {
    const raw = '{"status":"degraded","dispatch_failure_count":3}';
    const result = workflowReceiptPresentation(
      { summary: "workflow: check", status: "completed" },
      raw,
      null
    );
    expect(result.summary).toBe("3 个任务派发被拒绝");
    expect(result.failed).toBe(true);
  });

  it("keeps a failed workflow without a dispatch count honest", () => {
    const result = workflowReceiptPresentation(
      { summary: "workflow: gate", status: "completed" },
      '{"status":"failed"}',
      null
    );
    expect(result).toEqual({
      label: "工作流 · 失败",
      summary: "工作流未完成",
      raw: null,
      failed: true,
      variant: "plan",
    });
  });

  it("labels degraded completions without rejected dispatches as needs attention", () => {
    const result = workflowReceiptPresentation(
      { summary: "workflow: check", status: "completed" },
      '{"status":"degraded"}',
      null
    );
    expect(result.label).toBe("工作流 · 需要处理");
    expect(result.summary).toBe("工作流完成，但结果降级");
  });

  it("returns null for non-workflow receipts", () => {
    expect(
      workflowReceiptPresentation(
        { summary: "3 tests passed", status: "completed" },
        "3 tests passed",
        null
      )
    ).toBeNull();
  });
});

/** Minimal stand-in for the live stream state the real client threads through. */
function streamState(threadId, approvalIds = []) {
  return { threadId, approvals: new Map(approvalIds.map((id) => [id, {}])) };
}

describe("canReply", () => {
  it("admits only a live thread with an id", () => {
    expect(canReply(threadTarget("thread-a"))).toBe(true);
    expect(canReply(sessionTarget("session-a"))).toBe(false);
    expect(canReply(NO_TARGET)).toBe(false);
    expect(canReply(threadTarget(""))).toBe(false);
    expect(canReply(undefined)).toBe(false);
  });
});

describe("resolveReplyTarget", () => {
  it("resolves a live thread that the stream is following", () => {
    const resolved = resolveReplyTarget(threadTarget("thread-a"), streamState("thread-a"));
    expect(resolved).toEqual({ ok: true, threadId: "thread-a" });
  });

  it("resolves a live thread before any stream state exists", () => {
    // A freshly created thread has no stream yet; refusing here would make the
    // first message of every new thread impossible to send.
    expect(resolveReplyTarget(threadTarget("thread-a"), null)).toEqual({
      ok: true,
      threadId: "thread-a",
    });
    expect(resolveReplyTarget(threadTarget("thread-a"), { threadId: "" })).toEqual({
      ok: true,
      threadId: "thread-a",
    });
  });

  it("refuses a saved session — a recording has no runtime to receive a reply", () => {
    expect(resolveReplyTarget(sessionTarget("session-a"), streamState("thread-a"))).toEqual({
      ok: false,
      reason: "session-not-live",
    });
  });

  it("refuses when nothing is selected", () => {
    expect(resolveReplyTarget(NO_TARGET, streamState("thread-a"))).toEqual({
      ok: false,
      reason: "no-target",
    });
    expect(resolveReplyTarget(null, streamState("thread-a"))).toEqual({
      ok: false,
      reason: "no-target",
    });
    expect(resolveReplyTarget(threadTarget(""), streamState("thread-a"))).toEqual({
      ok: false,
      reason: "no-target",
    });
  });

  it("refuses a stale target: the stream moved on while the user did not", () => {
    expect(resolveReplyTarget(threadTarget("thread-a"), streamState("thread-b"))).toEqual({
      ok: false,
      reason: "stale-target",
    });
  });

  it("never returns an id on refusal", () => {
    for (const [target, state] of [
      [sessionTarget("session-a"), streamState("thread-a")],
      [NO_TARGET, streamState("thread-a")],
      [threadTarget("thread-a"), streamState("thread-b")],
    ]) {
      expect(resolveReplyTarget(target, state).threadId).toBeUndefined();
    }
  });
});

describe("resolveApprovalTarget", () => {
  it("resolves an approval the watched thread actually holds", () => {
    const state = streamState("thread-a", ["approval-1"]);
    expect(resolveApprovalTarget("approval-1", threadTarget("thread-a"), state)).toEqual({
      ok: true,
      threadId: "thread-a",
      approvalId: "approval-1",
    });
  });

  it("inherits every reply refusal — authority cannot outrank the target check", () => {
    const state = streamState("thread-a", ["approval-1"]);
    expect(resolveApprovalTarget("approval-1", sessionTarget("session-a"), state).reason).toBe(
      "session-not-live",
    );
    expect(resolveApprovalTarget("approval-1", NO_TARGET, state).reason).toBe("no-target");
    expect(
      resolveApprovalTarget("approval-1", threadTarget("thread-b"), state).reason,
    ).toBe("stale-target");
  });

  it("refuses an approval that is not in the live approval set", () => {
    // Already decided, expired, or belonging to a thread we stopped watching.
    const state = streamState("thread-a", ["approval-1"]);
    expect(resolveApprovalTarget("approval-2", threadTarget("thread-a"), state)).toEqual({
      ok: false,
      reason: "stale-approval",
    });
  });

  it("refuses when the stream state is missing or belongs to another thread", () => {
    expect(resolveApprovalTarget("approval-1", threadTarget("thread-a"), null)).toEqual({
      ok: false,
      reason: "stale-target",
    });
    // A reply would be allowed here (no stream yet), but an approval must not:
    // there is no live approval set to check membership against.
    expect(
      resolveApprovalTarget("approval-1", threadTarget("thread-a"), { threadId: "" }),
    ).toEqual({ ok: false, reason: "stale-target" });
  });

  it("refuses an unidentified approval", () => {
    const state = streamState("thread-a", ["approval-1"]);
    expect(resolveApprovalTarget("", threadTarget("thread-a"), state)).toEqual({
      ok: false,
      reason: "no-approval",
    });
  });

  it("refuses when the thread has no approvals at all", () => {
    expect(
      resolveApprovalTarget("approval-1", threadTarget("thread-a"), { threadId: "thread-a" }),
    ).toEqual({ ok: false, reason: "stale-approval" });
  });

  it("never returns an approval id on refusal", () => {
    const state = streamState("thread-a", ["approval-1"]);
    for (const [id, target] of [
      ["approval-2", threadTarget("thread-a")],
      ["approval-1", sessionTarget("session-a")],
      ["", threadTarget("thread-a")],
    ]) {
      expect(resolveApprovalTarget(id, target, state).approvalId).toBeUndefined();
    }
  });
});

describe("refusalMessage", () => {
  it("gives every refusal reason a distinct user-visible sentence", () => {
    const reasons = ["session-not-live", "stale-target", "stale-approval", "no-approval", "no-target"];
    const messages = reasons.map(refusalMessage);
    expect(new Set(messages).size).toBe(reasons.length);
    for (const message of messages) {
      // Every refusal must state the outcome, not just the cause: the user
      // needs to know their message did not go anywhere.
      // AsBudy：文案已中文化 —— 五条都写明「没有发送」，官方这句断言要的意图不变
      expect(message).toContain("没有发送");
    }
  });

  it("falls back to the select-a-thread message for an unknown reason", () => {
    expect(refusalMessage("something-new")).toBe(refusalMessage("no-target"));
  });
});

describe("streamCursor", () => {
  it("reports the resume sequence and a live label", () => {
    expect(streamCursor({ latestSeq: 12 })).toEqual({
      latestSeq: 12,
      gap: false,
      connected: true,
      label: "实时——事件 #12",
    });
  });

  it("names a gap and a disconnect distinctly, both carrying the resume point", () => {
    expect(streamCursor({ latestSeq: 5 }, { gap: true }).label).toBe(
      "检测到断档——从 #5 重新同步",
    );
    expect(streamCursor({ latestSeq: 5 }, { connected: false }).label).toBe(
      "重新连接中——从 #5 续上",
    );
  });

  it("normalizes a missing or nonsense sequence to zero rather than NaN", () => {
    expect(streamCursor(undefined).latestSeq).toBe(0);
    expect(streamCursor({ latestSeq: "not a number" }).latestSeq).toBe(0);
  });
});
