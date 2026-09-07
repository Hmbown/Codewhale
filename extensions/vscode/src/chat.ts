import * as crypto from "node:crypto";
import * as path from "node:path";
import * as vscode from "vscode";
import {
  answerUserInput,
  checkConnection,
  createThread,
  getThreadDetail,
  interruptTurn,
  listThreadSummaries,
  openEventStream,
  resolveApproval,
  startTurn,
  steerTurn,
  ApiConfig,
  ConnectionInfo,
  EventStream,
  ItemRecord,
  PendingApproval,
  PendingUserInput,
  ThreadDetail,
  ThreadSummary,
  type RuntimeEvent,
} from "./api";
import { SseParser } from "./sse";
import {
  assemblePrompt,
  collectActiveFileContext,
  collectDiagnosticsContext,
  collectSelectionContext,
  type ContextChip,
} from "./context";
import {
  isInsideRoot,
  projectItem,
  statusForEvent,
  type ItemView,
} from "./transcript";

/**
 * Sidebar chat view: one active Codewhale thread at a time, streaming over
 * the runtime's replayable SSE contract, with inline approvals, clarification
 * questions, steer, and interrupt. State lives here; the webview only
 * renders what it is told and posts intent back.
 */

interface SyncMessage {
  type: "sync";
  connection?: ConnectionInfo;
  threads: ThreadSummary[];
  activeThreadId?: string;
  model?: string;
  streaming: boolean;
  /** Interrupt asked for, turn not ended yet: keep Stop/Steer on screen. */
  interrupting: boolean;
  chips: ContextChip[];
  approvals: PendingApproval[];
  inputs: PendingUserInput[];
  items: ItemView[];
}

type OutboundMessage =
  | SyncMessage
  | { type: "delta"; itemId: string; text: string }
  | { type: "focusComposer" }
  /** Tells the composer whether the send was accepted; it only clears on `ok`. */
  | { type: "composerResult"; ok: boolean };

export class ChatView implements vscode.WebviewViewProvider {
  public static readonly viewType = "codewhale.chat";
  /** The secondary-sidebar twin. The same instance serves both ids. */
  public static readonly secondaryViewType = "codewhale.chatSecondary";

  /**
   * Which of the two view ids actually resolved. Only one is ever visible —
   * the manifest gates them on `codewhale.noSecondarySidebar` — so `reveal()`
   * must focus the one this host chose, not a hardcoded id.
   */
  private resolvedViewType: string = ChatView.viewType;

  private view?: vscode.WebviewView;
  private webviewReady = false;
  private queued: OutboundMessage[] = [];

  private connection?: ConnectionInfo;
  private threads: ThreadSummary[] = [];
  private activeThreadId?: string;
  private activeDetail?: ThreadDetail;
  private items = new Map<string, ItemView>();
  private itemOrder: string[] = [];
  private stream?: EventStream;
  private lastSeq = 0;
  private streamingTurnId?: string;
  private interruptRequested = false;
  /**
   * The in-flight send. Its `operationKey` is reused when the same prompt is
   * retried after a timeout so the runtime dedupes instead of starting a
   * second turn.
   */
  private pendingSend?: { threadId: string; prompt: string; operationKey: string };
  private reconnectAttempt = 0;
  private reconnectTimer?: ReturnType<typeof setTimeout>;
  private chips: ContextChip[] = [];

  constructor(
    private readonly extensionContext: vscode.ExtensionContext,
    private readonly configProvider: () => Promise<ApiConfig>,
    private readonly output: vscode.OutputChannel,
  ) {}

  resolveWebviewView(view: vscode.WebviewView): void {
    this.view = view;
    // Remember which id resolved so reveal() focuses the container this host
    // actually shows. viewType is readonly on WebviewView and is one of the
    // two ids registered in extension.ts.
    this.resolvedViewType = view.viewType || ChatView.viewType;
    view.webview.options = { enableScripts: true };
    view.onDidDispose(() => {
      this.closeStream();
      this.view = undefined;
      this.webviewReady = false;
    });
    view.webview.onDidReceiveMessage((message: { command?: string; [key: string]: unknown }) => {
      void this.handleWebviewMessage(message);
    });
    view.webview.html = this.renderHtml(view);
  }

  /** Show the chat sidebar and put focus in the composer. */
  async reveal(): Promise<void> {
    await vscode.commands.executeCommand(`${this.resolvedViewType}.focus`);
  }

  /** Connection updates pushed from the extension host. */
  setConnection(connection: ConnectionInfo): void {
    this.connection = connection;
    this.postSync();
  }

  async refreshThreads(): Promise<void> {
    try {
      this.threads = await listThreadSummaries(await this.configProvider());
      this.postSync();
    } catch (error) {
      this.logError("Thread summaries unavailable", error);
    }
  }

  /** "Ask Codewhale" entry: attach the current selection and focus the composer. */
  async askWithSelection(): Promise<void> {
    const chip = collectSelectionContext() ?? collectActiveFileContext();
    if (chip && !this.chips.some((existing) => existing.label === chip.label)) {
      this.chips.push(chip);
    }
    await this.reveal();
    this.post({ type: "focusComposer" });
  }

  addChip(kind: ContextChip["kind"]): void {
    const chip =
      kind === "selection"
        ? collectSelectionContext()
        : kind === "file"
          ? collectActiveFileContext()
          : collectDiagnosticsContext();
    if (!chip) {
      void vscode.window.showInformationMessage("Nothing to attach for that context kind.");
      return;
    }
    this.chips = this.chips.filter((existing) => existing.label !== chip.label);
    this.chips.push(chip);
    this.postSync();
  }

  removeChip(id: string): void {
    this.chips = this.chips.filter((chip) => chip.id !== id);
    this.postSync();
  }

  async newThread(): Promise<void> {
    try {
      const workspace = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
      const thread = await createThread(await this.configProvider(), workspace ? { workspace } : {});
      this.output.appendLine(`Created thread ${thread.id}`);
      await this.selectThread(thread.id);
      await this.refreshThreads();
    } catch (error) {
      this.logError("Create thread failed", error);
      void vscode.window.showErrorMessage(errorMessage(error));
    }
  }

  async selectThread(threadId: string): Promise<void> {
    this.closeStream();
    this.streamingTurnId = undefined;
    this.interruptRequested = false;
    this.items.clear();
    this.itemOrder = [];
    this.activeThreadId = threadId;
    this.postSync();
    try {
      const detail = await getThreadDetail(await this.configProvider(), threadId);
      if (this.activeThreadId !== threadId) {
        return; // user switched away while loading
      }
      this.activeDetail = detail;
      this.lastSeq = detail.latestSeq;
      for (const item of detail.items) {
        this.ingestItem(item);
      }
      this.openStream(threadId, detail.latestSeq).catch((error) => this.logError("Stream failed", error));
      this.postSync();
      this.scheduleThreadListRefresh();
    } catch (error) {
      this.logError("Load thread failed", error);
      void vscode.window.showErrorMessage(errorMessage(error));
    }
  }

