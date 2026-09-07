import * as vscode from "vscode";

/** A piece of editor context attached to the next prompt. */
export interface ContextChip {
  id: string;
  kind: "selection" | "file" | "diagnostics";
  label: string;
  detail?: string;
  /** Full text included in the assembled prompt; may be truncated. */
  body: string;
}

const MAX_BODY_CHARS = 8000;

let chipCounter = 0;

export function collectSelectionContext(): ContextChip | undefined {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.selection.isEmpty) {
    return undefined;
  }
  const selection = editor.selection;
  const text = editor.document.getText(selection);
  if (text.trim().length === 0) {
    return undefined;
  }
  const relative = workspaceRelativePath(editor.document.uri);
  const startLine = selection.start.line + 1;
  const endLine = selection.end.line + 1;
  const lines = endLine - startLine + 1;
  return {
    id: `chip-${++chipCounter}`,
    kind: "selection",
    label: `${relative}:${startLine}-${endLine}`,
    detail: `${lines} line${lines === 1 ? "" : "s"}`,
    body: clip(`${text}\n`),
  };
}

export function collectActiveFileContext(): ContextChip | undefined {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    return undefined;
  }
  const relative = workspaceRelativePath(editor.document.uri);
  const text = editor.document.getText();
  return {
    id: `chip-${++chipCounter}`,
    kind: "file",
    label: relative,
    detail: `${editor.document.lineCount} lines`,
    body: clip(text),
  };
}

export function collectDiagnosticsContext(): ContextChip | undefined {
  const editor = vscode.window.activeTextEditor;
  const entries: string[] = [];
  const uris = editor
    ? [editor.document.uri]
    : vscode.workspace.textDocuments.map((document) => document.uri);
  for (const uri of uris) {
    for (const diagnostic of vscode.languages.getDiagnostics(uri)) {
      if (diagnostic.severity > vscode.DiagnosticSeverity.Warning) {
        continue;
      }
      const position = `${uri.path.split("/").pop()}:${diagnostic.range.start.line + 1}`;
      const severity = diagnostic.severity === vscode.DiagnosticSeverity.Error ? "error" : "warn";
      entries.push(`- [${severity}] ${position} ${diagnostic.message.split("\n")[0]}`);
      if (entries.length >= 20) {
        break;
      }
    }
  }
  if (entries.length === 0) {
    return undefined;
  }
  return {
    id: `chip-${++chipCounter}`,
    kind: "diagnostics",
    label: "Problems",
    detail: `${entries.length} entrie${entries.length === 1 ? "" : "s"}`,
    body: clip(entries.join("\n")),
  };
}

/** Assemble the final prompt with context blocks the runtime model can read. */
export function assemblePrompt(prompt: string, chips: ContextChip[]): string {
  if (chips.length === 0) {
    return prompt;
  }
  const sections = chips.map((chip) => {
    const header =
      chip.kind === "diagnostics"
        ? "Diagnostics (problems panel)"
        : `File \`${chip.label}\`${chip.kind === "selection" ? " (selected lines)" : ""}`;
    return `--- ${header} ---\n${chip.body}`;
  });
  return `${prompt}\n\n[Attached context]\n${sections.join("\n\n")}`;
}

export function workspaceRelativePath(uri: vscode.Uri): string {
  const folder = vscode.workspace.getWorkspaceFolder(uri);
  if (!folder) {
    return uri.fsPath;
  }
  const prefix = folder.uri.fsPath.endsWith("/")
    ? folder.uri.fsPath
    : `${folder.uri.fsPath}/`;
  return uri.fsPath.startsWith(prefix) ? uri.fsPath.slice(prefix.length) : uri.fsPath;
}

function clip(text: string): string {
  if (text.length <= MAX_BODY_CHARS) {
    return text;
  }
  return `${text.slice(0, MAX_BODY_CHARS)}\n… [truncated ${text.length - MAX_BODY_CHARS} chars]`;
}
