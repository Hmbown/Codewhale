/** Read-only Codewhale session/runtime adapter. Does not record, mutate, or own receipts. */
import { CATEGORIES, errorOnsetOf, type Category, type Status, type Trace, type WhaleEvent } from './model.js';

const PAYLOAD_LIMIT = 2000;
const COLLAPSED_SPAN_MS = 1000;
const ENVELOPE_MIN_MS = 60_000;

type Obj = Record<string, any>;
const obj = (v: unknown): Obj => v !== null && typeof v === 'object' && !Array.isArray(v) ? v as Obj : {};
const str = (v: unknown): string | undefined => typeof v === 'string' && v.length ? v : undefined;
const num = (v: unknown): number | undefined => typeof v === 'number' && Number.isFinite(v) ? v : undefined;

export function isCodewhaleSession(value: unknown): boolean {
  const root = obj(value);
  const metadata = obj(root.metadata);
  if (!str(metadata.id)) return false;
  if (root.format === 'whalesong.evidence/v1' || Array.isArray(root.resourceSpans) || root.schemaVersion === 1) return false;
  const journal = obj(root.journal);
  return Array.isArray(root.messages) || Array.isArray(journal.entries);
}

export function isCodewhaleRuntimeRecord(value: unknown): boolean {
  const rec = obj(value);
  return Number.isSafeInteger(rec.seq) && rec.seq >= 0 && typeof rec.event === 'string' && !!rec.event
    && typeof rec.thread_id === 'string' && !!rec.thread_id && rec.timestamp != null;
}

export function isCodewhaleRuntimeDocument(value: unknown): boolean {
  if (!Array.isArray(value) || !value.length) return false;
  const n = Math.min(value.length, 8);
  let hits = 0;
  for (let i = 0; i < n; i++) if (isCodewhaleRuntimeRecord(value[i])) hits++;
  return hits === n;
}

function clip(value: unknown): unknown {
  if (value == null) return value;
  const text = typeof value === 'string' ? value : JSON.stringify(value);
  if (text.length <= PAYLOAD_LIMIT) return typeof value === 'string' ? value : JSON.parse(text);
  return `${text.slice(0, PAYLOAD_LIMIT)}…[truncated ${text.length - PAYLOAD_LIMIT} source bytes]`;
}

function parseTime(value: unknown): number | undefined {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value !== 'string' || !value) return undefined;
  const ms = Date.parse(value);
  return Number.isFinite(ms) ? ms : undefined;
}

function statusOf(value: unknown, isError?: boolean): Status {
  if (isError === true) return 'error';
  if (isError === false) return 'success';
  const s = String(value ?? '').toLowerCase();
  if (s === 'completed' || s === 'success' || s === 'ok') return 'success';
  if (s === 'failed' || s === 'error' || s === 'errored') return 'error';
  if (s === 'canceled' || s === 'cancelled' || s === 'interrupted') return 'error';
  if (s === 'in_progress' || s === 'running') return 'running';
  if (s === 'pending') return 'pending';
  return 'unknown';
}

function classify(name: string): Category {
  const n = name.toLowerCase();
  if (/exception|^error\b/.test(n)) return 'error';
  if (/spawn|fork|subagent|^agent$/.test(n)) return 'agent';
  if (/message\.send|handoff|agent\.message|assistant_message/.test(n)) return 'communication';
  if (/retrieve|retrieval|context|embedding|vector|memory|rag/.test(n)) return 'memory';
  if (/browser|navigate|screenshot|click|playwright/.test(n)) return 'browser';
  if (/read_file|write_file|list_dir|^read$|^write$|^edit$|glob|grep|file\.|filesystem/.test(n)) return 'filesystem';
  if (/bash|exec|shell|run_test|cargo|pytest|compile/.test(n)) return 'code';
  if (/reason|thinking|completion|generate|chat|llm/.test(n)) return 'reasoning';
  if (/http|request|api|fetch|network|mcp_/.test(n)) return 'network';
  if (/user_message|human|approval/.test(n)) return 'human';
  if (/orchestrat|workflow|phase|join|session|thread|turn|todo|plan|operate_contract|status/.test(n)) return 'orchestration';
  if (/tool/.test(n)) return 'tool';
  return CATEGORIES.includes(n as Category) ? n as Category : 'other';
}
export function toolCategory(name: string): Category {
  const category = classify(name);
  return category === 'other' ? 'tool' : category;
}