  // ---- webview -> extension ----

  private async handleWebviewMessage(message: { command?: string; [key: string]: unknown }): Promise<void> {
    switch (message.command) {
      case "ready":
        this.webviewReady = true;
        for (const queued of this.queued.splice(0)) {
          void this.view?.webview.postMessage(queued);
        }
        this.postSync();
        break;
      case "check":
        await vscode.commands.executeCommand("codewhale.checkRuntime");
        break;
      case "start":
        await vscode.commands.executeCommand("codewhale.startRuntime");
        break;
      case "terminal":
        await vscode.commands.executeCommand("codewhale.openTerminal");
        break;
      case "setToken":
        await vscode.commands.executeCommand("codewhale.setRuntimeToken");
        break;
      case "newThread":
        await this.newThread();
        break;
      case "selectThread":
        await this.selectThread(String(message.id ?? ""));
        break;
      case "refreshThreads":
        await this.refreshThreads();
        break;
      case "sendPrompt":
        await this.sendPrompt(String(message.text ?? ""));
        break;
      case "steer":
        await this.steer(String(message.text ?? ""));
        break;
      case "interrupt":
        await this.interrupt();
        break;
      case "decideApproval":
        await this.decideApproval(
          String(message.id ?? ""),
          message.decision === "allow" ? "allow" : "deny",
          message.remember === true,
        );
        break;
      case "answerInput":
        await this.answerInput(message);
        break;
      case "addChip":
        this.addChip(message.kind === "file" ? "file" : message.kind === "diagnostics" ? "diagnostics" : "selection");
        break;
      case "removeChip":
        this.removeChip(String(message.id ?? ""));
        break;
      case "copyCode":
        await vscode.env.clipboard.writeText(String(message.code ?? ""));
        break;
      case "insertCode":
        await this.insertAtCursor(String(message.code ?? ""));
        break;
      case "openFile":
        await this.openFileAtPath(String(message.path ?? ""));
        break;
      case "openLink": {
        const url = String(message.url ?? "");
        if (/^https?:\/\//.test(url)) {
          void vscode.env.openExternal(vscode.Uri.parse(url));
        }
        break;
      }
    }
  }

  private async sendPrompt(text: string): Promise<void> {
    const prompt = text.trim();
    if (!prompt) {
      this.post({ type: "composerResult", ok: false });
      return;
    }
    if (!this.activeThreadId) {
      await this.newThread();
      if (!this.activeThreadId) {
        this.post({ type: "composerResult", ok: false });
        return;
      }
    }
    const threadId = this.activeThreadId;
    const assembled = assemblePrompt(prompt, this.chips);
    // A resend of the same pending prompt must carry the same operation key,
    // or a timeout-then-retry lands as two turns.
    const operationKey =
      this.pendingSend && this.pendingSend.threadId === threadId && this.pendingSend.prompt === assembled
        ? this.pendingSend.operationKey
        : crypto.randomUUID();
    this.pendingSend = { threadId, prompt: assembled, operationKey };
    try {
      const result = await startTurn(await this.configProvider(), threadId, {
        prompt: assembled,
        operationKey,
      });
      // Accepted: only now is it safe to drop the composer text and chips.
      this.pendingSend = undefined;
      this.chips = [];
      this.streamingTurnId = result.turn.id;
      this.interruptRequested = false;
      this.addLocalUserMessage(prompt);
      this.post({ type: "composerResult", ok: true });
      void this.openStream(threadId, this.lastSeq);
      this.postSync();
    } catch (error) {
      // Keep `pendingSend` so the retry reuses the key, and give the text back.
      this.post({ type: "composerResult", ok: false });
      this.handleError("Send failed", error);
    }
  }

  private async steer(text: string): Promise<void> {
    if (!this.activeThreadId || !this.streamingTurnId) {
      return;
    }
    try {
      await steerTurn(await this.configProvider(), this.activeThreadId, this.streamingTurnId, text.trim());
      this.addLocalUserMessage(`[steer] ${text.trim()}`);
    } catch (error) {
      this.handleError("Steer failed", error);
    }
  }

  private async interrupt(): Promise<void> {
    if (!this.activeThreadId || !this.streamingTurnId) {
      return;
    }
    try {
      await interruptTurn(await this.configProvider(), this.activeThreadId, this.streamingTurnId);
      this.output.appendLine(`Interrupt requested for turn ${this.streamingTurnId}`);
    } catch (error) {
      this.handleError("Interrupt failed", error);
    }
  }

  private async decideApproval(id: string, decision: "allow" | "deny", remember: boolean): Promise<void> {
    try {
      await resolveApproval(await this.configProvider(), id, decision, remember);
    } catch (error) {
      this.handleError("Approval failed", error);
    }
  }

  private async answerInput(message: { [key: string]: unknown }): Promise<void> {
    if (!this.activeThreadId) {
      return;
    }
    const raw = Array.isArray(message.answers) ? message.answers : [];
    const answers = raw.flatMap((entry) => {
      if (!entry || typeof entry !== "object") {
        return [];
      }
      const record = entry as Record<string, unknown>;
      const id = typeof record.id === "string" ? record.id : undefined;
      const label = typeof record.label === "string" ? record.label : undefined;
      if (!id || !label) {
        return [];
      }
      return [{ id, label, value: typeof record.value === "string" ? record.value : label }];
    });
    if (answers.length === 0) {
      return;
    }
    try {
      await answerUserInput(await this.configProvider(), this.activeThreadId, String(message.inputId ?? ""), answers);
    } catch (error) {
      this.handleError("Answer failed", error);
    }
  }

