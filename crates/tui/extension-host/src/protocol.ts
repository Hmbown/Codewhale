/**
 * Codewhale extension-host protocol, version 1 (phase 1 subset).
 *
 * The Rust serde types in `crates/tui/src/extension_host/protocol.rs` are the
 * source of truth. Phase 1 keeps this file hand-written and pins both sides
 * to the shared corpus in `crates/tui/tests/fixtures/extension_host/protocol`,
 * which both parsers must accept (or reject) identically and round-trip.
 *
 * Frame: 4-byte magic `CWX1`, u32 little-endian payload length, UTF-8 JSON.
 * Envelope: JSON-RPC 2.0.
 */

export const PROTOCOL_VERSION = 1
export const MAGIC = Buffer.from('CWX1', 'ascii')
export const MAX_FRAME = 32 * 1024 * 1024
export const HEADER_LEN = 8

/** JSON-RPC error codes used on this channel. */
export const ErrorCode = {
  ParseError: -32700,
  InvalidRequest: -32600,
  MethodNotFound: -32601,
  InvalidParams: -32602,
  Internal: -32603,
  /** The tool body threw. */
  ExecutionFailed: -32000,
  /** The handle is unknown, revoked, or its owner is not active. */
  NotAvailable: -32001,
  /** Cancelled by `$/cancel` (LSP's RequestCancelled). */
  Cancelled: -32800,
} as const

/** Methods the core sends to the host. */
export const CORE_TO_HOST = {
  requests: ['host/initialize', 'host/shutdown', 'ext/activate', 'ext/deactivate', 'tool/call'],
  notifications: ['$/cancel'],
} as const

/** Methods the host sends to the core. */
export const HOST_TO_CORE = {
  requests: ['registry/register', 'registry/unregister'],
  notifications: ['host/hello', 'host/ready', 'ext/faulted', 'log', '$/cancel'],
} as const

export type Json = null | boolean | number | string | Json[] | { [key: string]: Json }

export interface OwnerRef {
  plugin_id: string
  generation: number
  owner_token: string
}

export interface ToolSpecWire {
  name: string
  description: string
  input_schema: { [key: string]: Json }
}

export interface ContentBlockWire {
  type: 'text'
  text: string
}

export interface ToolResultWire {
  content: ContentBlockWire[]
  is_error: boolean
  structured?: Json
}

export interface RpcErrorWire {
  code: number
  message: string
  data?: Json
}

export type Message =
  | { jsonrpc: '2.0'; id: number; method: string; params?: any }
  | { jsonrpc: '2.0'; method: string; params?: any }
  | { jsonrpc: '2.0'; id: number; result: any }
  | { jsonrpc: '2.0'; id: number; error: RpcErrorWire }

export class FrameError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'FrameError'
  }
}

export class ProtocolError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ProtocolError'
  }
}

/** Encode one message into a `CWX1` frame. Oversized payloads are refused, never truncated. */
export function encodeFrame(message: unknown): Buffer {
  const payload = Buffer.from(JSON.stringify(message), 'utf8')
  if (payload.length > MAX_FRAME) {
    throw new FrameError(`frame of ${payload.length} bytes exceeds MAX_FRAME ${MAX_FRAME}`)
  }
  const header = Buffer.alloc(HEADER_LEN)
  MAGIC.copy(header, 0)
  header.writeUInt32LE(payload.length, 4)
  return Buffer.concat([header, payload])
}

/** Incremental `CWX1` decoder. Throws `FrameError` on bad magic, bad length, or bad JSON. */
export class FrameDecoder {
  private buffer: Buffer = Buffer.alloc(0)