function pointer(source: string, ids: Obj): Obj {
  return { format: source, ...ids };
}

function titleOfSession(metadata: Obj, filename: string): string {
  const title = str(metadata.title)?.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
  if (title && !title.startsWith('codewhale:runtime_event')) return title.slice(0, 120);
  return `Codewhale session · ${(str(metadata.id) ?? filename).slice(0, 8)}`;
}

function activeJournalEntries(journal: Obj): { entries: Obj[]; warnings: string[] } {
  const entries = Array.isArray(journal.entries) ? journal.entries.map(obj) : [];
  const leaf = str(journal.leaf_id);
  if (!leaf || !entries.length) return { entries, warnings: [] };
  const byId = new Map(entries.filter(e => str(e.id)).map(e => [e.id as string, e]));
  const chain: Obj[] = [];
  const seen = new Set<string>();
  let id: string | undefined = leaf;
  while (id && !seen.has(id)) {
    seen.add(id);
    const entry = byId.get(id);
    if (!entry) break;
    chain.push(entry);
    id = str(entry.parent_id);
  }
  if (!chain.length) return { entries, warnings: ['Journal leaf_id did not resolve; using append order instead of the active branch.'] };
  if (chain.length < entries.length) {
    return {
      entries: chain.reverse(),
      warnings: [`Active journal branch has ${chain.length} of ${entries.length} entries. Forked history was not invented into the timeline.`],
    };
  }
  return { entries: chain.reverse(), warnings: [] };
}

function collapsedTimestamps(entries: Obj[], created?: number, updated?: number): boolean {
  const times = entries.map(e => parseTime(e.created_at)).filter((n): n is number => n !== undefined);
  if (times.length < 2) return false;
  const span = Math.max(...times) - Math.min(...times);
  const envelope = created !== undefined && updated !== undefined ? updated - created : 0;
  return envelope >= ENVELOPE_MIN_MS && span < COLLAPSED_SPAN_MS;
}

function pushEvent(events: WhaleEvent[], event: WhaleEvent): void {
  events.push(event);
}

