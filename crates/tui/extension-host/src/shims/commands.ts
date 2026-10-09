/**
 * The `commands` service: slash commands a plugin contributes.
 *
 * Two audiences share one `register`:
 *
 * - **Codewhale plugins** write
 *   `ctx.commands.register({ name, description, argumentHint?, handler })`.
 * - **DSH plugins** (`@deepseek-ai/dsh-commands`) write
 *   `ctx.commands.register({ name, description, input?: { hint }, handler })`
 *   and return `{ kind: 'success' | 'error', text }`. That surface is
 *   accepted as is.
 *
 * `register` returns an idempotent disposer, like `tools.register`; a
 * registration is an effect of the calling plugin's fiber, so unloading the
 * plugin removes it. The core admits or refuses each one (`registry/register`
 * with `kind: 'command'`): a name that collides with a built-in command or
 * another plugin's command is refused and activation fails with the reason.
 *
 * A handler is called only when the *user* runs the command, and it can only
 * return an answer; it cannot call the model, a tool or approval. Results:
 *
 * - a string, or `{ kind: 'success', text? }`: shown to the user;
 * - `{ kind: 'error', text }`: shown as a failure (a thrown error is too);
 * - `{ kind: 'submit', prompt, text? }` (Codewhale only): `prompt` becomes the
 *   user's next message and runs through the normal turn, tool approval
 *   included; `text` is shown beside it.
 *
 * The invocation carries `args` (what follows the name, trimmed), `rawInput`
 * (DSH's spelling: the text after the name including its leading separator,
 * so `' hello'`), `commandId`, `signal` (aborted when the core cancels the
 * call), an always-empty `attachments`, and the read-only strings `workspace`
 * (where the user ran the command) and `dataDir` (the plugin's own writable
 * directory). Not provided: DSH's `agent`
 * (the host has no agent or session handle), attachments (`input.attachments`
 * is refused), `recordInput`/`definitionId` (accepted, ignored: the core logs
 * nothing about a command), `list`/`find`/`execute`, and `sourceEventSeq`
 * (accepted in a result, ignored).
 */
import { Service } from '@deepseek-ai/cordis'
import type { CommandResultWire } from '../protocol.ts'
import type { OwnedEntry, OwnerBase } from './owned.ts'

/** DSH's command grammar. The core enforces the same one. */
const COMMAND_NAME = /^[a-z][a-z0-9_-]*$/u
const NO_ATTACHMENTS: readonly never[] = Object.freeze([])

/** A registration, from this host's side. */
export interface LocalCommand<O extends OwnerBase = OwnerBase> extends OwnedEntry<O> {
  definition: NormalizedCommand
}

export interface NormalizedCommand {
  name: string
  description: string
  argumentHint?: string
  handler: (invocation: CommandInvocation) => unknown
}

export interface CommandInvocation {
  readonly commandId: string
  /** What follows the command name, trimmed. */
  readonly args: string
  /** DSH spelling: what follows the name, with its leading separator. */
  readonly rawInput: string
  readonly attachments: readonly never[]
  readonly signal: AbortSignal
  /** The workspace the user ran the command in (absent if the core did not say). */
  readonly workspace?: string
  /** This plugin's own writable directory. */
  readonly dataDir?: string
  readonly sessionId?: string
  readonly agentId?: string
  readonly originTurnId?: string
}

