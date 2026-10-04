// Fixed diagnostic logic, not a Windows/kernel isolation receipt.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import net from 'node:net'
import cp from 'node:child_process'
import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { windowsSandboxProbe } from '../src/windows-sandbox-probe.mjs'

const receipt = { version: 1, data_roundtrip: true, outside_read_denied: true, outside_write_denied: true, network_denied: true, descendant_denied: true }
const env = {
  CODEWHALE_WINDOWS_PROBE_INSIDE: '/inside/marker', CODEWHALE_WINDOWS_PROBE_OUTSIDE: '/outside/write',
  CODEWHALE_WINDOWS_PROBE_READS: JSON.stringify(['/codex/auth', '/dsh/credentials', '/builtin/state']),
  CODEWHALE_WINDOWS_PROBE_PORT: '12345', CODEWHALE_WINDOWS_PROBE_CHILD_ARGS: JSON.stringify(['fixed-probe']),
}
function controls(t, options = {}) {
  let value
  const originals = [fs.writeFileSync, fs.readFileSync, fs.unlinkSync, net.connect, cp.spawn]
  t.after(() => { [fs.writeFileSync, fs.readFileSync, fs.unlinkSync, net.connect, cp.spawn] = originals })
  const denied = (code) => Object.assign(new Error(code), { code })
  fs.writeFileSync = (path, content) => {
    if (path === env.CODEWHALE_WINDOWS_PROBE_INSIDE) { value = content; return }
    if (options.outsideWrite) return
    throw denied('EACCES')
  }
  fs.readFileSync = (path) => {
    if (path === env.CODEWHALE_WINDOWS_PROBE_INSIDE) return value
    throw denied(options.readError ?? 'EACCES')
  }
  fs.unlinkSync = () => { value = undefined }
  net.connect = () => {
    const socket = new EventEmitter()
    socket.destroy = () => {}
    if (!options.timeout) process.nextTick(() => {
      if (options.networkConnect) socket.emit('connect')
      else socket.emit('error', denied(options.networkError ?? 'EACCES'))
    })
    return socket
  }
  cp.spawn = (program, args, options) => {
    assert.equal(program, process.execPath)
    assert.deepEqual(args, ['fixed-probe'])
    assert.equal(options.env.CODEWHALE_WINDOWS_PROBE_DESCENDANT, '1')
    const child = new EventEmitter()
    child.stdout = new PassThrough(); child.stderr = new PassThrough(); child.kill = () => true
    process.nextTick(() => { child.stdout.write(JSON.stringify(receipt)); child.emit('close', 0, null) })
    return child
  }
}

test('fixed probe requires exact own-data and parent/descendant denial receipt', async (t) => {
  controls(t)
  assert.deepEqual(await windowsSandboxProbe(env), receipt)
})
test('outside write success is an admission failure', async (t) => {
  controls(t, { outsideWrite: true })
  await assert.rejects(windowsSandboxProbe(env), /allowed an outside write/)
})
test('a missing credential file cannot count as filesystem isolation', async (t) => {
  controls(t, { readError: 'ENOENT' })
  await assert.rejects(windowsSandboxProbe(env), /ENOENT/)
})
test('a reachable loopback connection is an admission failure', async (t) => {
  controls(t, { networkConnect: true })
  await assert.rejects(windowsSandboxProbe(env), /allowed direct network/)
})
test('an unavailable listener cannot count as network isolation', async (t) => {
  controls(t, { networkError: 'ECONNREFUSED' })
  await assert.rejects(windowsSandboxProbe(env), /without an access denial: ECONNREFUSED/)
})
test('network timeout fails admission instead of passing a denial', async (t) => {
  controls(t, { timeout: true })
  const timeout = globalThis.setTimeout
  t.after(() => { globalThis.setTimeout = timeout })
  globalThis.setTimeout = (callback, after, ...args) => {
    assert.equal(after, 3000)
    queueMicrotask(() => callback(...args))
    return 0
  }
  await assert.rejects(windowsSandboxProbe(env), /network probe timed out/)
})
test('invalid Core projection fails before side effects', async () => {
  await assert.rejects(windowsSandboxProbe({}), /invalid Core sandbox probe projection/)
})
test('actual unrestricted Node is rejected by the fixed write control', async () => {
  const root = fs.mkdtempSync(join(tmpdir(), 'cw-win-probe-negative-'))
  const inside = join(root, 'inside'), outside = join(root, 'outside')
  const reads = ['codex', 'dsh', 'builtin'].map((name) => join(root, name))
  try {
    reads.forEach((path) => fs.writeFileSync(path, 'non-secret-control'))
    await assert.rejects(windowsSandboxProbe({ ...env, CODEWHALE_WINDOWS_PROBE_INSIDE: inside,
      CODEWHALE_WINDOWS_PROBE_OUTSIDE: outside, CODEWHALE_WINDOWS_PROBE_READS: JSON.stringify(reads) }), /allowed an outside write/)
    assert.equal(fs.existsSync(outside), true, 'real unrestricted write must prove the negative control is exercised')
  } finally { fs.rmSync(root, { recursive: true, force: true }) }
})