export function fromCodewhaleSession(document: unknown, filename = 'Codewhale session', maxEvents = 250_000): Trace {
  const root = obj(document);
  const metadata = obj(root.metadata);
  const sessionId = str(metadata.id) ?? filename;
  const journal = obj(root.journal);
  const { entries, warnings } = activeJournalEntries(journal);
  const sourceEntries: Obj[] = entries.length ? entries : (Array.isArray(root.messages) ? root.messages.map((message: unknown, i: number) => ({ id: `${sessionId}/message/${i}`, kind: 'message', message })) : []);
  if (!sourceEntries.length) throw new Error('Codewhale session contains no journal entries or messages.');
  const created = parseTime(metadata.created_at);
  const updated = parseTime(metadata.updated_at);
  const orderOnly = collapsedTimestamps(sourceEntries, created, updated);
  if (orderOnly) {
    warnings.push('Journal created_at values are collapsed to last-save time, not execution time. The time axis is journal order (1 ms per emitted event), not wall-clock duration. Gap, burst, and cycle-period findings are not execution-time claims.');
  } else {
    const times = sourceEntries.map(e => parseTime(e.created_at)).filter((n): n is number => n !== undefined);
    if (!times.length) warnings.push('Journal entries have no usable timestamps. The time axis is journal order.');
  }

  const events: WhaleEvent[] = [];
  const pending = new Map<string, number>();
  let seq = 0;
  const originWall = orderOnly ? undefined : sourceEntries.map(e => parseTime(e.created_at)).find((n): n is number => n !== undefined);
  const agentId = 'parent';
  const model = str(metadata.model);
  const provider = str(metadata.model_provider);

  const when = (entry: Obj, fallback: number): { start: number; open: boolean } => {
    if (orderOnly || originWall === undefined) return { start: fallback, open: false };
    const t = parseTime(entry.created_at);
    if (t === undefined) return { start: fallback, open: true };
    return { start: t - originWall, open: false };
  };

  for (const entry of sourceEntries) {
    if (events.length >= maxEvents) throw new Error(`Import exceeds the ${maxEvents.toLocaleString()} event limit.`);
    const entryId = str(entry.id) ?? `${sessionId}/entry/${seq}`;
    const message = obj(entry.message ?? (entry.kind === 'message' ? entry : {}));
    const role = str(message.role) ?? (str(entry.kind) === 'user' ? 'user' : str(entry.kind) === 'assistant' ? 'assistant' : undefined);
    const blocks: Obj[] = Array.isArray(message.content) ? message.content.map(obj) : [];
    if (!blocks.length) {
      const text = str(entry.text) ?? str(message.text);
      if (text) blocks.push({ type: role === 'user' ? 'text' : 'text', text });
    }
    if (!blocks.length) continue;
    const parentEventId = events.length ? events[events.length - 1]!.id : undefined;
    for (const block of blocks) {
      const t = when(entry, seq);
      const idBase = `${entryId}/${seq}`;
      const type = str(block.type) ?? 'text';
      const raw = pointer('codewhale.session/v1', { sessionId, entryId, seq, blockType: type, toolUseId: block.id ?? block.tool_use_id });
      if (type === 'tool_use' || type === 'server_tool_use') {
        const tool = str(block.name) ?? 'tool';
        const callId = str(block.id) ?? idBase;
        const started = tool === 'agent' && obj(block.input).action === 'start';
        const event: WhaleEvent = {
          schemaVersion: 1, id: callId, traceId: sessionId, parentId: parentEventId,
          startTime: t.start, endTime: t.start, openEnded: true,
          agentId, name: started ? 'agent.spawn' : tool, tool, category: toolCategory(tool),
          subtype: started ? 'fork' : undefined, model, provider,
          status: 'running', attributes: { 'codewhale.entry_id': entryId, 'codewhale.seq': seq, 'tool.name': tool },
          payload: { arguments: clip(block.input) }, raw,
        };
        pending.set(callId, events.length);
        pushEvent(events, event);
      } else if (type === 'tool_result') {
        const callId = str(block.tool_use_id);
        const isError = block.is_error === true;
        const target = callId !== undefined ? pending.get(callId) : undefined;
        if (target !== undefined) {
          const prior = events[target]!;
          prior.endTime = t.start;
          prior.openEnded = false;
          prior.status = statusOf('completed', isError);
          prior.payload = { ...(obj(prior.payload)), result: clip(block.content) };
          prior.attributes = { ...prior.attributes, 'codewhale.result_entry_id': entryId };
          pending.delete(callId as string);
        } else {
          pushEvent(events, {
            schemaVersion: 1, id: idBase, traceId: sessionId, parentId: callId ?? parentEventId,
            startTime: t.start, endTime: t.start, agentId,
            name: 'tool_result', category: 'tool', model, provider,
            status: statusOf(undefined, isError),
            attributes: { 'codewhale.entry_id': entryId, 'codewhale.seq': seq, tool_use_id: callId },
            payload: { result: clip(block.content) }, raw,
          });
        }
      } else if (type === 'thinking') {
        pushEvent(events, {
          schemaVersion: 1, id: idBase, traceId: sessionId, parentId: parentEventId,
          startTime: t.start, endTime: t.start, agentId, name: 'thinking', category: 'reasoning',
          model, provider, status: 'success',
          attributes: { 'codewhale.entry_id': entryId, 'codewhale.seq': seq },
          payload: { thinking: clip(block.thinking ?? block.text) }, raw,
        });
      } else {
        const text = str(block.text) ?? '';
        const operate = text.includes('codewhale:runtime_event');
        const user = role === 'user' || role === 'User';
        pushEvent(events, {
          schemaVersion: 1, id: idBase, traceId: sessionId, parentId: parentEventId,
          startTime: t.start, endTime: t.start, agentId,
          name: operate ? 'operate_contract' : user ? 'user_message' : 'assistant_message',
          category: operate ? 'orchestration' : user ? 'human' : 'communication',
          model, provider, status: 'success',
          attributes: { 'codewhale.entry_id': entryId, 'codewhale.seq': seq, role: role ?? 'unknown' },
          payload: { text: clip(text) }, raw,
        });
      }
      seq += 1;
    }
  }

  if (!events.length) throw new Error('Codewhale session produced no inspectable events.');
  for (const event of events) {
    if (event.openEnded && event.tool) warnings.push(`Tool ${event.id} has no matching tool_result in this snapshot; duration remains unknown.`);
  }
  const base = events.reduce((m, e) => Math.min(m, e.startTime), events[0]!.startTime);
  for (const event of events) { event.startTime -= base; event.endTime -= base; }
  const cost = obj(metadata.cost);
  const sessionCost = num(cost.session_cost_usd);
  const duration = Math.max(1, events.reduce((m, e) => Math.max(m, e.endTime, e.startTime), 0));
  const uniqueWarnings = [...new Set(warnings)];
  return {
    id: sessionId,
    name: titleOfSession(metadata, filename),
    events,
    duration,
    originTime: orderOnly ? 'journal-order' : (str(metadata.created_at) ?? `${base} ms`),
    source: 'codewhale',
    privacy: 'redact',
    warnings: uniqueWarnings,
    metadata: {
      sourceFormat: 'codewhale.session/v1',
      timeBasis: orderOnly || originWall === undefined ? 'journal-order' : 'wall-clock',
      sourceFilename: filename,
      sessionId,
      model,
      provider,
      workspace: metadata.workspace,
      mode: metadata.mode,
      envelopeCreatedAt: metadata.created_at,
      envelopeUpdatedAt: metadata.updated_at,
      cumulativeTurnSecs: metadata.cumulative_turn_secs,
      messageCount: metadata.message_count,
      journalEntries: sourceEntries.length,
      totalTokens: metadata.total_tokens,
      sessionCostUsd: sessionCost,
      pricedTurns: cost.priced_turns,
      unpricedTurns: cost.unpriced_turns,
      runtimeStore: metadata.runtime_store,
      timeUnit: 'ms',
    },
  };
}

