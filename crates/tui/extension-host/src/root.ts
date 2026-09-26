/**
 * The Cordis root that plugin fibers run under.
 *
 * The host has no turn loop, store, prompt authority or approval. Plugins
 * reach the core only through the shim services here, and every one of them
 * becomes a `registry/*` request that Rust admits or refuses. Service names
 * that belong to the core cannot be provided by a plugin at all.
 */
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { AsyncLocalStorage } from 'node:async_hooks'
import { pathToFileURL } from 'node:url'
import { Context, Inject, Service } from '@deepseek-ai/cordis'
import { ErrorCode, type ContentBlockWire, type Json, type OwnerRef, type ToolResultWire } from './protocol.ts'
import { RpcError, type RpcPeer } from './rpc.ts'

/** Context key carrying the owner record; inherited by every nested fiber. */
export const OWNER = Symbol.for('codewhale.extension-host.owner')

/**
 * Service names a plugin may never provide: each is one authority the Rust
 * core owns (§4.3 of the design). `tools` and `logger` are provided by the
 * host root as shims and are refused to plugins for the same reason.
 */
export const REFUSED_SERVICES = new Set([
  'approval',
  'agents',
  'sessions',
  'llm',
  'sandboxPolicy',
  'credentials',
  'fs',
  'subprocess',
  'systemPrompt',
  'tools',
  'commands',
  'skills',
  'logger',
])

/** Services the root provides; `inject` of anything else fails activation. */
const PROVIDED_SERVICES = new Set(['tools', 'logger', 'events', 'reflect', 'registry'])

const ACTIVATE_DEADLINE_MS = 5_000
const DISPOSE_DEADLINE_MS = 2_000

export interface OwnerRecord {
  ref: OwnerRef
  pluginName: string
  fibers: any[]
  /** In-flight `registry/register` requests, awaited before activation acks. */
  pendingRegistrations: Set<Promise<void>>
  refusals: string[]
  tools: Map<number, LocalTool>
  disposing?: Promise<void>
  state: 'activating' | 'active' | 'failed' | 'disposed'
}

interface LocalTool {
  owner: OwnerRecord
  name: string
  handle?: number
  definition: any
  disposed: boolean
}

export interface ActivateParams {
  owner: OwnerRef
  plugin_name: string
  entry: { path: string; sha256: string }
  config?: Json
}

export type ActivateResult = { status: 'ok'; tools: string[] } | { status: 'failed'; diagnostic: string }

export const ownerStorage = new AsyncLocalStorage<OwnerRecord>()

function describeError(error: unknown): string {
  if (error instanceof Error) return `${error.name}: ${error.message}`
  return String(error)
}