  private async insertAtCursor(code: string): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      void vscode.window.showInformationMessage("Open a file to insert code.");
      return;
    }
    await editor.edit((builder) => builder.replace(editor.selection, code));
    void vscode.window.showTextDocument(editor.document);
  }

  /**
   * Open a path that came from an item's tool metadata. That value is
   * model-influenced, so it is treated as untrusted: workspace-relative
   * resolution is preferred, `..` escapes are refused outright, and anything
   * outside the workspace needs an explicit confirmation from the user.
   */
  private async openFileAtPath(raw: string): Promise<void> {
    const candidate = raw.trim();
    if (!candidate || /[\u0000-\u001f\u007f]/.test(candidate)) {
      return;
    }
    const folders = vscode.workspace.workspaceFolders ?? [];
    const targets: vscode.Uri[] = [];

    if (path.isAbsolute(candidate)) {
      const resolved = path.resolve(candidate);
      const inWorkspace = folders.some((folder) => isInsideRoot(folder.uri.fsPath, resolved));
      if (!inWorkspace) {
        const choice = await vscode.window.showWarningMessage(
          `Open a file outside this workspace?\n\n${resolved}`,
          { modal: true },
          "Open File",
        );
        if (choice !== "Open File") {
          return;
        }
      }
      targets.push(vscode.Uri.file(resolved));
    } else {
      for (const folder of folders) {
        // joinPath keeps the folder's scheme (remote/virtual workspaces); the
        // containment check runs on the resolved filesystem path.
        if (isInsideRoot(folder.uri.fsPath, candidate)) {
          targets.push(vscode.Uri.joinPath(folder.uri, candidate));
        }
      }
      if (targets.length === 0) {
        void vscode.window.showWarningMessage(`Refused to open a path outside the workspace: ${candidate}`);
        return;
      }
    }

    for (const uri of targets) {
      try {
        await vscode.workspace.fs.stat(uri);
        await vscode.window.showTextDocument(uri, { preview: true });
        return;
      } catch {
        // try the next candidate
      }
    }
    void vscode.window.showInformationMessage(`File not found: ${candidate}`);
  }

  // ---- SSE event ingestion ----

  private async openStream(threadId: string, sinceSeq: number): Promise<void> {
    this.closeStream();
    this.reconnectAttempt = 0;
    const config = await this.configProvider();
    if (this.activeThreadId !== threadId) {
      return;
    }
    const stream = openEventStream(config, threadId, sinceSeq, new SseParser());
    stream.onEvent = (event) => this.handleStreamEvent(event);
    stream.onError = (error) => this.handleStreamError(threadId, error);
    this.stream = stream;
  }

  private closeStream(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = undefined;
    }
    this.stream?.close();
    this.stream = undefined;
  }

  private handleStreamEvent(event: RuntimeEvent): void {
    if (event.seq <= this.lastSeq) {
      return; // duplicate or stale replay
    }
    this.lastSeq = event.seq;
    this.reconnectAttempt = 0;

    switch (event.event) {
      case "item.started":
      case "item.completed":
      case "item.failed":
      case "item.interrupted":
      case "item.canceled": {
        const payloadItem = readPayloadItem(event.payload);
        const itemId = event.itemId ?? payloadItem?.id;
        if (!itemId) {
          return;
        }
        const existing = this.items.get(itemId);
        const merged: ItemRecord = {
          id: itemId,
          turnId: event.turnId ?? existing?.turnId,
          kind: payloadItem?.kind ?? existing?.kind ?? "status",
          status: payloadItem?.status ?? statusForEvent(event.event),
          summary: payloadItem?.summary ?? existing?.summary ?? "",
          detail: payloadItem?.detail ?? existing?.detail,
          metadata: payloadItem?.metadata ?? existing?.metadata,
        };
        this.ingestItem(merged, event.event);
        this.postSync();
        break;
      }
      case "item.delta": {
        const delta = readPayloadDelta(event.payload);
        if (!delta || !event.itemId) {
          return;
        }
        const view = this.items.get(event.itemId);
        if (view) {
          view.streamText = (view.streamText ?? "") + delta;
          view.rev += 1;
        } else {
          this.ingestItem({
            id: event.itemId,
            turnId: event.turnId,
            kind: readPayloadKind(event.payload) ?? "agent_message",
            summary: delta,
          });
        }
        this.post({ type: "delta", itemId: event.itemId, text: delta });
        break;
      }
      case "approval.required": {
        const approval = readPayloadApproval(event.payload);
        if (approval && this.activeDetail) {
          this.activeDetail.pendingApprovals = [
            ...this.activeDetail.pendingApprovals.filter((entry) => entry.id !== approval.id),
            approval,
          ];
          this.postSync();
        }
        break;
      }
      case "approval.decided":
      case "approval.timeout": {
        const id = readPayloadId(event.payload) ?? event.itemId;
        if (id && this.activeDetail) {
          this.activeDetail.pendingApprovals = this.activeDetail.pendingApprovals.filter(
            (entry) => entry.id !== id,
          );
          this.postSync();
        }
        break;
      }
      case "user_input.required": {
        const input = readPayloadUserInput(event.payload);
        if (input && this.activeDetail) {
          this.activeDetail.pendingUserInputs = [
            ...this.activeDetail.pendingUserInputs.filter((entry) => entry.id !== input.id),
            input,
          ];
          this.postSync();
        }
        break;
      }
      case "user_input.answered":
      case "user_input.canceled": {
        const id = readPayloadId(event.payload);
        if (id && this.activeDetail) {
          this.activeDetail.pendingUserInputs = this.activeDetail.pendingUserInputs.filter(
            (entry) => entry.id !== id,
          );
          this.postSync();
        }
        break;
      }
      case "turn.interrupt_requested": {
        // The turn is still running until it reports an end state; keep the
        // Stop/Steer controls on screen instead of hiding them here.
        if (event.turnId && event.turnId === this.streamingTurnId) {
          this.interruptRequested = true;
          this.postSync();
        }
        break;
      }
      case "turn.completed":
      case "turn.failed":
      case "turn.interrupted":
      case "turn.ended": {
        if (event.turnId && event.turnId === this.streamingTurnId) {
          this.streamingTurnId = undefined;
          this.interruptRequested = false;
          this.postSync();
          this.scheduleThreadListRefresh();
        }
        break;
      }
      default:
        break;
    }
  }

  private handleStreamError(threadId: string, error: Error): void {
    if (this.activeThreadId !== threadId) {
      return;
    }
    if (error instanceof Error && "statusCode" in error && (error as { statusCode?: number }).statusCode === 401) {
      this.connection = { kind: "auth-required", detail: "Runtime token was rejected." };
      this.postSync();
      return;
    }
    // The stream is dead; drop it, or the `!this.stream` guard below never
    // passes and the reconnect silently never happens.
    this.stream?.close();
    this.stream = undefined;
    const attempt = ++this.reconnectAttempt;
    const delay = Math.min(1000 * attempt, 5000);
    this.output.appendLine(`Event stream for ${threadId} dropped (${error.message}); retrying in ${delay}ms`);
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = undefined;
      if (this.activeThreadId === threadId && !this.stream) {
        void this.openStream(threadId, this.lastSeq).catch((error) =>
          this.logError("Stream reconnect failed", error),
        );
      }
    }, delay);
  }

  /** Fill the item projection from a thread-detail snapshot or SSE event. */
  private ingestItem(item: ItemRecord, event?: string): void {
    const existing = this.items.get(item.id);
    this.items.set(item.id, projectItem(item, existing, event));
    if (!existing) {
      this.itemOrder.push(item.id);
    }
  }

  private addLocalUserMessage(text: string): void {
    const id = `local-${crypto.randomUUID()}`;
    this.items.set(id, { id, kind: "user_message", summary: text, rev: 1 });
    this.itemOrder.push(id);
    this.postSync();
  }

  private scheduleThreadListRefresh(): void {
    // Titles/previews settle right after a turn finishes; refresh lazily.
    setTimeout(() => {
      void this.refreshThreads();
    }, 1200);
  }

  // ---- outbound ----

  private post(message: OutboundMessage): void {
    if (!this.view) {
      return;
    }
    if (!this.webviewReady) {
      this.queued.push(message);
      return;
    }
    void this.view.webview.postMessage(message);
  }

  private postSync(): void {
    this.post({
      type: "sync",
      connection: this.connection,
      threads: this.threads,
      activeThreadId: this.activeThreadId,
      model: this.activeDetail?.thread.model ?? this.threads.find((t) => t.id === this.activeThreadId)?.model,
      streaming: this.streamingTurnId !== undefined,
      interrupting: this.interruptRequested,
      chips: this.chips,
      approvals: this.activeDetail?.pendingApprovals ?? [],
      inputs: this.activeDetail?.pendingUserInputs ?? [],
      items: this.itemOrder.flatMap((id) => {
        const view = this.items.get(id);
        return view ? [view] : [];
      }),
    });
  }

  private handleError(where: string, error: unknown): void {
    this.logError(where, error);
    if (isAuthError(error)) {
      this.connection = { kind: "auth-required", detail: "Runtime token was rejected." };
      this.postSync();
    }
    void vscode.window.showErrorMessage(`CodeWhale ${where.toLowerCase()}: ${errorMessage(error)}`);
  }

  private logError(where: string, error: unknown): void {
    this.output.appendLine(`${new Date().toISOString()} ${where}: ${errorMessage(error)}`);
  }

  /** Load the thread list + latest detail for the active thread (used after reconnects). */
  async resyncAfterConnection(): Promise<void> {
    await this.refreshThreads();
    if (this.activeThreadId) {
      await this.selectThread(this.activeThreadId);
    }
  }

  // ---- webview HTML ----

  private renderHtml(view: vscode.WebviewView): string {
    const nonce = makeNonce();
    return `<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}';">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <style>
    ${chatStyles()}
  </style>
</head>
<body>
  <header id="conn">
    <span id="conn-dot" class="dot offline" aria-hidden="true"></span>
    <span id="conn-label" role="status" aria-live="polite">Checking runtime…</span>
    <span class="spacer"></span>
    <button id="btn-new" class="icon" title="New thread" aria-label="New thread">＋ New</button>
  </header>
  <div id="conn-actions" class="hidden" role="group" aria-label="Runtime connection actions">
    <button id="btn-start">Start Local Runtime</button>
    <button id="btn-token">Set Runtime Token</button>
    <button id="btn-terminal">Open Terminal</button>
  </div>
  <details id="threads-box">
    <summary>Threads <span id="threads-count"></span></summary>
    <div id="threads" role="group" aria-label="Threads"></div>
  </details>
  <main id="transcript" role="log" aria-live="polite" aria-relevant="additions text"
        aria-label="Conversation transcript" tabindex="0"></main>
  <div id="attention" role="region" aria-live="assertive" aria-label="Needs your attention"></div>
  <div id="chips" role="list" aria-label="Attached context"></div>
  <div id="steer-box" class="hidden" role="group" aria-label="Running turn controls">
    <label class="sr-only" for="steer-input">Steer the running turn</label>
    <input id="steer-input" type="text" placeholder="Steer the running turn…"
           aria-label="Steer the running turn" />
    <button id="btn-steer">Steer</button>
    <button id="btn-interrupt" class="danger">Stop</button>
  </div>
  <footer id="composer">
    <div class="attach-row" role="group" aria-label="Attach editor context">
      <button id="btn-chip-selection" title="Attach current selection" aria-label="Attach current selection">＋ Selection</button>
      <button id="btn-chip-file" title="Attach active file" aria-label="Attach active file">＋ File</button>
      <button id="btn-chip-diagnostics" title="Attach problems" aria-label="Attach problems from the active file">＋ Problems</button>
      <span id="model-label" class="model" role="status" aria-live="polite"></span>
    </div>
    <label class="sr-only" for="prompt">Message Codewhale</label>
    <textarea id="prompt" rows="3" aria-label="Message Codewhale"
              aria-describedby="prompt-hint"
              placeholder="Ask Codewhale… (Enter to send, Shift+Enter for newline)"></textarea>
    <span id="prompt-hint" class="sr-only">Press Enter to send, Shift plus Enter for a new line.</span>
  </footer>
  <script nonce="${nonce}">
    ${chatScript()}
  </script>
</body>
</html>`;
  }
}

