import { fromCodewhaleRuntime, fromCodewhaleSession, isCodewhaleRuntimeDocument, isCodewhaleSession } from './codewhale.js';
import { validateBundle, evidenceToTrace, validateObservation, bundleFromTrace } from './evidence.js';
import { CATEGORIES, errorOnsetOf, type Category, type PrivacyMode, type Status, type Trace, type WhaleEvent } from './model.js';

export interface ImportOptions {
  privacy?: PrivacyMode; maxEvents?: number; maxBytes?: number; maxTraces?: number;
  /** Optional local-only redaction hook. Never invoked by a network service. */
  redactor?: (value: unknown, path: string) => unknown;
}
type Obj = Record<string, any>;
const obj = (v: unknown): Obj => v !== null && typeof v === 'object' && !Array.isArray(v) ? v as Obj : {};
const list = (v: unknown): any[] => Array.isArray(v) ? v : [];
const str = (v: unknown): string | undefined => typeof v === 'string' && v.length ? v : undefined;
const SECRET_KEY = /(?:^|[._-])(?:api[._-]?key|authorization|password|passwd|secret|access[._-]?token|refresh[._-]?token|cookie|private[._-]?key)(?:$|[._-])/i;
const SAFE_META = /^(?:gen_ai\.(?:usage\.(?:input_tokens(?:\.cached)?|output_tokens|cache_read\.input_tokens|cost)|request\.(?:model|max_tokens)|response\.model|provider\.name|operation\.name)|whalesong\.(?:category|agent_id)|agent\.(?:id|parent_id|state)|(?:http|rpc)\.(?:response\.status_code|method)|retry\.(?:count|attempt)|context\.(?:tokens|limit)|phase|spawnedAgentId|parentAgentId|targetType|iteration|benchmark|blocked)$/;

