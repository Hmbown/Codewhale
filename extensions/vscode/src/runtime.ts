import * as vscode from "vscode";
import type { SnapshotEntry, ThreadSummary } from "./api";

export type { SnapshotEntry, ThreadSummary };

export interface RuntimeState {
  kind: "connected" | "offline" | "auth-required" | "error";
  baseUrl: string;
  detail: string;
  version?: string;
}

export interface RuntimeConfig {
  commandPath: string;
  host: string;
  port: number;
  agentViewRefreshIntervalSeconds: number;
}

export function readRuntimeConfig(): RuntimeConfig {
  const config = vscode.workspace.getConfiguration("codewhale");
  const commandPath = config.get<string>("commandPath", "codewhale").trim() || "codewhale";
  const host = config.get<string>("runtimeHost", "127.0.0.1").trim() || "127.0.0.1";
  const port = config.get<number>("runtimePort", 7878);
  const interval = config.get<number>("agentViewRefreshIntervalSeconds", 15);
  // The bearer token is deliberately absent here: it resolves through
  // `secrets.ts`, where SecretStorage wins over the deprecated setting.
  return {
    commandPath,
    host,
    port,
    agentViewRefreshIntervalSeconds: clampRefreshInterval(interval),
  };
}

export function runtimeBaseUrl(config: RuntimeConfig): string {
  return `http://${config.host}:${config.port}`;
}

/**
 * Start `codewhale serve` in a visible terminal.
 *
 * The bearer token never enters argv: a sent command line lands in the terminal
 * buffer, the shell history file, and every local `ps`. The runtime accepts
 * `CODEWHALE_RUNTIME_TOKEN` as the fallback for `--auth-token`
 * (`crates/tui/src/lib.rs:1325-1327`), so it travels in the terminal's
 * environment instead.
 */
export function startRuntimeTerminal(config: RuntimeConfig, token?: string): vscode.Terminal {
  const terminal = vscode.window.createTerminal({
    name: "CodeWhale Runtime",
    env: token ? { CODEWHALE_RUNTIME_TOKEN: token } : undefined,
  });
  const args = [
    "serve",
    "--http",
    "--host",
    shellQuote(config.host),
    "--port",
    String(config.port),
  ];
  terminal.sendText(`${shellQuote(config.commandPath)} ${args.join(" ")}`);
  terminal.show();
  return terminal;
}

export function openCodeWhaleTerminal(config: RuntimeConfig): vscode.Terminal {
  const terminal = vscode.window.createTerminal("CodeWhale");
  terminal.sendText(shellQuote(config.commandPath));
  terminal.show();
  return terminal;
}

function clampRefreshInterval(value: number): number {
  if (!Number.isFinite(value)) {
    return 15;
  }
  return Math.max(0, Math.min(300, Math.floor(value)));
}

function shellQuote(value: string): string {
  if (/^[A-Za-z0-9_./:=+-]+$/.test(value)) {
    return value;
  }
  return `'${value.replace(/'/g, "'\\''")}'`;
}