function readPayloadItem(payload: unknown): Partial<ItemRecord> | undefined {
  if (!payload || typeof payload !== "object") {
    return undefined;
  }
  const record = payload as Record<string, unknown>;
  const source =
    record.item && typeof record.item === "object" ? (record.item as Record<string, unknown>) : record;
  const summary = typeof source.summary === "string" ? source.summary : undefined;
  return {
    id: typeof source.id === "string" ? source.id : undefined,
    kind: typeof source.kind === "string" ? source.kind : undefined,
    status: typeof source.status === "string" ? source.status : undefined,
    summary,
    detail: typeof source.detail === "string" ? source.detail : undefined,
    metadata:
      source.metadata && typeof source.metadata === "object"
        ? (source.metadata as Record<string, unknown>)
        : undefined,
  };
}

function readPayloadDelta(payload: unknown): string | undefined {
  if (!payload || typeof payload !== "object") {
    return undefined;
  }
  const delta = (payload as Record<string, unknown>).delta;
  return typeof delta === "string" ? delta : undefined;
}

function readPayloadKind(payload: unknown): string | undefined {
  if (!payload || typeof payload !== "object") {
    return undefined;
  }
  const kind = (payload as Record<string, unknown>).kind;
  return typeof kind === "string" ? kind : undefined;
}

function readPayloadId(payload: unknown): string | undefined {
  if (!payload || typeof payload !== "object") {
    return undefined;
  }
  const record = payload as Record<string, unknown>;
  for (const key of ["approval_id", "input_id", "id"]) {
    const value = record[key];
    if (typeof value === "string") {
      return value;
    }
  }
  return undefined;
}