/** Best effort, not a guarantee of de-identification. Preserves structure, not secrets. */
export function redact(value: unknown, path = '', hook?: ImportOptions['redactor'], depth = 0, seen = new WeakSet<object>()): unknown {
  if (depth > 48) return '[REDACTED: nesting limit]';
  const key = path.split('/').at(-1) ?? '';
  if (SECRET_KEY.test(key) || /^(apiKey|accessToken|refreshToken|privateKey)$/i.test(key)) return '[REDACTED]';
  // OTLP attributes encode a sensitive name in `key`, not in the JSON path.
  // Redact the AnyValue itself while preserving a valid OTLP value envelope.
  const record = obj(value);
  if (typeof record.key === 'string' && Object.hasOwn(record, 'value') &&
      (SECRET_KEY.test(record.key) || /^(apiKey|accessToken|refreshToken|privateKey)$/i.test(record.key))) {
    const safe = { ...record, value: { stringValue: '[REDACTED]' } };
    // Only the known KeyValue fields are kept; arbitrary siblings cannot bypass redaction.
    const cleaned = { key: record.key, value: safe.value };
    return hook ? hook(cleaned, path) : cleaned;
  }
  if (value && typeof value === 'object') { if (seen.has(value)) return '[REDACTED: cycle]'; seen.add(value); }
  let out: unknown = value;
  if (typeof value === 'string') {
    out = value
      .replace(/-----BEGIN [^-]*PRIVATE KEY-----[\s\S]*?-----END [^-]*PRIVATE KEY-----/g, '[REDACTED: private key]')
      .replace(/\b(?:sk-[A-Za-z0-9_-]{16,}|AKIA[0-9A-Z]{16})\b/g, '[REDACTED: key]')
      .replace(/\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[baprs]-[A-Za-z0-9-]{16,})\b/g, '[REDACTED: token]')
      .replace(/(https?:\/\/)[^\s/@:]+:[^\s/@]+@/gi, '$1[REDACTED]@')
      .replace(/\bBearer\s+[A-Za-z0-9._~+\/-]+=*/gi, 'Bearer [REDACTED]')
      .replace(/\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b/g, '[REDACTED: JWT]')
      .replace(/((?:api[_-]?key|password|secret)\s*[=:]\s*)[^\s,;"'}]+/gi, '$1[REDACTED]')
      .replace(/\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi, '[REDACTED: email]');
  } else if (Array.isArray(value)) {
    out = value.map((v, i) => redact(v, `${path}/${i}`, hook, depth + 1, seen));
  } else if (value && typeof value === 'object') {
    out = Object.fromEntries(Object.entries(value).map(([k, v]) => [k, redact(v, `${path}/${k}`, hook, depth + 1, seen)]));
  }
  if (value && typeof value === 'object') seen.delete(value);
  return hook ? hook(out, path) : out;
}
export function privacyEvent(e: WhaleEvent, mode: PrivacyMode): WhaleEvent {
  if (mode === 'metadata') {
    const { payload: _payload, raw: _raw, ...rest } = e;
    return { ...rest, links: e.links?.map(link=>({traceId:link.traceId,spanId:link.spanId})), attributes: Object.fromEntries(Object.entries(e.attributes).filter(([k, v]) =>
      SAFE_META.test(k) && (typeof v !== 'object' || v === null)
      || ['whalesong.container', 'codewhale.container', 'whalesong.waiting'].includes(k) && typeof v === 'boolean'
      // Relative timestamps can be negative before the importer rebases them.
      || k === 'whalesong.error_onset_ms' && typeof v === 'number' && Number.isFinite(v))) };
  }
  return e;
}
function number(v: unknown, field: string, optional = true): number | undefined {
  if (v === undefined || v === null) {
    if (optional) return undefined;
    throw new Error(`Missing ${field}. Times must be numeric milliseconds.`);
  }
  if (typeof v !== 'number' || !Number.isFinite(v) || Math.abs(v) > Number.MAX_SAFE_INTEGER) throw new Error(`Invalid ${field}: expected a finite, safely representable number.`);
  return v;
}
function nonnegative(v: unknown, field: string): number | undefined {
  const n = number(v, field); if (n !== undefined && n < 0) throw new Error(`${field} must be nonnegative.`); return n;
}
function numericAttr(v: unknown): number | undefined {
  if (v === undefined || v === null || v === '') return undefined;
  const n = Number(v); return Number.isFinite(n) && n >= 0 ? n : undefined;
}
export function categoryFor(name: string, a: Obj): Category {
  const explicit = a['whalesong.category'] ?? a.category;
  if (CATEGORIES.includes(explicit)) return explicit;
  const n = name.toLowerCase(), op = String(a['gen_ai.operation.name'] ?? '').toLowerCase();
  if (/exception|^error\b/.test(n)) return 'error';
  if (/spawn|fork|subagent/.test(n) || op === 'invoke_agent') return 'agent';
  if (/message\.send|handoff|agent\.message/.test(n)) return 'communication';
  if (/retrieve|retrieval|context|embedding|vector|memory|rag/.test(n)) return 'memory';
  if (/browser|navigate|screenshot|click|playwright/.test(n)) return 'browser';
  if (/read_file|write_file|list_dir|file\.|filesystem|fs\.|patch/.test(n)) return 'filesystem';
  if (/exec|shell|run_test|cargo|pytest|compile/.test(n)) return 'code';
  if (a['gen_ai.request.model'] || a['llm.model_name'] || /reason|completion|generate|chat|llm/.test(n) || ['chat', 'generate_content', 'text_completion'].includes(op)) return 'reasoning';
  if (a['gen_ai.tool.name'] || a['tool.name'] || /tool|search|function/.test(n)) return 'tool';
  if (a['http.request.method'] || a['http.method'] || a['rpc.system'] || /http|request|api|fetch|network/.test(n)) return 'network';
  if (/user|human|approval/.test(n)) return 'human';
  if (/orchestrat|workflow|phase|join|session|root/.test(n)) return 'orchestration';
  return 'other';
}
export function decodeAnyValue(v: unknown): unknown {
  const a = obj(v);
  if ('stringValue' in a) return a.stringValue;
  if ('boolValue' in a) return a.boolValue;
  if ('doubleValue' in a) return a.doubleValue;
  if ('intValue' in a) {
    const n = Number(a.intValue);
    return Number.isSafeInteger(n) ? n : String(a.intValue);
  }
  if ('bytesValue' in a) return a.bytesValue;
  if ('arrayValue' in a) return list(obj(a.arrayValue).values).map(decodeAnyValue);
  if ('kvlistValue' in a) return attributes(obj(a.kvlistValue).values);
  return v;
}
export function attributes(value: unknown): Obj {
  if (!Array.isArray(value)) return obj(value);
  return Object.fromEntries(value.filter(x => typeof x?.key === 'string').map(x => [x.key, decodeAnyValue(x.value)]));
}
function ns(v: unknown, field: string): bigint {
  if (typeof v === 'number' && !Number.isSafeInteger(v)) throw new Error(`${field} lost precision: encode OTLP nanoseconds as a decimal string.`);
  const s = String(v ?? '');
  if (!/^\d{1,20}$/.test(s) || BigInt(s) > 18446744073709551615n) throw new Error(`Invalid ${field}: expected an OTLP nanosecond integer string.`);
  return BigInt(s);
}
function otelStatus(v: unknown): Status {
  const c = obj(v).code;
  return c === 2 || c === 'STATUS_CODE_ERROR' ? 'error' : c === 1 || c === 'STATUS_CODE_OK' ? 'success' : 'unknown';
}
function normalizedEvent(v: unknown, index: number): WhaleEvent {
  const a = obj(v), id = str(a.id), traceId = str(a.traceId), name = str(a.name);
  if (!id || !traceId || !name) throw new Error(`Record ${index + 1} requires nonempty id, traceId, and name.`);
  if (a.schemaVersion !== undefined && a.schemaVersion !== 1) throw new Error(`Record ${index + 1}: unsupported schemaVersion ${a.schemaVersion}.`);
  const startTime = number(a.startTime, 'startTime', false)!;
  const endTime = number(a.endTime, 'endTime') ?? startTime;
  if (endTime < startTime) throw new Error(`Record ${index + 1}: endTime precedes startTime.`);
  if (a.category !== undefined && !CATEGORIES.includes(a.category)) throw new Error(`Unknown category "${a.category}". Use "other" plus subtype for extensions.`);
  const allowed: Status[] = ['pending', 'running', 'success', 'error', 'unknown'];
  if (a.status !== undefined && !allowed.includes(a.status)) throw new Error(`Record ${index + 1}: invalid status.`);
  const at = obj(a.attributes);
  return {
    schemaVersion: 1, id, traceId, name, parentId: str(a.parentId), startTime, endTime,
    openEnded: a.endTime === undefined || a.openEnded === true,
    agentId: str(a.agentId) ?? 'unattributed', agentType: str(a.agentType), parentAgentId: str(a.parentAgentId),
    category: a.category ?? categoryFor(name, at), subtype: str(a.subtype),
    model: str(a.model), provider: str(a.provider), tool: str(a.tool),
    inputTokens: nonnegative(a.inputTokens, 'inputTokens'), outputTokens: nonnegative(a.outputTokens, 'outputTokens'),
    cachedTokens: nonnegative(a.cachedTokens, 'cachedTokens'), cost: nonnegative(a.cost, 'cost'),
    costCurrency: str(a.costCurrency), latency: nonnegative(a.latency, 'latency'),
    contextTokens: nonnegative(a.contextTokens, 'contextTokens'), contextLimit: nonnegative(a.contextLimit, 'contextLimit'),
    retry: nonnegative(a.retry, 'retry'), status: a.status ?? 'unknown',
    sourceId: str(a.sourceId), targetId: str(a.targetId), targetType: str(a.targetType),
    links: list(a.links).filter(x => typeof x?.traceId === 'string' && typeof x?.spanId === 'string'),
    attributes: at, payload: a.payload, raw: a.raw ?? v,
    observation: a.observation === undefined ? undefined : validateObservation(a.observation),
  };
}
interface OtlpRecord { span: Obj; resource: Obj; scope: Obj; resourceSchema?: string; scopeSchema?: string }
function otlpRecords(doc: Obj): OtlpRecord[] {
  const out: OtlpRecord[] = [];
  for (const r of list(doc.resourceSpans)) {
    for (const s of list(r.scopeSpans ?? r.instrumentationLibrarySpans)) {
      for (const span of list(s.spans)) out.push({ span: obj(span), resource: obj(r.resource), scope: obj(s.scope ?? s.instrumentationLibrary), resourceSchema: r.schemaUrl, scopeSchema: s.schemaUrl });
    }
  }
  return out;
}
function fromOTLP(doc: Obj, maxEvents: number): { events: WhaleEvent[]; origins: Map<string, string>; warnings: string[] } {
  const records = otlpRecords(doc), bases = new Map<string, bigint>(), warnings: string[] = [];
  if (!records.length) throw new Error('No spans found in resourceSpans[].scopeSpans[].spans[].');
  if (records.length > maxEvents) throw new Error(`Import exceeds the ${maxEvents.toLocaleString()} event limit.`);
  for (const { span: s } of records) {
    if (!str(s.traceId) || !str(s.spanId)) throw new Error('Every OTLP span requires traceId and spanId.');
    const start = ns(s.startTimeUnixNano, 'startTimeUnixNano');
    const end = s.endTimeUnixNano === undefined ? start : ns(s.endTimeUnixNano, 'endTimeUnixNano');
    if (end < start) throw new Error(`OTLP span ${s.spanId}: end precedes start.`);
    let earliest = start;
    for (const e of list(s.events)) { const t = ns(e.timeUnixNano, 'event.timeUnixNano'); if (t < earliest) earliest = t; }
    if (!bases.has(s.traceId) || earliest < bases.get(s.traceId)!) bases.set(s.traceId, earliest);
    if ((s.droppedEventsCount ?? 0) > 0) warnings.push(`Span ${s.spanId} reports ${s.droppedEventsCount} dropped events; coverage is incomplete.`);
  }
  const events: WhaleEvent[] = [];
  for (const rec of records) {
    const s = rec.span, a = { ...attributes(rec.resource.attributes), ...attributes(s.attributes) };
    const origin = bases.get(s.traceId)!, start = ns(s.startTimeUnixNano, 'startTimeUnixNano');
    const end = s.endTimeUnixNano === undefined ? start : ns(s.endTimeUnixNano, 'endTimeUnixNano');
    const name = String(s.name ?? 'unnamed span');
    const e: WhaleEvent = {
      schemaVersion: 1, id: s.spanId, traceId: s.traceId, parentId: str(s.parentSpanId),
      name, startTime: Number(start - origin) / 1e6, endTime: Number(end - origin) / 1e6,
      openEnded: s.endTimeUnixNano === undefined,
      agentId: String(a['whalesong.agent_id'] ?? a['gen_ai.agent.id'] ?? a['agent.id'] ?? a['service.name'] ?? 'unattributed'),
      parentAgentId: str(a['agent.parent_id']), agentType: str(a['gen_ai.agent.name']),
      category: categoryFor(name, a), model: str(a['gen_ai.request.model'] ?? a['gen_ai.response.model'] ?? a['llm.model_name']),
      provider: str(a['gen_ai.provider.name'] ?? a['gen_ai.system']), tool: str(a['gen_ai.tool.name'] ?? a['tool.name']),
      inputTokens: numericAttr(a['gen_ai.usage.input_tokens'] ?? a['llm.token_count.prompt']),
      outputTokens: numericAttr(a['gen_ai.usage.output_tokens'] ?? a['llm.token_count.completion']),
      cachedTokens: numericAttr(a['gen_ai.usage.cache_read.input_tokens'] ?? a['gen_ai.usage.input_tokens.cached']),
      cost: numericAttr(a['whalesong.cost'] ?? a['gen_ai.usage.cost']), costCurrency: str(a['whalesong.cost_currency']),
      contextTokens: numericAttr(a['context.tokens']), contextLimit: numericAttr(a['context.limit']),
      retry: numericAttr(a['retry.count'] ?? a['retry.attempt']), status: otelStatus(s.status),
      latency: Number(end - start) / 1e6, attributes: a,
      sourceId: str(a['whalesong.source_id']), targetId: str(a['whalesong.target_id']), targetType: str(a['whalesong.target_type']),
      links: list(s.links).map(l => ({ traceId: l.traceId, spanId: l.spanId, attributes: attributes(l.attributes) })),
      payload: a['gen_ai.input.messages'] !== undefined || a['gen_ai.output.messages'] !== undefined ? {
        request: a['gen_ai.input.messages'], response: a['gen_ai.output.messages'],
      } : undefined,
      raw: rec,
    };
    events.push(e);
    for (const [i, record] of list(s.events).entries()) {
      const ea = attributes(record.attributes), time = Number(ns(record.timeUnixNano, 'event.timeUnixNano') - origin) / 1e6;
      const ename = String(record.name ?? 'span event');
      events.push({ schemaVersion: 1, id: `${s.spanId}/event/${i}`, traceId: s.traceId, parentId: s.spanId,
        startTime: time, endTime: time, agentId: e.agentId, name: ename, category: categoryFor(ename, ea),
        status: ename === 'exception' ? 'error' : 'unknown', attributes: ea, raw: { event: record, spanId: s.spanId, resource: rec.resource, scope: rec.scope } });
    }
    if (events.length > maxEvents) throw new Error(`Import exceeds the ${maxEvents.toLocaleString()} event limit, including span events.`);
  }
  return { events, origins: new Map([...bases].map(([k, v]) => [k, v.toString()])), warnings };
}

/** Parse strictly: malformed lines or duplicate identities never disappear silently. */
export function importTrace(text: string, filename = 'Imported trace', options: ImportOptions = {}): Trace[] {
  const mode = options.privacy ?? 'redact', maxEvents = options.maxEvents ?? 250_000;
  if (!['redact', 'metadata', 'retain'].includes(mode)) throw new Error('Unknown privacy mode.');
  const maxTraces = options.maxTraces ?? 8;
  if (!Number.isInteger(maxEvents) || maxEvents < 1 || maxEvents > 250_000) throw new Error('maxEvents must be in [1, 250000].');
  if (!Number.isInteger(maxTraces) || maxTraces < 1 || maxTraces > 64) throw new Error('maxTraces must be in [1, 64].');
  if (new TextEncoder().encode(text).length > (options.maxBytes ?? 64 * 1024 * 1024)) throw new Error('File exceeds the 64 MiB MVP import limit. Split the export by trace.');
  const trimmed = text.replace(/^\uFEFF/, '').trim();
  if (!trimmed) throw new Error('The trace file is empty.');
  let document: unknown;
  try { document = JSON.parse(trimmed); }
  catch {
    document = trimmed.split(/\r?\n/).filter(l => l.trim()).map((line, i) => {
      try { return JSON.parse(line); } catch { throw new Error(`Invalid JSON on nonempty line ${i + 1}. Import cancelled; no rows were skipped.`); }
    });
  }
  // Transform before both normalization and raw retention, so raw cannot bypass redaction.
  const safe = mode === 'retain' ? (options.redactor ? options.redactor(document, '') : document) : redact(document, '', options.redactor);
  const root = obj(safe);
  if(root.format === 'whalesong.evidence/v1') return [evidenceToTrace(validateBundle(root, Math.min(maxEvents, 100_000)))];
  if (isCodewhaleSession(safe)) {
    const trace = fromCodewhaleSession(safe, filename, maxEvents);
    trace.privacy = mode;
    trace.events = trace.events.map(e => privacyEvent(e, mode));
    return [trace];
  }
  const records = Array.isArray(safe) ? safe : Array.isArray(root.events) ? root.events : null;
  if (isCodewhaleRuntimeDocument(records ?? [safe])) {
    const trace = fromCodewhaleRuntime(records ?? [safe], filename, maxEvents);
    trace.privacy = mode;
    trace.events = trace.events.map(e => privacyEvent(e, mode));
    return [trace];
  }
  const isOTLP = Array.isArray(root.resourceSpans);
  let all: WhaleEvent[], origins = new Map<string, string>(), warnings: string[] = [];
  if (isOTLP) { const r = fromOTLP(root, maxEvents); all = r.events; origins = r.origins; warnings = r.warnings; }
  else {
    const incoming = records ?? [safe];
    if (incoming.length > maxEvents) throw new Error(`Import exceeds the ${maxEvents.toLocaleString()} event limit.`);
    all = incoming.map(normalizedEvent);
  }
  if (!all.length) throw new Error('The file contains no events.');
  const groups = new Map<string, WhaleEvent[]>(), ids = new Set<string>();
  for (const e of all) {
    const key = `${e.traceId}\0${e.id}`;
    if (ids.has(key)) throw new Error(`Duplicate event identity (${e.traceId}, ${e.id}). Import cancelled.`);
    ids.add(key);
    const group = groups.get(e.traceId) ?? []; group.push(privacyEvent(e, mode)); groups.set(e.traceId, group);
    if (groups.size > maxTraces) throw new Error(`This import contains more than ${maxTraces} traces. Split it by trace ID.`);
  }
  return [...groups].map(([id, events]) => {
    events.sort((a, b) => a.startTime - b.startTime || a.id.localeCompare(b.id));
    const base = isOTLP ? 0 : events[0].startTime;
    if (!isOTLP && events.some(e => Math.abs(e.endTime - base) > Number.MAX_SAFE_INTEGER)) throw new Error('Trace duration exceeds safely representable milliseconds.');
    for (const e of events) {
      if (e.attributes['whalesong.error_onset_ms'] !== undefined) e.attributes['whalesong.error_onset_ms'] = errorOnsetOf(e) - base;
      e.startTime -= base; e.endTime -= base;
    }
    const localIds = new Set(events.map(e => e.id)), missingParents = events.filter(e => e.parentId && !localIds.has(e.parentId)).length;
    const traceWarnings = [...warnings];
    if (missingParents) traceWarnings.push(`${missingParents} parent spans are absent from this trace; no parent relationship was invented.`);
    if (events.some(e => e.openEnded)) traceWarnings.push('Open spans have unknown duration and are displayed as onset-only, not extended into invented activity.');
    const currencies = new Set(events.filter(e => e.cost !== undefined).map(e => e.costCurrency ?? 'unspecified'));
    if (currencies.size > 1) traceWarnings.push('Mixed cost currencies: aggregate cost comparison is disabled.');
    return { id, name: groups.size > 1 ? `${filename} · ${id.slice(0, 8)}` : String(root.name ?? filename),
      events, duration: Math.max(1, events.reduce((m, e) => Math.max(m, e.endTime, e.startTime, e.status === 'error' ? errorOnsetOf(e) : 0), 0)),
      originTime: origins.get(id) ?? (root.originTime !== undefined && base === 0 ? str(root.originTime) : `${base} ms`),
      source: isOTLP ? 'otlp' as const : 'jsonl' as const, privacy: mode, warnings: [...new Set(traceWarnings)],
      metadata: { ...(mode === 'metadata' ? {} : obj(root.metadata)), timeUnit: 'ms', originUnit: isOTLP ? 'unix-nanoseconds' : 'milliseconds', sourceFilename: filename },
    };
  });
}

export function exportJSONL(trace: Trace): string {
  const bundle=bundleFromTrace(trace);
  if(bundle)return JSON.stringify(bundle);
  return trace.events.map(e => JSON.stringify(e)).join('\n') + '\n';
}