  push(chunk: Buffer): unknown[] {
    this.buffer = this.buffer.length === 0 ? chunk : Buffer.concat([this.buffer, chunk])
    const out: unknown[] = []
    while (this.buffer.length >= HEADER_LEN) {
      if (!this.buffer.subarray(0, 4).equals(MAGIC)) {
        throw new FrameError('bad frame magic')
      }
      const length = this.buffer.readUInt32LE(4)
      if (length > MAX_FRAME) throw new FrameError(`frame length ${length} exceeds MAX_FRAME`)
      if (this.buffer.length < HEADER_LEN + length) break
      const payload = this.buffer.subarray(HEADER_LEN, HEADER_LEN + length)
      this.buffer = this.buffer.subarray(HEADER_LEN + length)
      let value: unknown
      try {
        value = JSON.parse(payload.toString('utf8'))
      } catch (error) {
        throw new FrameError(`frame payload is not JSON: ${(error as Error).message}`)
      }
      out.push(value)
    }
    return out
  }
}

// ---------------------------------------------------------------------------
// Validation. Host→core output is validated strictly before it is sent (the
// Rust side uses deny_unknown_fields for it); core→host input is validated
// tolerantly (unknown fields ignored), mirroring the Rust side.
// ---------------------------------------------------------------------------

type Shape = Record<string, 'string' | 'number' | 'boolean' | 'object' | 'array' | 'json' | 'owner' | 'u64'>

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function checkShape(
  where: string,
  value: unknown,
  required: Shape,
  optional: Shape = {},
  strict = false,
): asserts value is Record<string, any> {
  if (!isObject(value)) throw new ProtocolError(`${where}: expected an object`)
  for (const [key, kind] of Object.entries(required)) {
    if (!(key in value)) throw new ProtocolError(`${where}: missing field \`${key}\``)
    checkKind(`${where}.${key}`, value[key], kind)
  }
  for (const [key, kind] of Object.entries(optional)) {
    if (key in value && value[key] !== undefined) checkKind(`${where}.${key}`, value[key], kind)
  }
  if (strict) {
    for (const key of Object.keys(value)) {
      if (!(key in required) && !(key in optional)) {
        throw new ProtocolError(`${where}: unknown field \`${key}\``)
      }
    }
  }
}

function checkKind(where: string, value: unknown, kind: Shape[string]) {
  switch (kind) {
    case 'string':
      if (typeof value !== 'string') throw new ProtocolError(`${where}: expected a string`)
      return
    case 'number':
      if (typeof value !== 'number' || !Number.isFinite(value)) throw new ProtocolError(`${where}: expected a number`)
      return
    case 'u64':
      if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
        throw new ProtocolError(`${where}: expected an unsigned integer`)
      }
      return
    case 'boolean':
      if (typeof value !== 'boolean') throw new ProtocolError(`${where}: expected a boolean`)
      return
    case 'object':
      if (!isObject(value)) throw new ProtocolError(`${where}: expected an object`)
      return
    case 'array':
      if (!Array.isArray(value)) throw new ProtocolError(`${where}: expected an array`)
      return
    case 'owner':
      checkShape(where, value, { plugin_id: 'string', generation: 'u64', owner_token: 'string' }, {}, true)
      return
    case 'json':
      return
  }
}