function withDeadline<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  let timer: NodeJS.Timeout
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${label} exceeded ${ms} ms`)), ms)
    timer.unref()
  })
  return Promise.race([promise, deadline]).finally(() => clearTimeout(timer))
}

function isJson(value: unknown, depth = 0): value is Json {
  if (depth > 64) return false
  if (value === null) return true
  switch (typeof value) {
    case 'boolean':
    case 'string':
      return true
    case 'number':
      return Number.isFinite(value)
    case 'object':
      if (Array.isArray(value)) return value.every((v) => isJson(v, depth + 1))
      if (Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null) return false
      return Object.values(value as object).every((v) => isJson(v, depth + 1))
    default:
      return false
  }
}

export class HostRoot {
  readonly root: any
  readonly owners = new Map<string, OwnerRecord>()
  private readonly toolsByHandle = new Map<number, LocalTool>()

  constructor(private readonly rpc: RpcPeer) {
    const root: any = new Context()
    this.root = root
    const host = this

    // Refuse core service names before any plugin can run. The refusal does
    // not depend on who calls: `ctx.root.provide(...)` runs with the root as
    // its context, so an owner check alone could be sidestepped. The one
    // exception is the host's own `tools` shim, provided once, below.
    const reflect = root.reflect
    const originalProvide = reflect.provide
    let toolsShimProvided = false
    reflect.provide = function (this: any, name: string, value: unknown, ...rest: unknown[]) {
      if (REFUSED_SERVICES.has(name)) {
        const hostShim = name === 'tools' && !toolsShimProvided && value instanceof ToolsShim
        if (!hostShim) {
          throw new Error(`extension may not provide core service \`${name}\`: the Codewhale core owns it`)
        }
        toolsShimProvided = true
      }
      return originalProvide.call(this, name, value, ...rest)
    }

    // Logger shim: every Cordis log line becomes a `log` notification.
    root.logger.exporter({
      colors: false,
      export: (message: any) => {
        const fiber = message.fiber?.deref?.()
        const owner: OwnerRecord | undefined = fiber?.ctx?.[OWNER]
        const text = (message.args ?? []).map((arg: unknown) => (arg instanceof Error ? describeError(arg) : typeof arg === 'string' ? arg : safeStringify(arg))).join(' ')
        host.log(message.type === 'error' ? 'error' : message.type === 'warn' ? 'warn' : 'info', `[${message.name}] ${text}`, owner)
      },
    })

    class ToolsShim extends Service {
      constructor(ctx: any) {
        super(ctx, 'tools')
      }

      /** DSH `ctx.tools.register(defineTool(...))`: returns an idempotent disposer. */
      register(definition: any) {
        const ctx: any = this.ctx
        const owner: OwnerRecord | undefined = ctx[OWNER]
        if (!owner) throw new Error('tools.register called outside an extension owner')
        validateDefinition(definition)
        return ctx.effect(() => host.addTool(owner, definition), `tools.register(${JSON.stringify(definition.name)})`)
      }
    }
    // Plugins share one process, so the owner token is not a boundary
    // between them (design §4.4, threat 3). Freezing the shim at least stops
    // the direct route of one plugin rewriting `register` for every other
    // plugin; shared globals remain, and the approval card says so.
    Object.freeze(ToolsShim.prototype)
    root.plugin(ToolsShim)
  }

  log(level: string, msg: string, owner?: OwnerRecord) {
    const params: Record<string, string> = { level, msg: msg.slice(0, 8192) }
    if (owner) params.plugin_id = owner.ref.plugin_id
    this.rpc.notify('log', params)
  }

  /** Called inside the owner's effect; returns the effect's cleanup. */
  private addTool(owner: OwnerRecord, definition: any): () => void {
    const local: LocalTool = { owner, name: definition.name, definition, disposed: false }
    const registration = this.rpc
      .request('registry/register', {
        owner: owner.ref,
        kind: 'tool',
        spec: {
          name: definition.name,
          description: String(definition.description ?? ''),
          input_schema: definition.parameters ?? { type: 'object', properties: {} },
        },
      })
      .then(
        (result: any) => {
          if (typeof result?.handle === 'number') {
            local.handle = result.handle
            if (local.disposed) {
              void this.unregister(local)
              return
            }
            owner.tools.set(result.handle, local)
            this.toolsByHandle.set(result.handle, local)
          } else {
            const reason = typeof result?.refused === 'string' ? result.refused : 'refused without a reason'
            owner.refusals.push(`tool \`${definition.name}\` refused: ${reason}`)
            if (owner.state === 'active') this.log('warn', `tool \`${definition.name}\` refused: ${reason}`, owner)
          }
        },
        (error: unknown) => {
          owner.refusals.push(`tool \`${definition.name}\` registration failed: ${describeError(error)}`)
        },
      )
      .finally(() => owner.pendingRegistrations.delete(registration))
    owner.pendingRegistrations.add(registration)
    return () => {
      if (local.disposed) return
      local.disposed = true
      if (local.handle !== undefined) void this.unregister(local)
    }
  }

  private async unregister(local: LocalTool) {
    const handle = local.handle!
    local.owner.tools.delete(handle)
    this.toolsByHandle.delete(handle)
    try {
      await this.rpc.request('registry/unregister', { owner: local.owner.ref, handle })
    } catch {
      // The core revokes first; a failed unregister after revocation is expected.
    }
  }

  async activate(params: ActivateParams): Promise<ActivateResult> {
    const key = params.owner.owner_token
    if (this.owners.has(key)) return { status: 'failed', diagnostic: 'owner token already active' }
    const owner: OwnerRecord = {
      ref: params.owner,
      pluginName: params.plugin_name,
      fibers: [],
      pendingRegistrations: new Set(),
      refusals: [],
      tools: new Map(),
      state: 'activating',
    }
    this.owners.set(key, owner)
    try {
      const bytes = await readFile(params.entry.path)
      const digest = createHash('sha256').update(bytes).digest('hex')
      if (digest !== params.entry.sha256) {
        throw new Error(`entry ${params.entry.path} changed after review (sha256 ${digest.slice(0, 12)}…)`)
      }
      const module = await ownerStorage.run(owner, () => import(pathToFileURL(params.entry.path).href))
      const plugin = pickPlugin(module)
      const missing = Object.keys(Inject.resolve(plugin.inject)).filter((name) => !PROVIDED_SERVICES.has(name))
      if (missing.length > 0) {
        throw new Error(`requires ${missing.map((name) => `\`${name}\``).join(', ')}, not provided by the Codewhale extension host in this phase`)
      }
      const ownerCtx = this.root.extend({ [OWNER]: owner })
      const fiber = ownerStorage.run(owner, () => ownerCtx.plugin(plugin, params.config ?? {}))
      owner.fibers.push(fiber)
      await withDeadline(Promise.resolve(fiber.await()), ACTIVATE_DEADLINE_MS, 'activation')
      const pending = pendingFibers(fiber)
      if (pending.length > 0) {
        throw new Error(`plugin fibers are waiting on services the host does not provide: ${pending.join(', ')}`)
      }
      while (owner.pendingRegistrations.size > 0) {
        await Promise.allSettled([...owner.pendingRegistrations])
      }
      if (owner.refusals.length > 0) throw new Error(owner.refusals.join('; '))
      owner.state = 'active'
      return { status: 'ok', tools: [...owner.tools.values()].map((tool) => tool.name).sort() }
    } catch (error) {
      owner.state = 'failed'
      // All-or-nothing: dispose the partial fiber, rolling back every registration.
      await this.disposeOwner(owner).catch(() => undefined)
      this.owners.delete(key)
      return { status: 'failed', diagnostic: describeError(error) }
    }
  }

  /** Dispose one owner's fibers (reverse order, async disposers awaited). Memoised. */
  disposeOwner(owner: OwnerRecord): Promise<void> {
    owner.disposing ??= (async () => {
      for (const fiber of [...owner.fibers].reverse()) {
        await fiber.dispose()
      }
      owner.state = 'disposed'
    })()
    return owner.disposing
  }

  async deactivate(ref: OwnerRef): Promise<{ disposed: boolean; leaked: string[] }> {
    const owner = this.owners.get(ref.owner_token)
    if (!owner) return { disposed: true, leaked: [] }
    let disposed = true
    try {
      await withDeadline(this.disposeOwner(owner), DISPOSE_DEADLINE_MS, 'dispose')
    } catch {
      disposed = false
    }
    const leaked = [...owner.tools.values()].map((tool) => `tool:${tool.name}`)
    for (const fiber of owner.fibers) {
      for (const effect of fiber.getEffects?.() ?? []) leaked.push(`effect:${effect.label}`)
    }
    for (const tool of owner.tools.values()) this.toolsByHandle.delete(tool.handle!)
    this.owners.delete(ref.owner_token)
    return { disposed, leaked }
  }

  async deactivateAll(deadlineMs: number) {
    await withDeadline(
      Promise.allSettled([...this.owners.values()].map((owner) => this.deactivate(owner.ref))),
      deadlineMs,
      'shutdown',
    ).catch(() => undefined)
  }

  async callTool(handle: number, input: unknown, callId: string, signal: AbortSignal): Promise<ToolResultWire> {
    const local = this.toolsByHandle.get(handle)
    if (!local || local.disposed || local.owner.state !== 'active') {
      throw new RpcError(ErrorCode.NotAvailable, `tool handle ${handle} is not live`)
    }
    const definition = local.definition
    const aborted = new Promise<never>((_, reject) => {
      const onAbort = () => reject(new RpcError(ErrorCode.Cancelled, 'cancelled'))
      if (signal.aborted) onAbort()
      else signal.addEventListener('abort', onAbort, { once: true })
    })
    const run = ownerStorage.run(local.owner, async () => {
      const value = await definition.execute(input, { signal, callId, args: input })
      return renderResult(definition, input, value)
    })
    return Promise.race([run, aborted])
  }
}

function renderResult(definition: any, input: unknown, value: unknown): ToolResultWire {
  let blocks: unknown = undefined
  if (typeof definition.output?.render === 'function') {
    blocks = definition.output.render(input, value)
  }
  const content: ContentBlockWire[] = []
  if (Array.isArray(blocks)) {
    for (const block of blocks) {
      if (block && typeof block === 'object' && (block as any).type === 'text' && typeof (block as any).text === 'string') {
        content.push({ type: 'text', text: (block as any).text })
      }
    }
  } else {
    content.push({ type: 'text', text: typeof value === 'string' ? value : safeStringify(value) })
  }
  const result: ToolResultWire = { content, is_error: false }
  if (value !== undefined && isJson(value)) result.structured = value
  return result
}

function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value) ?? String(value)
  } catch {
    return String(value)
  }
}

function validateDefinition(definition: any) {
  if (!definition || typeof definition !== 'object') throw new TypeError('tool definition must be an object')
  if (typeof definition.name !== 'string' || definition.name.length === 0) throw new TypeError('tool definition needs a name')
  if (typeof definition.execute !== 'function') throw new TypeError(`tool \`${definition.name}\` needs an execute function`)
  if (definition.parameters !== undefined && (typeof definition.parameters !== 'object' || definition.parameters === null)) {
    throw new TypeError(`tool \`${definition.name}\` parameters must be a JSON schema object`)
  }
}

function pickPlugin(module: any): any {
  const candidate = module?.default ?? module
  if (typeof candidate === 'function') return candidate
  if (candidate && typeof candidate.apply === 'function') return candidate
  if (module && typeof module.apply === 'function') return module
  throw new Error('entry module exports no Cordis plugin (a function, or an object with `apply`)')
}

/** Names of services that nested fibers under `fiber` are still waiting on. */
function pendingFibers(fiber: any): string[] {
  const out: string[] = []
  const seen = new Set<any>()
  const visit = (current: any) => {
    if (!current || seen.has(current)) return
    seen.add(current)
    if (current.state === 0 /* PENDING */) {
      const names = Object.keys(current.inject ?? {})
      out.push(`${current.name ?? 'plugin'} (${names.join(', ')})`)
    }
  }
  visit(fiber)
  for (const runtime of fiber.ctx?.registry?.values?.() ?? []) {
    for (const child of runtime.fibers ?? []) {
      if (child.parent?.[OWNER] === fiber.ctx?.[OWNER]) visit(child)
    }
  }
  return out
}
