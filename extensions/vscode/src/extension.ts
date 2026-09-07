import * as vscode from "vscode";
import { checkConnection, listSnapshots, type ApiConfig, type ConnectionInfo } from "./api";
import { ChatView } from "./chat";
import {
  openCodeWhaleTerminal,
  readRuntimeConfig,
  runtimeBaseUrl,
  startRuntimeTerminal,
  type RuntimeState,
} from "./runtime";
import { promptForToken, resolveToken } from "./secrets";
import { RuntimeStatusView } from "./status";

export function activate(context: vscode.ExtensionContext): void {
  const output = vscode.window.createOutputChannel("CodeWhale");
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  const statusView = new RuntimeStatusView();
  const apiConfig = async (): Promise<ApiConfig> => {
    const config = readRuntimeConfig();
    // SecretStorage is the only token source; `resolveToken` treats the
    // deprecated `codewhale.runtimeToken` setting as a migration source and
    // ignores a workspace-scoped one outright.
    const token = await resolveToken(context);
    return { baseUrl: runtimeBaseUrl(config), token };
  };
  const chatView = new ChatView(context, apiConfig, output);
  let autoRefreshTimer: ReturnType<typeof setInterval> | undefined;
  let autoRefreshInFlight = false;
  let lastConnectionKind: ConnectionInfo["kind"] | undefined;

  status.command = "codewhale.checkRuntime";
  context.subscriptions.push(output, status);
  // The chat lives in the secondary (right) sidebar on hosts that support it,
  // and falls back to the activity bar on older ones. The manifest gates both
  // containers on `codewhale.noSecondarySidebar`, so this key MUST be set at
  // activation — an unset key is falsy, which would hide the activity-bar
  // container AND leave the secondary panel unserved. Threshold matches the
  // shipping Codex extension, which gates at runtime rather than via engines.
  const [vsMajor = 0, vsMinor = 0] = vscode.version
    .split(".")
    .map((part) => Number.parseInt(part, 10) || 0);
  const supportsSecondarySidebar = vsMajor > 1 || (vsMajor === 1 && vsMinor >= 106);
  void vscode.commands.executeCommand(
    "setContext",
    "codewhale.noSecondarySidebar",
    !supportsSecondarySidebar,
  );

  // One ChatView instance serves both ids; only the gated one ever resolves.
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(ChatView.viewType, chatView),
    vscode.window.registerWebviewViewProvider(ChatView.secondaryViewType, chatView),
    vscode.window.registerWebviewViewProvider(RuntimeStatusView.viewType, statusView),
  );

  const updateStatus = (text: string, tooltip: string): void => {
    status.text = text;
    status.tooltip = tooltip;
    status.show();
  };

  const checkAndRefreshRuntime = async (
    showSpinner: boolean,
    logResult: boolean,
  ): Promise<RuntimeState> => {
    const config = readRuntimeConfig();
    const baseUrl = runtimeBaseUrl(config);
    if (showSpinner) {
      updateStatus("$(sync~spin) CodeWhale", "Checking CodeWhale runtime...");
    }

    let connection: ConnectionInfo;
    try {
      connection = await checkConnection(await apiConfig());
    } catch (error: unknown) {
      const detail = error instanceof Error ? error.message : String(error);
      connection = { kind: "error", detail };
    }
    const state: RuntimeState = { ...connection, baseUrl };

    statusView.update(state);
    chatView.setConnection(connection);

    const becameConnected =
      connection.kind === "connected" && lastConnectionKind !== "connected";
    lastConnectionKind = connection.kind;

    switch (connection.kind) {
      case "connected":
        updateStatus("$(check) CodeWhale", state.detail);
        await chatView.refreshThreads();
        if (becameConnected) {
          await chatView.resyncAfterConnection();
        }
        break;
      case "auth-required":
        updateStatus("$(lock) CodeWhale", state.detail);
        statusView.updateThreads([], "Runtime token is required before threads can load.");
        statusView.updateSnapshots([], "Runtime token is required before restore points can load.");
        break;
      case "offline":
      case "error":
        updateStatus("$(warning) CodeWhale", state.detail);
        statusView.updateThreads([], "Connect to the runtime to load recent threads.");
        statusView.updateSnapshots([], "Connect to the runtime to load restore points.");
        break;
    }

    if (logResult) {
      output.appendLine(`${new Date().toISOString()} ${state.kind}: ${state.detail}`);
    }
    return state;
  };

  const runAutoRefresh = async (): Promise<void> => {
    if (autoRefreshInFlight) {
      return;
    }
    autoRefreshInFlight = true;
    try {
      await checkAndRefreshRuntime(false, false);
    } finally {
      autoRefreshInFlight = false;
    }
  };

  const scheduleAutoRefresh = (): void => {
    if (autoRefreshTimer) {
      clearInterval(autoRefreshTimer);
      autoRefreshTimer = undefined;
    }
    const intervalSeconds = readRuntimeConfig().agentViewRefreshIntervalSeconds;
    if (intervalSeconds === 0) {
      output.appendLine("Auto-refresh is disabled.");
      return;
    }
    autoRefreshTimer = setInterval(() => {
      void runAutoRefresh();
    }, intervalSeconds * 1000);
    output.appendLine(`Runtime auto-refresh scheduled every ${intervalSeconds}s.`);
  };

  updateStatus("$(terminal) CodeWhale", "Check CodeWhale runtime");
  scheduleAutoRefresh();
  context.subscriptions.push(
    new vscode.Disposable(() => {
      if (autoRefreshTimer) {
        clearInterval(autoRefreshTimer);
      }
    }),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (
        event.affectsConfiguration("codewhale.agentViewRefreshIntervalSeconds") ||
        event.affectsConfiguration("codewhale.runtimeHost") ||
        event.affectsConfiguration("codewhale.runtimePort") ||
        event.affectsConfiguration("codewhale.runtimeToken")
      ) {
        lastConnectionKind = undefined;
        scheduleAutoRefresh();
        void checkAndRefreshRuntime(false, true);
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("codewhale.openTerminal", () => {
      openCodeWhaleTerminal(readRuntimeConfig());
      output.appendLine(`Opened CodeWhale terminal using ${readRuntimeConfig().commandPath}.`);
    }),
    vscode.commands.registerCommand("codewhale.startRuntime", async () => {
      const config = readRuntimeConfig();
      const baseUrl = runtimeBaseUrl(config);
      const token = await resolveToken(context);

      // Anything that answers on this address is already bound to the port; a
      // second `serve` would only fail noisily, so report instead of starting.
      let bound: ConnectionInfo | undefined;
      try {
        bound = await checkConnection({ baseUrl, token });
      } catch {
        bound = undefined;
      }
      if (bound && bound.kind !== "offline") {
        const detail = `A runtime is already listening at ${baseUrl}: ${bound.detail}`;
        output.appendLine(detail);
        void vscode.window.showInformationMessage(detail);
        await checkAndRefreshRuntime(false, false);
        return;
      }

      startRuntimeTerminal(config, token);
      updateStatus("$(sync~spin) CodeWhale", `Runtime terminal started for ${baseUrl}`);
      output.appendLine(`Started CodeWhale runtime terminal at ${baseUrl}.`);
      void vscode.window.showInformationMessage(`CodeWhale runtime starting at ${baseUrl}`);
    }),
    vscode.commands.registerCommand("codewhale.checkRuntime", async () => {
      return await checkAndRefreshRuntime(true, true);
    }),
    vscode.commands.registerCommand("codewhale.refreshAgentView", async () => {
      await chatView.refreshThreads();
    }),
    vscode.commands.registerCommand("codewhale.refreshSnapshots", async () => {
      try {
        const snapshots = await listSnapshots(await apiConfig());
        statusView.updateSnapshots(snapshots, "Showing recent restore points.");
      } catch (error: unknown) {
        const detail = error instanceof Error ? error.message : String(error);
        statusView.updateSnapshots([], detail);
        output.appendLine(`Runtime restore points unavailable: ${detail}`);
        void vscode.window.showWarningMessage(detail);
      }
    }),
    vscode.commands.registerCommand("codewhale.openRuntimeDocs", () => {
      void vscode.env.openExternal(
        vscode.Uri.parse("https://github.com/Hmbown/CodeWhale/blob/main/docs/RUNTIME_API.md"),
      );
    }),
    vscode.commands.registerCommand("codewhale.ask", async () => {
      await chatView.askWithSelection();
    }),
    vscode.commands.registerCommand("codewhale.newChat", async () => {
      await chatView.reveal();
      await chatView.newThread();
    }),
    vscode.commands.registerCommand("codewhale.setRuntimeToken", async () => {
      const token = await promptForToken(context);
      if (token !== undefined) {
        await checkAndRefreshRuntime(true, true);
      }
    }),
  );

  void vscode.commands.executeCommand("codewhale.checkRuntime");
}

export function deactivate(): void {
  // No background process is owned by the extension; runtime starts in a user-visible terminal.
}