function itemToolName(item: Obj, payload: Obj): string | undefined {
  const named = str(payload.tool) ?? str(item.tool) ?? str(item.name);
  if (named) return named;
  if (str(item.kind) !== 'tool_call') return undefined;
  const head = str(item.summary)?.split(':')[0]?.trim();
  if (head && head.length < 80 && !/\s/.test(head)) return head;
  return undefined;
}

function itemCategory(kind: string, tool?: string): Category {
  if (kind === 'user_message') return 'human';
  if (kind === 'agent_reasoning') return 'reasoning';
  if (kind === 'agent_message') return 'communication';
  if (kind === 'status') return 'orchestration';
  if (kind === 'tool_call' && tool) return toolCategory(tool);
  if (kind === 'tool_call') return 'tool';
  return classify(kind);
}

export function fromCodewhaleRuntime(records: unknown[], filename = 'Codewhale runtime', maxEvents = 250_000): Trace {
  if (!records.length) throw new Error('Codewhale runtime event file is empty.');
  const first = obj(records[0]);
  const threadId = str(first.thread_id) ?? filename;
  const warnings: string[] = [];
  let skippedDeltas = 0;
  const open = new Map<string, { index: number; startWall: number }>();
  const requests = new Map<string, WhaleEvent>();
  const events: WhaleEvent[] = [];
  let origin: number | undefined;
  let model: string | undefined;
  let threadName = threadId;

  const stamp = (rec: Obj): number => {
    const t = parseTime(rec.timestamp);
    if (t === undefined) throw new Error(`Runtime event seq ${rec.seq} is missing a usable timestamp.`);
    if (origin === undefined) origin = t;
    return t - origin;
  };

  for (const raw of records) {
    if (!isCodewhaleRuntimeRecord(raw)) throw new Error('Runtime import cancelled: a line is not a Codewhale runtime event record. No rows were skipped.');
    const rec = obj(raw);
    if (rec.thread_id !== threadId) throw new Error('Runtime import contains multiple threads. Export one thread before importing.');
    const eventName = rec.event as string;
    if (eventName === 'item.delta') { skippedDeltas++; continue; }
    if (events.length >= maxEvents) throw new Error(`Import exceeds the ${maxEvents.toLocaleString()} event limit.`);
    const payload = obj(rec.payload);
    const item = obj(payload.item);
    const turn = obj(payload.turn);
    const thread = obj(payload.thread);
    const relative = stamp(rec);
    const turnId = str(rec.turn_id) ?? str(payload.turn_id);
    const itemId = str(rec.item_id) ?? str(item.id);
    const agentId = 'parent';
    if (str(thread.model)) model = str(thread.model);
    if (str(turn.model)) model = str(turn.model) ?? model;

    if (eventName === 'thread.started') {
      model = str(thread.model) ?? model;
      threadName = str(thread.id) ?? threadId;
      pushEvent(events, {
        schemaVersion: 1, id: `thread:${threadId}`, traceId: threadId,
        startTime: relative, endTime: relative, openEnded: true,
        agentId, name: 'thread', category: 'orchestration', model, status: 'running',
        attributes: { 'codewhale.seq': rec.seq, 'whalesong.container': true }, raw: rec,
      });
      continue;
    }
    if (eventName === 'turn.started' || eventName === 'turn.completed') {
      const id = `turn:${turnId ?? rec.seq}`;
      if (eventName === 'turn.completed') for (const [key, request] of requests) {
        if (request.parentId !== id) continue;
        request.endTime = Math.max(request.startTime, relative);
        request.openEnded = false; request.status = 'unknown'; requests.delete(key);
      }
      const startWall = parseTime(turn.started_at) ?? parseTime(turn.created_at);
      const endWall = parseTime(turn.ended_at);
      const start = startWall !== undefined && origin !== undefined ? startWall - origin : relative;
      const end = eventName === 'turn.completed' && endWall !== undefined && origin !== undefined ? endWall - origin : relative;
      const usage = obj(turn.usage);
      const existing = events.findIndex(e => e.id === id);
      const next: WhaleEvent = {
        schemaVersion: 1, id, traceId: threadId, parentId: `thread:${threadId}`,
        startTime: start, endTime: Math.max(start, end), openEnded: eventName !== 'turn.completed',
        agentId, name: 'turn', category: 'orchestration', model,
        inputTokens: num(usage.input_tokens), outputTokens: num(usage.output_tokens),
        status: statusOf(turn.status ?? payload.status), latency: num(turn.duration_ms),
        attributes: { 'codewhale.seq': rec.seq, 'codewhale.turn_id': turnId, 'whalesong.container': true,
          ...(statusOf(turn.status ?? payload.status) === 'error' ? { 'whalesong.error_onset_ms': relative } : {}) },
        payload: { input_summary: clip(turn.input_summary) }, raw: rec,
      };
      if (existing >= 0) events[existing] = { ...events[existing]!, ...next, startTime: events[existing]!.startTime };
      else pushEvent(events, next);
      continue;
    }
    if (eventName === 'turn.lifecycle') continue;
    if (['approval.required', 'approval.decided', 'approval.timeout', 'user_input.required', 'user_input.answered', 'user_input.canceled'].includes(eventName)) {
      const kind = eventName.startsWith('approval.') ? 'approval' : 'user_input';
      const requestId = str(payload[kind === 'approval' ? 'approval_id' : 'input_id']) ?? str(payload.id);
      if (!requestId) throw new Error(`Runtime ${eventName} is missing its request identity.`);
      const key = JSON.stringify([turnId ?? '', kind, requestId]);
      const prior = requests.get(key), required = eventName.endsWith('.required');
      if (required && prior) continue;
      if (!required && prior) {
        prior.endTime = Math.max(prior.startTime, relative); prior.openEnded = false;
        prior.status = eventName === 'approval.decided' || eventName === 'user_input.answered' ? 'success' : 'unknown';
        if (payload.auto === true) {
          // Automatic consent has a receipt, but never asked the human to wait.
          prior.category = 'orchestration'; delete prior.attributes['whalesong.waiting'];
          prior.attributes['whalesong.container'] = true;
        }
        requests.delete(key); continue;
      }
      const automatic = payload.auto === true;
      const event: WhaleEvent = {
        schemaVersion: 1, id: `request:${key}:${rec.seq}`, traceId: threadId,
        parentId: turnId ? `turn:${turnId}` : undefined, startTime: relative, endTime: relative,
        openEnded: required, agentId, name: eventName, category: automatic ? 'orchestration' : 'human',
        status: required ? 'pending' : 'success', model,
        attributes: { 'codewhale.seq': rec.seq, 'whalesong.waiting': required, 'whalesong.container': automatic }, raw: rec,
      };
      if (required) requests.set(key, event);
      pushEvent(events, event); continue;
    }
    if (eventName === 'tool_call.requested' || eventName === 'tool_call.canceled') {
      const callId = str(payload.call_id) ?? `call:${rec.seq}`;
      const tool = str(payload.tool);
      const canceled = eventName === 'tool_call.canceled';
      pushEvent(events, {
        schemaVersion: 1, id: `${eventName}:${callId}`, traceId: threadId, parentId: turnId ? `turn:${turnId}` : undefined,
        startTime: relative, endTime: relative, agentId, name: tool ?? eventName, tool,
        category: tool ? toolCategory(tool) : 'tool', model, status: canceled ? 'error' : 'pending',
        attributes: { 'codewhale.seq': rec.seq, 'codewhale.turn_id': turnId, 'codewhale.call_id': callId, reason: payload.reason },
        payload: { arguments: clip(payload.arguments) }, raw: rec,
      });
      continue;
    }
    if (eventName === 'item.started' || eventName === 'item.completed') {
      const kind = str(item.kind) ?? 'item';
      const tool = itemToolName(item, payload);
      const id = itemId ?? `item:${rec.seq}`;
      const startWall = parseTime(item.started_at);
      const endWall = parseTime(item.ended_at);
      const start = startWall !== undefined && origin !== undefined ? startWall - origin : relative;
      const end = eventName === 'item.completed' && endWall !== undefined && origin !== undefined ? endWall - origin : relative;
      const openEnded = eventName === 'item.started' && endWall === undefined;
      const existing = open.get(id);
      if (existing && eventName === 'item.completed') {
        const prior = events[existing.index]!;
        prior.endTime = Math.max(prior.startTime, end);
        prior.openEnded = false;
        prior.status = statusOf(item.status);
        if (prior.status === 'error') prior.attributes['whalesong.error_onset_ms'] = relative;
        prior.payload = { summary: clip(item.summary), detail: clip(item.detail) };
        open.delete(id);
        continue;
      }
      const event: WhaleEvent = {
        schemaVersion: 1, id, traceId: threadId, parentId: turnId ? `turn:${turnId}` : undefined,
        startTime: start, endTime: Math.max(start, end), openEnded,
        agentId, name: tool ?? kind, tool, category: itemCategory(kind, tool), model,
        status: statusOf(item.status ?? (eventName === 'item.started' ? 'running' : undefined)),
        attributes: { 'codewhale.seq': rec.seq, 'codewhale.turn_id': turnId, 'codewhale.item_kind': kind,
          ...(statusOf(item.status) === 'error' ? { 'whalesong.error_onset_ms': relative } : {}) },
        payload: { summary: clip(item.summary), detail: clip(item.detail) }, raw: rec,
      };
      if (eventName === 'item.started') open.set(id, { index: events.length, startWall: (origin ?? 0) + start });
      pushEvent(events, event);
      continue;
    }
    pushEvent(events, {
      schemaVersion: 1, id: `${eventName}:${rec.seq}`, traceId: threadId, parentId: turnId ? `turn:${turnId}` : undefined,
      startTime: relative, endTime: relative, agentId, name: eventName, category: classify(eventName),
      model, status: 'unknown', attributes: { 'codewhale.seq': rec.seq }, raw: rec,
    });
  }

  if (skippedDeltas) warnings.push(`Dropped ${skippedDeltas.toLocaleString()} item.delta records; they are token stream fragments, not spans. Item start/end remain the source of duration.`);
  for (const [id] of open) warnings.push(`Item ${id} started and never completed in this file; duration remains unknown.`);
  for (const request of requests.values()) warnings.push(`Request ${request.id} has no terminal receipt; its duration remains unknown in this file.`);
  if (!events.length) throw new Error('Codewhale runtime file contained only stream deltas or unreadable records.');
  const base = events.reduce((m, e) => Math.min(m, e.startTime), events[0]!.startTime);
  for (const event of events) {
    if (event.attributes['whalesong.error_onset_ms'] !== undefined) event.attributes['whalesong.error_onset_ms'] = errorOnsetOf(event) - base;
    event.startTime -= base; event.endTime -= base;
  }
  return {
    id: threadId,
    name: `Codewhale runtime · ${threadName}`,
    events,
    duration: Math.max(1, events.reduce((m, e) => Math.max(m, e.endTime, e.startTime, e.status === 'error' ? errorOnsetOf(e) : 0), 0)),
    originTime: origin !== undefined ? new Date(origin + base).toISOString() : '0 ms',
    source: 'codewhale',
    privacy: 'redact',
    warnings: [...new Set(warnings)],
    metadata: {
      sourceFormat: 'codewhale.runtime-events/v2',
      timeBasis: 'wall-clock',
      sourceFilename: filename,
      threadId,
      model,
      skippedDeltas,
      recordCount: records.length,
      timeUnit: 'ms',
    },
  };
}

/** The journal owns request state until a matching terminal receipt. A live
 * driver may confirm that state only while its cursor-checked stream is healthy.
 * Ordinary open tool spans remain unknown-duration; no execution is inferred. */
export function observeRuntimeRequests(trace: Trace, observedThrough: number): Trace {
  const origin = Date.parse(trace.originTime ?? '');
  if (trace.metadata.sourceFormat !== 'codewhale.runtime-events/v2' || !Number.isFinite(origin)
    || !Number.isFinite(observedThrough)) throw new Error('Invalid Runtime observation horizon.');
  const at = observedThrough - origin;
  const events = trace.events.map(e => e.openEnded && e.attributes['whalesong.waiting'] === true && at >= e.startTime
    ? { ...e, endTime: at, openEnded: false } : e);
  return { ...trace, events, duration: Math.max(trace.duration, at) };
}