/** Reject an invalid definition before it reaches the core, with a message that names the problem. */
export function normalizeCommand(definition: any): NormalizedCommand {
  if (!definition || typeof definition !== 'object') throw new TypeError('command definition must be an object')
  const { name } = definition
  if (typeof name !== 'string' || !COMMAND_NAME.test(name)) {
    throw new TypeError(`command name ${JSON.stringify(name)} must match ${String(COMMAND_NAME)}`)
  }
  if (typeof definition.description !== 'string' || definition.description.trim().length === 0) {
    throw new TypeError(`command "${name}" needs a non-empty description`)
  }
  if (typeof definition.handler !== 'function') throw new TypeError(`command "${name}" handler must be a function`)
  let hint: unknown = definition.argumentHint
  const input: unknown = definition.input
  if (input !== undefined) {
    if (typeof input !== 'object' || input === null || typeof (input as any).hint !== 'string') {
      throw new TypeError(`command "${name}" input hint must be a string`)
    }
    if ((input as any).attachments === true) {
      throw new TypeError(`command "${name}": attachments are not supported by the Codewhale extension host`)
    }
    hint ??= (input as any).hint
  }
  if (hint !== undefined && (typeof hint !== 'string' || hint.trim().length === 0)) {
    throw new TypeError(`command "${name}" argument hint must be a non-empty string`)
  }
  return {
    name,
    description: definition.description,
    ...(hint === undefined ? {} : { argumentHint: hint as string }),
    handler: definition.handler,
  }
}

/** What `registry/register` carries for a command. */
export function commandSpec(command: NormalizedCommand) {
  return {
    name: command.name,
    description: command.description,
    ...(command.argumentHint === undefined ? {} : { argument_hint: command.argumentHint }),
  }
}

export function makeInvocation(
  args: string,
  commandId: string,
  signal: AbortSignal,
  context: { workspace?: string; dataDir?: string; sessionId?: string; agentId?: string; originTurnId?: string } = {},
): CommandInvocation {
  return Object.freeze({
    commandId,
    args,
    rawInput: args === '' ? '' : ` ${args}`,
    attachments: NO_ATTACHMENTS,
    signal,
    ...context,
  })
}

/** Validate and detach whatever a handler returned. */
export function normalizeResult(command: string, value: unknown): CommandResultWire {
  if (value === undefined || value === null) return { kind: 'success' }
  if (typeof value === 'string') return { kind: 'success', text: value }
  if (typeof value !== 'object') throw new TypeError(`command "${command}" handler must return a result object or a string`)
  const result = value as { kind?: unknown; text?: unknown; prompt?: unknown }
  const text = result.text
  if (text !== undefined && typeof text !== 'string') {
    throw new TypeError(`command "${command}" result text must be a string when supplied`)
  }
  switch (result.kind) {
    case 'success':
      return text === undefined ? { kind: 'success' } : { kind: 'success', text }
    case 'error':
      if (typeof text !== 'string' || text.trim().length === 0) {
        throw new TypeError(`command "${command}" error text must be a non-empty string`)
      }
      return { kind: 'error', text }
    case 'submit':
      if (typeof result.prompt !== 'string' || result.prompt.trim().length === 0) {
        throw new TypeError(`command "${command}" submit prompt must be a non-empty string`)
      }
      return text === undefined ? { kind: 'submit', prompt: result.prompt } : { kind: 'submit', prompt: result.prompt, text }
    default:
      throw new TypeError(`command "${command}" returned unknown result kind ${JSON.stringify(result.kind)}`)
  }
}

/** The host's side of the shim: how a `register` call becomes a core registration. */
export interface CommandsHost<O extends OwnerBase> {
  ownerOf(ctx: any): O | undefined
  addCommand(owner: O, command: NormalizedCommand): () => void
}

/**
 * Build the `commands` service class for `host`. The class is frozen so one
 * plugin cannot rewrite `register` for the others (the owner token is not a
 * boundary between plugins that share the process; design §4.4).
 */
export function defineCommandsService<O extends OwnerBase>(host: CommandsHost<O>) {
  class CommandsShim extends Service {
    constructor(ctx: any) {
      super(ctx, 'commands')
    }

    /** `ctx.commands.register(definition)`: returns an idempotent disposer. */
    register(definition: unknown): () => void {
      const ctx: any = this.ctx
      const owner = host.ownerOf(ctx)
      if (!owner) throw new Error('commands.register called outside an extension owner')
      const command = normalizeCommand(definition)
      return ctx.effect(() => host.addCommand(owner, command), `commands.register(${JSON.stringify(command.name)})`)
    }
  }
  Object.freeze(CommandsShim.prototype)
  return CommandsShim
}
