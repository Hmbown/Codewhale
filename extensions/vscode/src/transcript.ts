/**
 * Transcript projection: turns runtime `ItemRecord`s (snapshot rows and SSE
 * payloads) into the `ItemView` shape the chat webview renders.
 *
 * This is deliberately pure and free of `vscode` imports so it can be unit
 * tested under plain `node --test`. Two behaviors live here because both are
 * trust-boundary sensitive and were getting them wrong in the view layer:
 *
 * - `detail` vs `summary`: the runtime truncates `summary` to a stub and puts
 *   the real body in `detail`, which is `skip_serializing_if` on the wire and
 *   therefore legitimately absent. Prefer `detail`, fall back to `summary`.
 * - file paths for the "Open file" action arrive inside `metadata.tool_input`
 *   (a JSON string), not as `metadata.path`. That value is model-influenced,
 *   so it is parsed defensively here and containment-checked before use.
 */

import * as path from "node:path";
import type { ItemRecord } from "./api";
import { renderMarkdown } from "./markdown";

export interface ItemView {
  id: string;
  kind: string;
  status?: string;
  turnId?: string;
  summary: string;
  detail?: string;
  metadata?: Record<string, unknown>;
  /** Workspace-relative or absolute path parsed out of tool metadata, if any. */
  filePath?: string;
  /** Rendered markdown for completed agent messages. */
  html?: string;
  codeBlocks?: string[];
  /** In-progress agent text (plain, re-rendered on completion). */
  streamText?: string;
  rev: number;
}

/** Item status implied by an SSE event name. */
export function statusForEvent(event: string): string {
  if (event === "item.completed") {
    return "completed";
  }
  if (event === "item.failed") {
    return "failed";
  }
  if (event === "item.interrupted" || event === "item.canceled") {
    return "interrupted";
  }
  return "in_progress";
}

/**
 * Build the view for one item, merging with whatever is already on screen so a
 * partial SSE payload never blanks text that was already rendered.
 */
export function projectItem(
  item: ItemRecord,
  existing: ItemView | undefined,
  event?: string,
): ItemView {
  const isTerminal = event === "item.completed" || item.status === "completed";
  const streamText = existing?.streamText;
  const rev = (existing?.rev ?? 0) + 1;
  const turnId = item.turnId ?? existing?.turnId;
  const detail = item.detail ?? existing?.detail;
  const metadata = item.metadata ?? existing?.metadata;

  if (item.kind === "agent_message") {
    // `detail` carries the full reply; `summary` is a 280-char stub on reload.
    const text = detail || item.summary || streamText || existing?.summary || "";
    if (!isTerminal) {
      return {
        id: item.id,
        kind: item.kind,
        status: item.status,
        turnId,
        summary: text,
        detail,
        streamText: text,
        rev,
      };
    }
    const rendered = renderMarkdown(text);
    return {
      id: item.id,
      kind: item.kind,
      status: item.status,
      turnId,
      summary: text,
      detail,
      html: rendered.html,
      codeBlocks: rendered.codeBlocks,
      rev,
    };
  }

  return {
    id: item.id,
    kind: item.kind,
    status: item.status,
    turnId,
    summary: item.summary || existing?.summary || "",
    detail,
    metadata,
    filePath: extractFilePath(metadata),
    rev,
  };
}

const PATH_KEYS = ["path", "file_path", "filePath", "file", "notebook_path", "target_file"];

function firstStringField(record: Record<string, unknown>, keys: readonly string[]): string | undefined {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim() !== "") {
      return value.trim();
    }
  }
  return undefined;
}

/**
 * Pull a file path out of item metadata. The runtime puts tool arguments in
 * `metadata.tool_input` as a JSON string, so a direct `metadata.path` lookup
 * finds nothing for the file-change items that most want an Open button.
 * Everything here is untrusted model output: parse failures are swallowed and
 * callers must still validate the result before opening it.
 */
export function extractFilePath(metadata: Record<string, unknown> | undefined): string | undefined {
  if (!metadata) {
    return undefined;
  }
  const direct = firstStringField(metadata, PATH_KEYS);
  if (direct) {
    return sanitizePath(direct);
  }
  const raw = metadata.tool_input ?? metadata.toolInput ?? metadata.input ?? metadata.arguments;
  let parsed: unknown = raw;
  if (typeof raw === "string") {
    try {
      parsed = JSON.parse(raw) as unknown;
    } catch {
      return undefined; // not JSON; nothing safe to offer
    }
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return undefined;
  }
  const nested = firstStringField(parsed as Record<string, unknown>, PATH_KEYS);
  return nested ? sanitizePath(nested) : undefined;
}

/** Reject paths carrying control characters or NULs before they reach the FS. */
function sanitizePath(value: string): string | undefined {
  return /[\u0000-\u001f\u007f]/.test(value) ? undefined : value;
}

/**
 * True when `candidate` resolves strictly inside `root`. Used to keep a
 * model-supplied path from escaping the workspace via `..` or an absolute
 * path somewhere else on disk.
 */
export function isInsideRoot(root: string, candidate: string): boolean {
  if (!root) {
    return false;
  }
  const rootAbs = path.resolve(root);
  // Relative candidates resolve against the root, not the process cwd.
  const relative = path.relative(rootAbs, path.resolve(rootAbs, candidate));
  return relative !== "" && !relative.startsWith("..") && !path.isAbsolute(relative);
}