/** Params validation per method and direction. `strict` = deny unknown fields. */
const PARAMS: Record<string, { dir: 'core' | 'host'; required: Shape; optional?: Shape; check?: (p: any, strict: boolean) => void }> = {
  // host → core
  'host/hello': {
    dir: 'host',
    required: { protocol: 'object', host_version: 'string', bundle_sha256: 'string', node_version: 'string' },
    check: (p, strict) => checkShape('host/hello.protocol', p.protocol, { min: 'u64', max: 'u64' }, {}, strict),
  },
  'host/ready': { dir: 'host', required: {} },
  'registry/register': {
    dir: 'host',
    required: { owner: 'owner', kind: 'string', spec: 'object' },
    check: (p, strict) => {
      if (p.kind !== 'tool') throw new ProtocolError(`registry/register: unsupported kind \`${p.kind}\``)
      checkShape('registry/register.spec', p.spec, { name: 'string', description: 'string', input_schema: 'object' }, {}, strict)
    },
  },
  'registry/unregister': { dir: 'host', required: { owner: 'owner', handle: 'u64' } },
  'ext/faulted': { dir: 'host', required: { owner: 'owner', error: 'string' } },
  log: { dir: 'host', required: { level: 'string', msg: 'string' }, optional: { plugin_id: 'string' } },
  // core → host
  'host/initialize': {
    dir: 'core',
    required: { protocol: 'u64', limits: 'object' },
    check: (p, strict) =>
      checkShape(
        'host/initialize.limits',
        p.limits,
        { max_frame: 'u64', max_inflight: 'u64', dispose_deadline_ms: 'u64', activate_deadline_ms: 'u64' },
        {},
        strict,
      ),
  },
  'host/shutdown': { dir: 'core', required: {} },
  'ext/activate': {
    dir: 'core',
    required: { owner: 'owner', plugin_name: 'string', entry: 'object' },
    optional: { config: 'json' },
    check: (p, strict) => checkShape('ext/activate.entry', p.entry, { path: 'string', sha256: 'string' }, {}, strict),
  },
  'ext/deactivate': { dir: 'core', required: { owner: 'owner' } },
  'tool/call': {
    dir: 'core',
    required: { handle: 'u64', call_id: 'string', input: 'json', deadline_ms: 'u64' },
  },
}

/** `$/cancel` flows both ways. */
const CANCEL_SHAPE: Shape = { id: 'u64' }

export type Direction = 'core_to_host' | 'host_to_core'

/**
 * Validate one decoded message travelling in `direction`. Host→core messages
 * are validated strictly (unknown fields rejected); core→host tolerantly.
 * Responses are validated as envelopes only: their result shape depends on the
 * request, which the RPC layer checks.
 */
export function validateMessage(value: unknown, direction: Direction): Message {
  const strict = direction === 'host_to_core'
  if (!isObject(value)) throw new ProtocolError('message: expected an object')
  if (value.jsonrpc !== '2.0') throw new ProtocolError('message: jsonrpc must be "2.0"')
  const hasId = 'id' in value
  if (hasId) checkKind('message.id', value.id, 'u64')
  if ('method' in value) {
    checkShape('message', value, { jsonrpc: 'string', method: 'string' }, { id: 'u64', params: 'json' }, strict)
    const method = value.method as string
    const params = value.params ?? {}
    if (method === '$/cancel') {
      if (hasId) throw new ProtocolError('$/cancel must be a notification')
      checkShape('$/cancel', params, CANCEL_SHAPE, {}, strict)
      return value as Message
    }
    const spec = PARAMS[method]
    const expectedDir = direction === 'host_to_core' ? 'host' : 'core'
    if (!spec || spec.dir !== expectedDir) {
      throw new ProtocolError(`unknown ${direction} method \`${method}\``)
    }
    const isRequest = (direction === 'host_to_core' ? HOST_TO_CORE.requests : CORE_TO_HOST.requests).includes(
      method as never,
    )
    if (isRequest !== hasId) {
      throw new ProtocolError(`\`${method}\` must be ${isRequest ? 'a request (with id)' : 'a notification (no id)'}`)
    }
    checkShape(method, params, spec.required, spec.optional ?? {}, strict)
    spec.check?.(params, strict)
    return value as Message
  }
  if (!hasId) throw new ProtocolError('response: missing id')
  if ('error' in value) {
    checkShape('message', value, { jsonrpc: 'string', id: 'u64', error: 'object' }, {}, strict)
    checkShape('error', value.error, { code: 'number', message: 'string' }, { data: 'json' }, strict)
    return value as Message
  }
  if (!('result' in value)) throw new ProtocolError('response: needs result or error')
  checkShape('message', value, { jsonrpc: 'string', id: 'u64', result: 'json' }, {}, strict)
  return value as Message
}