function readPayloadApproval(payload: unknown): PendingApproval | undefined {
  if (!payload || typeof payload !== "object") {
    return undefined;
  }
  const record = payload as Record<string, unknown>;
  const id = readPayloadId(record);
  if (!id) {
    return undefined;
  }
  return {
    id,
    turnId: typeof record.turn_id === "string" ? record.turn_id : undefined,
    toolName: typeof record.tool_name === "string" ? record.tool_name : "tool",
    description: typeof record.description === "string" ? record.description : "",
    intentSummary: typeof record.intent_summary === "string" ? record.intent_summary : undefined,
  };
}

function readPayloadUserInput(payload: unknown): PendingUserInput | undefined {
  if (!payload || typeof payload !== "object") {
    return undefined;
  }
  const record = payload as Record<string, unknown>;
  const id = readPayloadId(record);
  if (!id) {
    return undefined;
  }
  const request =
    record.request && typeof record.request === "object"
      ? (record.request as Record<string, unknown>)
      : record;
  const questions = Array.isArray(request.questions)
    ? request.questions.flatMap((raw) => {
        if (!raw || typeof raw !== "object") {
          return [];
        }
        const question = raw as Record<string, unknown>;
        const questionId = typeof question.id === "string" ? question.id : undefined;
        if (!questionId) {
          return [];
        }
        return [
          {
            id: questionId,
            header: typeof question.header === "string" ? question.header : undefined,
            question: typeof question.question === "string" ? question.question : "",
            allowFreeText: question.allow_free_text === true,
            multiSelect: question.multi_select === true,
            options: Array.isArray(question.options)
              ? question.options.flatMap((optionRaw) => {
                  if (!optionRaw || typeof optionRaw !== "object") {
                    return [];
                  }
                  const option = optionRaw as Record<string, unknown>;
                  return typeof option.label === "string"
                    ? [
                        {
                          label: option.label,
                          description:
                            typeof option.description === "string" ? option.description : undefined,
                        },
                      ]
                    : [];
                })
              : [],
          },
        ];
      })
    : [];
  return { id, turnId: typeof record.turn_id === "string" ? record.turn_id : undefined, questions };
}

