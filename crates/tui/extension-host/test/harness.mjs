// A minimal fake core: spawns the committed host bundle and speaks CWX1 to it.
// Tests import only dist/ — no type stripping, no install.
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { FrameDecoder, encodeFrame, validateMessage } from '../dist/protocol.mjs'

const here = dirname(fileURLToPath(import.meta.url))
export const BUNDLE = join(here, '..', 'dist', 'codewhale-extension-host.mjs')
export const FIXTURES = join(here, '..', '..', 'tests', 'fixtures', 'extension_host')

export function sha256File(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex')
}

export const LIMITS = { max_frame: 32 * 1024 * 1024, max_inflight: 256, dispose_deadline_ms: 2000, activate_deadline_ms: 5000 }

/**
 * Start a host. `admit(spec, owner)` decides `registry/register`: return a
 * handle number or `{ refused }`. Registry traffic is recorded in `registry`.
 */
export async function startHost({ admit, env, ownGroup = false } = {}) {
  const started = performance.now()
  // `ownGroup` spawns the host as a process-group leader and tells it so, as
  // the Rust core does on Unix.
  const child = spawn(process.execPath, ['--max-old-space-size=256', '--disable-proto=throw', '--no-addons', BUNDLE], {
    stdio: ['pipe', 'pipe', 'pipe'],
    env: { ...process.env, ...env, ...(ownGroup ? { CODEWHALE_HOST_PROCESS_GROUP: '1' } : {}) },
    detached: ownGroup,
  })
  const decoder = new FrameDecoder()
  const pending = new Map()
  const waiters = []
  const host = {
    child,
    stderr: '',
    logs: [],
    faulted: [],
    registry: [],
    hello: null,
    nextId: 1,
    nextHandle: 1,
    exit: new Promise((resolve) => child.on('exit', (code, signal) => resolve({ code, signal }))),
    send(message) {
      validateMessage(message, 'core_to_host')
      child.stdin.write(encodeFrame(message))
    },
    request(method, params) {
      const id = host.nextId++
      const promise = new Promise((resolve, reject) => pending.set(id, { resolve, reject }))
      host.send({ jsonrpc: '2.0', id, method, params })
      return { id, promise }
    },
    call(method, params) {
      return host.request(method, params).promise
    },
    cancel(id) {
      host.send({ jsonrpc: '2.0', method: '$/cancel', params: { id } })
    },
    waitFor(predicate, timeoutMs = 5000) {
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('timed out waiting for host message')), timeoutMs)
        waiters.push({ predicate, resolve: (value) => { clearTimeout(timer); resolve(value) } })
      })
    },
    async stop() {
      if (child.exitCode === null && child.signalCode === null) {
        child.stdin.end()
        await host.exit
      }
    },
  }
  child.stderr.on('data', (chunk) => { host.stderr += chunk.toString() })
  child.stdout.on('data', (chunk) => {
    for (const raw of decoder.push(chunk)) {
      const message = validateMessage(raw, 'host_to_core')
      for (let i = waiters.length - 1; i >= 0; i--) {
        if (waiters[i].predicate(message)) waiters.splice(i, 1)[0].resolve(message)
      }
      if ('method' in message) {
        switch (message.method) {
          case 'host/hello':
            host.hello = message.params
            break
          case 'log':
            host.logs.push(message.params)
            break
          case 'ext/faulted':
            host.faulted.push(message.params)
            break
          case 'registry/register': {
            host.registry.push({ op: 'register', ...message.params })
            const verdict = admit ? admit(message.params.spec, message.params.owner) : undefined
            const result = verdict && typeof verdict === 'object' ? verdict : { handle: verdict ?? host.nextHandle++ }
            if ('handle' in result) host.registry.at(-1).handle = result.handle
            host.send({ jsonrpc: '2.0', id: message.id, result })
            break
          }
          case 'registry/unregister':
            host.registry.push({ op: 'unregister', ...message.params })
            host.send({ jsonrpc: '2.0', id: message.id, result: {} })
            break
        }
      } else {
        const waiter = pending.get(message.id)
        if (waiter) {
          pending.delete(message.id)
          if ('error' in message) waiter.reject(Object.assign(new Error(message.error.message), { code: message.error.code }))
          else waiter.resolve(message.result)
        }
      }
    }
  })
  await host.waitFor((m) => m.method === 'host/hello')
  const ready = host.waitFor((m) => m.method === 'host/ready')
  await host.call('host/initialize', { protocol: 1, limits: LIMITS })
  await ready
  host.readyMs = performance.now() - started
  return host
}

let token = 0
export function owner(pluginId) {
  token += 1
  return { plugin_id: pluginId, generation: 1, owner_token: `token-${pluginId}-${token}-${'0'.repeat(24)}` }
}

export async function activate(host, fixture, entryPath = join(FIXTURES, fixture, 'index.mjs')) {
  const ref = owner(fixture)
  const result = await host.call('ext/activate', {
    owner: ref,
    plugin_name: fixture,
    entry: { path: entryPath, sha256: sha256File(entryPath) },
    config: {},
  })
  return { ref, result }
}