function isAuthError(error: unknown): boolean {
  return error instanceof Error && "statusCode" in error && (error as { statusCode?: number }).statusCode === 401;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function makeNonce(): string {
  return crypto.randomBytes(16).toString("hex");
}

function chatStyles(): string {
  return `
    body { display: flex; flex-direction: column; height: 100vh; margin: 0; padding: 0;
           color: var(--vscode-foreground); font-family: var(--vscode-font-family); font-size: var(--vscode-font-size, 13px); }
    button { font-family: inherit; font-size: 11px; cursor: pointer; color: var(--vscode-button-foreground);
             background: var(--vscode-button-secondaryBackground); border: none; border-radius: 3px; padding: 3px 8px; }
    button.primary { background: var(--vscode-button-background); }
    button.danger { background: var(--vscode-errorForeground); color: var(--vscode-editor-background); }
    button:hover { filter: brightness(1.1); }
    button[aria-disabled="true"] { opacity: 0.6; cursor: default; }
    /* Keyboard focus must be visible everywhere it can land. */
    :focus-visible { outline: 2px solid var(--vscode-focusBorder, #0078d4); outline-offset: 1px; border-radius: 3px; }
    button:focus-visible, summary:focus-visible, [tabindex]:focus-visible, .thread:focus-visible, .chip button:focus-visible {
      outline: 2px solid var(--vscode-focusBorder, #0078d4); outline-offset: 1px; }
    input[type="text"]:focus-visible, input[type="checkbox"]:focus-visible, textarea:focus-visible {
      outline: 2px solid var(--vscode-focusBorder, #0078d4); outline-offset: -1px;
      border-color: var(--vscode-focusBorder, #0078d4); }
    /* Some hosts still report only :focus for the textarea; keep it obvious. */
    textarea:focus, input[type="text"]:focus { border-color: var(--vscode-focusBorder, #0078d4); }
    .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden;
               clip: rect(0 0 0 0); clip-path: inset(50%); white-space: nowrap; border: 0; }
    input[type="text"], textarea { width: 100%; box-sizing: border-box; color: var(--vscode-input-foreground);
      background: var(--vscode-input-background); border: 1px solid var(--vscode-input-border, transparent); border-radius: 3px;
      padding: 6px 8px; font-family: inherit; font-size: inherit; resize: vertical; }
    header { display: flex; align-items: center; gap: 6px; padding: 8px 10px; }
    .dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; }
    .dot.connected { background: var(--vscode-testing-iconPassed, #2ea043); }
    .dot.offline { background: var(--vscode-testing-iconFailed, #f14c4c); }
    .dot.auth-required { background: var(--vscode-editorWarning-foreground, #cca700); }
    .dot.error { background: var(--vscode-testing-iconFailed, #f14c4c); }
    .spacer { flex: 1; }
    .hidden { display: none !important; }
    #conn-label { color: var(--vscode-descriptionForeground); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    #conn-actions { display: flex; gap: 6px; padding: 0 10px 8px; flex-wrap: wrap; }
    details { border-top: 1px solid var(--vscode-panel-border, #333); }
    #threads-box { padding: 0 10px; }
    #threads-box summary { cursor: pointer; padding: 6px 0; color: var(--vscode-descriptionForeground); font-size: 11px; text-transform: uppercase; letter-spacing: 0.04em; }
    .thread { padding: 5px 6px; border-radius: 4px; cursor: pointer; overflow: hidden; }
    .thread:hover { background: var(--vscode-list-hoverBackground); }
    .thread.active { background: var(--vscode-list-activeSelectionBackground); color: var(--vscode-list-activeSelectionForeground); }
    .thread-title { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .thread-meta { color: var(--vscode-descriptionForeground); font-size: 10px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    main { flex: 1; overflow-y: auto; padding: 4px 10px; }
    .msg { margin: 8px 0; }
    .msg .who { font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em;
                color: var(--vscode-descriptionForeground); margin-bottom: 2px; }
    .msg.user .who { color: var(--vscode-textLink-foreground); }
    .bubble { border-radius: 6px; padding: 6px 9px; line-height: 1.5; overflow-wrap: anywhere; white-space: pre-wrap; }
    .msg.user .bubble { background: var(--vscode-input-background); border: 1px solid var(--vscode-panel-border, #333); }
    .msg.agent .bubble { white-space: normal; }
    .msg.agent .bubble p { margin: 0 0 8px; }
    .msg.agent .bubble p:last-child { margin-bottom: 0; }
    .msg.agent .bubble h3, .msg.agent .bubble h4, .msg.agent .bubble h5 { margin: 10px 0 4px; }
    .msg.agent .bubble ul, .msg.agent .bubble ol { margin: 4px 0; padding-left: 20px; }
    .msg.agent .bubble hr { border: none; border-top: 1px solid var(--vscode-panel-border, #333); }
    .msg.agent .bubble a { color: var(--vscode-textLink-foreground); }
    .streaming::after { content: "▍"; animation: blink 1s steps(2) infinite; color: var(--vscode-descriptionForeground); }
    @keyframes blink { 50% { opacity: 0; } }
    .tool { margin: 6px 0; border: 1px solid var(--vscode-panel-border, #333); border-radius: 5px; font-size: 12px; }
    .tool summary { cursor: pointer; padding: 5px 8px; color: var(--vscode-descriptionForeground); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .tool pre { margin: 0; padding: 6px 8px; overflow-x: auto; font-family: var(--vscode-editor-font-family, monospace); font-size: 11px; white-space: pre-wrap; }
    .msg.error .bubble { color: var(--vscode-errorForeground); }
    .msg.status .bubble { color: var(--vscode-descriptionForeground); font-size: 11px; }
    .codeblock { margin: 8px 0; border: 1px solid var(--vscode-panel-border, #333); border-radius: 5px; overflow: hidden; }
    .codeblock-bar { display: flex; justify-content: space-between; align-items: center; padding: 2px 4px 2px 8px;
                     background: var(--vscode-titleBar-activeBackground, #222); }
    .codeblock-lang { font-size: 10px; color: var(--vscode-descriptionForeground); text-transform: uppercase; }
    .codeblock-actions button { margin-left: 4px; padding: 1px 6px; font-size: 10px; }
    .codeblock pre { margin: 0; padding: 8px; overflow-x: auto; font-family: var(--vscode-editor-font-family, monospace); font-size: 11px; }
    .card { margin: 8px 0; border: 1px solid var(--vscode-editorWarning-foreground, #cca700); border-radius: 6px; padding: 8px; }
    .card .title { font-weight: 700; margin-bottom: 4px; }
    .card .desc { color: var(--vscode-descriptionForeground); margin-bottom: 8px; overflow-wrap: anywhere; white-space: pre-wrap; }
    .card .row { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; margin-top: 6px; }
    .card label { font-size: 11px; color: var(--vscode-descriptionForeground); display: flex; gap: 4px; align-items: center; }
    .opt { display: block; width: 100%; text-align: left; margin: 3px 0; padding: 5px 8px; }
    .opt .opt-desc { display: block; font-weight: 400; color: var(--vscode-descriptionForeground); font-size: 10px; }
    #chips { display: flex; gap: 4px; flex-wrap: wrap; padding: 0 10px 4px; }
    .chip { display: inline-flex; gap: 4px; align-items: center; background: var(--vscode-badge-background);
            color: var(--vscode-badge-foreground); border-radius: 8px; padding: 1px 8px; font-size: 10px; }
    .chip button { background: transparent; color: inherit; padding: 0 2px; font-size: 11px; }
    #steer-box { display: flex; gap: 6px; padding: 4px 10px; }
    #steer-box input { flex: 1; }
    #composer { border-top: 1px solid var(--vscode-panel-border, #333); padding: 6px 10px 10px; }
    .attach-row { display: flex; gap: 4px; margin-bottom: 4px; align-items: center; }
    .attach-row button { font-size: 10px; padding: 1px 6px; }
    .model { margin-left: auto; color: var(--vscode-descriptionForeground); font-size: 10px; }
    main:focus-visible { outline: 2px solid var(--vscode-focusBorder, #0078d4); outline-offset: -2px; }
    #transcript .empty { color: var(--vscode-descriptionForeground); text-align: center; margin-top: 30px; line-height: 1.6; }
  `;
}

/**
 * The webview script as a string. Kept here (not in a separate file) so the
 * extension stays a no-bundler build; it must never interpolate runtime data.
 */
function chatScript(): string {
  return `
    const vscode = acquireVsCodeApi();
    const transcript = document.getElementById("transcript");
    const attention = document.getElementById("attention");
    const chipsRow = document.getElementById("chips");
    const threadsList = document.getElementById("threads");
    const itemEls = new Map();      // item id -> element
    const codeBlocks = new Map();   // item id -> [raw code]
    const streamBufs = new Map();   // item id -> streaming text element
    let state = { streaming: false, activeThreadId: undefined };

    document.getElementById("btn-new").addEventListener("click", () => vscode.postMessage({ command: "newThread" }));
    document.getElementById("btn-start").addEventListener("click", () => vscode.postMessage({ command: "start" }));
    document.getElementById("btn-token").addEventListener("click", () => vscode.postMessage({ command: "setToken" }));
    document.getElementById("btn-terminal").addEventListener("click", () => vscode.postMessage({ command: "terminal" }));
    document.getElementById("btn-chip-selection").addEventListener("click", () => vscode.postMessage({ command: "addChip", kind: "selection" }));
    document.getElementById("btn-chip-file").addEventListener("click", () => vscode.postMessage({ command: "addChip", kind: "file" }));
    document.getElementById("btn-chip-diagnostics").addEventListener("click", () => vscode.postMessage({ command: "addChip", kind: "diagnostics" }));
    document.getElementById("btn-interrupt").addEventListener("click", (e) => {
      if (e.currentTarget.getAttribute("aria-disabled") === "true") { return; }
      vscode.postMessage({ command: "interrupt" });
    });
    document.getElementById("btn-steer").addEventListener("click", sendSteer);
    document.getElementById("steer-input").addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); sendSteer(); } });

    const promptBox = document.getElementById("prompt");
    promptBox.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        sendPrompt();
      }
    });

    let sending = false;
    let sentText = "";
    function sendPrompt() {
      const text = promptBox.value.trim();
      if (!text || sending) { return; }
      sentText = promptBox.value;
      // Do NOT clear here: the turn can still be refused and the text is the
      // user's. It is cleared only when the extension confirms acceptance.
      sending = true;
      promptBox.setAttribute("aria-busy", "true");
      vscode.postMessage({ command: "sendPrompt", text });
    }

    function composerResult(ok) {
      sending = false;
      promptBox.removeAttribute("aria-busy");
      // Only clear what was actually sent: the user may have kept typing.
      if (ok && promptBox.value === sentText) { promptBox.value = ""; }
      sentText = "";
      promptBox.focus();
    }
    function sendSteer() {
      const box = document.getElementById("steer-input");
      const text = box.value.trim();
      if (!text) { return; }
      box.value = "";
      vscode.postMessage({ command: "steer", text });
    }

    window.addEventListener("message", (event) => {
      const msg = event.data;
      if (msg.type === "sync") { renderSync(msg); }
      else if (msg.type === "delta") { appendDelta(msg.itemId, msg.text); }
      else if (msg.type === "focusComposer") { promptBox.focus(); }
      else if (msg.type === "composerResult") { composerResult(msg.ok === true); }
    });
    vscode.postMessage({ command: "ready" });

    function renderSync(msg) {
      state = msg;
      renderConnection(msg);
      renderThreads(msg);
      renderChips(msg.chips || []);
      renderAttention(msg);
      renderItems(msg.items || []);
      const running = !!msg.streaming;
      document.getElementById("steer-box").classList.toggle("hidden", !running);
      const stop = document.getElementById("btn-interrupt");
      stop.classList.toggle("hidden", !running);
      stop.textContent = msg.interrupting ? "Stopping…" : "Stop";
      stop.setAttribute("aria-label", msg.interrupting ? "Stopping the current turn" : "Stop the current turn");
      stop.setAttribute("aria-disabled", msg.interrupting ? "true" : "false");
      document.getElementById("model-label").textContent = msg.model ? msg.model : "";
    }

    function renderConnection(msg) {
      const conn = msg.connection;
      const dot = document.getElementById("conn-dot");
      const label = document.getElementById("conn-label");
      const actions = document.getElementById("conn-actions");
      if (!conn) { dot.className = "dot offline"; label.textContent = "Checking runtime…"; actions.classList.add("hidden"); return; }
      dot.className = "dot " + conn.kind;
      label.textContent = conn.detail;
      label.title = conn.detail;
      const showActions = conn.kind !== "connected";
      actions.classList.toggle("hidden", !showActions);
      document.getElementById("btn-token").classList.toggle("hidden", conn.kind !== "auth-required");
    }

    function renderThreads(msg) {
      const threads = msg.threads || [];
      document.getElementById("threads-count").textContent = "(" + threads.length + ")";
      threadsList.textContent = "";
      for (const thread of threads) {
        const el = document.createElement("div");
        el.className = "thread" + (thread.id === msg.activeThreadId ? " active" : "");
        el.setAttribute("role", "button");
        el.tabIndex = 0;
        if (thread.id === msg.activeThreadId) { el.setAttribute("aria-current", "true"); }
        const title = document.createElement("div");
        title.className = "thread-title";
        title.textContent = thread.title || "New Thread";
        const meta = document.createElement("div");
        meta.className = "thread-meta";
        meta.textContent = [thread.model, thread.branch, thread.latestTurnStatus].filter(Boolean).join(" · ");
        el.appendChild(title);
        el.appendChild(meta);
        const open = () => vscode.postMessage({ command: "selectThread", id: thread.id });
        el.setAttribute("aria-label", "Thread: " + (thread.title || "New Thread"));
        el.addEventListener("click", open);
        el.addEventListener("keydown", (e) => {
          if (e.key === "Enter" || e.key === " ") { e.preventDefault(); open(); }
        });
        threadsList.appendChild(el);
      }
    }

    function renderChips(chips) {
      chipsRow.textContent = "";
      for (const chip of chips) {
        const el = document.createElement("span");
        el.className = "chip";
        el.setAttribute("role", "listitem");
        const text = document.createElement("span");
        text.textContent = chip.label + (chip.detail ? " (" + chip.detail + ")" : "");
        const remove = document.createElement("button");
        remove.textContent = "×";
        remove.title = "Remove";
        remove.setAttribute("aria-label", "Remove context " + chip.label);
        remove.addEventListener("click", () => vscode.postMessage({ command: "removeChip", id: chip.id }));
        el.appendChild(text);
        el.appendChild(remove);
        chipsRow.appendChild(el);
      }
    }

    function renderAttention(msg) {
      attention.textContent = "";
      for (const approval of msg.approvals || []) {
        attention.appendChild(approvalCard(approval));
      }
      for (const input of msg.inputs || []) {
        attention.appendChild(userInputCard(msg.activeThreadId, input));
      }
    }

    function approvalCard(approval) {
      const card = document.createElement("div");
      card.className = "card";
      card.setAttribute("role", "group");
      card.setAttribute("aria-label", "Approval request for " + approval.toolName);
      const title = document.createElement("div");
      title.className = "title";
      title.textContent = "Approval: " + approval.toolName;
      const desc = document.createElement("div");
      desc.className = "desc";
      desc.textContent = approval.intentSummary ? approval.intentSummary + "\\n" + approval.description : approval.description;
      const row = document.createElement("div");
      row.className = "row";
      const remember = document.createElement("label");
      const rememberBox = document.createElement("input");
      rememberBox.type = "checkbox";
      remember.appendChild(rememberBox);
      remember.appendChild(document.createTextNode(" remember"));
      const allow = document.createElement("button");
      allow.className = "primary";
      allow.textContent = "Allow";
      allow.setAttribute("aria-label", "Allow " + approval.toolName);
      allow.addEventListener("click", () => vscode.postMessage({ command: "decideApproval", id: approval.id, decision: "allow", remember: rememberBox.checked }));
      const deny = document.createElement("button");
      deny.textContent = "Deny";
      deny.setAttribute("aria-label", "Deny " + approval.toolName);
      deny.addEventListener("click", () => vscode.postMessage({ command: "decideApproval", id: approval.id, decision: "deny", remember: rememberBox.checked }));
      row.appendChild(allow); row.appendChild(deny); row.appendChild(remember);
      card.appendChild(title); card.appendChild(desc); card.appendChild(row);
      return card;
    }

    function userInputCard(threadId, input) {
      const card = document.createElement("div");
      card.className = "card";
      card.setAttribute("role", "group");
      card.setAttribute("aria-label", "Codewhale needs an answer");
      for (const question of input.questions || []) {
        const title = document.createElement("div");
        title.className = "title";
        title.textContent = (question.header ? question.header + ": " : "") + question.question;
        card.appendChild(title);
        const selected = new Set();
        const answer = (label) => vscode.postMessage({
          command: "answerInput", inputId: input.id,
          answers: [{ id: question.id, label: label, value: label }],
        });
        for (const option of question.options || []) {
          const btn = document.createElement("button");
          btn.className = "opt";
          btn.textContent = option.label;
          if (option.description) {
            const desc = document.createElement("span");
            desc.className = "opt-desc";
            desc.textContent = option.description;
            btn.appendChild(desc);
          }
          if (question.multiSelect) {
            btn.setAttribute("role", "checkbox");
            btn.setAttribute("aria-checked", "false");
            btn.addEventListener("click", () => {
              const on = !selected.has(option.label);
              if (on) { selected.add(option.label); btn.style.opacity = "0.6"; }
              else { selected.delete(option.label); btn.style.opacity = ""; }
              btn.setAttribute("aria-checked", on ? "true" : "false");
            });
          } else {
            btn.addEventListener("click", () => answer(option.label));
          }
          card.appendChild(btn);
        }
        if (question.multiSelect && (question.options || []).length > 0) {
          const confirm = document.createElement("button");
          confirm.className = "primary";
          confirm.textContent = "Confirm";
          confirm.addEventListener("click", () => vscode.postMessage({
            command: "answerInput", inputId: input.id,
            answers: Array.from(selected).map((label) => ({ id: question.id, label: label, value: label })),
          }));
          card.appendChild(confirm);
        }
        if (question.allowFreeText) {
          const free = document.createElement("div");
          free.className = "row";
          const box = document.createElement("input");
          box.type = "text";
          box.placeholder = "Other…";
          box.setAttribute("aria-label", "Other answer for: " + question.question);
          const send = document.createElement("button");
          send.textContent = "Send";
          send.addEventListener("click", () => { if (box.value.trim()) { answer(box.value.trim()); } });
          free.appendChild(box); free.appendChild(send);
          card.appendChild(free);
        }
      }
      return card;
    }

    function renderItems(items) {
      const seen = new Set();
      for (const view of items) {
        seen.add(view.id);
        const existing = itemEls.get(view.id);
        if (!existing) {
          itemEls.set(view.id, renderItem(view));
        } else if (existing.dataset.rev !== String(view.rev)) {
          const fresh = renderItem(view);
          existing.replaceWith(fresh);
          itemEls.set(view.id, fresh);
        }
      }
      for (const [id, el] of Array.from(itemEls)) {
        if (!seen.has(id)) { el.remove(); itemEls.delete(id); streamBufs.delete(id); codeBlocks.delete(id); }
      }
      orderTranscript(items);
      trimEmpty();
      scrollToBottom();
    }

    function orderTranscript(items) {
      let cursor = transcript.firstChild;
      for (const view of items) {
        const el = itemEls.get(view.id);
        if (!el) { continue; }
        if (cursor === el) { cursor = el.nextSibling; continue; }
        transcript.insertBefore(el, cursor);
      }
    }

    function renderItem(view) {
      if (view.kind === "user_message") {
        return wrap("user", "You", plainBubble(view.summary));
      }
      if (view.kind === "agent_message") {
        if (view.html !== undefined) {
          streamBufs.delete(view.id);
          const bubble = document.createElement("div");
          bubble.className = "bubble";
          bubble.innerHTML = view.html;
          if (view.codeBlocks) { codeBlocks.set(view.id, view.codeBlocks); wireCodeButtons(bubble, view.id); }
          return wrap("agent", "Codewhale", bubble, view);
        }
        const bubble = document.createElement("div");
        bubble.className = "bubble streaming";
        bubble.setAttribute("aria-busy", "true");
        bubble.textContent = view.streamText || "";
        streamBufs.set(view.id, bubble);
        return wrap("agent", "Codewhale", bubble, view);
      }
      if (view.kind === "tool_call" || view.kind === "command_execution" || view.kind === "file_change") {
        return wrap(view.kind, "", toolDetails(view), view);
      }
      if (view.kind === "error") {
        return wrap("error", "Error", plainBubble(view.summary + (view.detail ? "\\n" + view.detail : "")), view);
      }
      return wrap("status", "", plainBubble(view.summary), view);
    }

    function wrap(kind, who, body, view) {
      const msg = document.createElement("div");
      msg.className = "msg " + kind;
      msg.setAttribute("role", "group");
      msg.setAttribute("aria-label", who || kind.replace(/_/g, " "));
      if (view) { msg.dataset.rev = String(view.rev); }
      if (who) {
        const whoEl = document.createElement("div");
        whoEl.className = "who";
        whoEl.textContent = who;
        msg.appendChild(whoEl);
      }
      msg.appendChild(body);
      return msg;
    }

    function plainBubble(text) {
      const bubble = document.createElement("div");
      bubble.className = "bubble";
      bubble.textContent = text;
      return bubble;
    }

    function toolDetails(view) {
      const details = document.createElement("details");
      details.className = "tool";
      const summary = document.createElement("summary");
      summary.textContent = toolLabel(view);
      details.appendChild(summary);
      const body = document.createElement("pre");
      body.textContent = view.detail || view.summary || "";
      details.appendChild(body);
      // view.filePath is parsed extension-side (metadata.tool_input included)
      // and validated again before anything is opened.
      if (view.filePath) {
        const open = document.createElement("button");
        open.textContent = "Open file";
        open.setAttribute("aria-label", "Open file " + view.filePath);
        open.title = view.filePath;
        open.style.margin = "4px 8px";
        open.addEventListener("click", () => vscode.postMessage({ command: "openFile", path: view.filePath }));
        details.appendChild(open);
      }
      return details;
    }

    function toolLabel(view) {
      const icon = view.kind === "file_change" ? "✎ " : view.kind === "command_execution" ? "▶ " : "🔧 ";
      return icon + (view.summary || view.kind);
    }

    function wireCodeButtons(scope, itemId) {
      for (const btn of scope.querySelectorAll("button.cb-copy, button.cb-insert")) {
        const slot = Number(btn.dataset.cb);
        btn.addEventListener("click", () => {
          const blocks = codeBlocks.get(itemId) || [];
          const code = blocks[slot] || "";
          vscode.postMessage({ command: btn.classList.contains("cb-copy") ? "copyCode" : "insertCode", code: code });
        });
      }
      for (const link of scope.querySelectorAll("a[href]")) {
        link.addEventListener("click", (e) => {
          e.preventDefault();
          vscode.postMessage({ command: "openLink", url: link.getAttribute("href") });
        });
      }
    }

    function appendDelta(itemId, text) {
      let bubble = streamBufs.get(itemId);
      if (!bubble) { return; }
      bubble.textContent += text;
      scrollToBottom();
    }

    function trimEmpty() {
      const empty = transcript.querySelector(".empty");
      if (empty && transcript.children.length > 1) { empty.remove(); }
    }

    function scrollToBottom() {
      transcript.scrollTop = transcript.scrollHeight;
    }

    function ensureEmpty() {
      if (transcript.children.length === 0) {
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.textContent = "Start a task: attach context below and ask Codewhale. The same thread stays available in the terminal.";
        transcript.appendChild(empty);
      }
    }
    ensureEmpty();
  `;
}
