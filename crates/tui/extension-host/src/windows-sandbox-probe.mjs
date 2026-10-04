// Fixed Core-selected startup diagnostic. No plugin or provider code executes.
import fs from 'node:fs'
import net from 'node:net'
import childProcess from 'node:child_process'

const denied = (operation) => {
  try { operation() } catch (error) {
    if (error?.code === 'EACCES' || error?.code === 'EPERM') return true
    throw error
  }
  return false
}

export async function windowsSandboxProbe(env = process.env) {
  const inside = env.CODEWHALE_WINDOWS_PROBE_INSIDE
  const outside = env.CODEWHALE_WINDOWS_PROBE_OUTSIDE
  const reads = JSON.parse(env.CODEWHALE_WINDOWS_PROBE_READS ?? '[]')
  const port = Number(env.CODEWHALE_WINDOWS_PROBE_PORT)
  if (!inside || !outside || reads.length < 3 || !Number.isInteger(port) || port <= 0 || port > 65535) throw new Error('invalid Core sandbox probe projection')
  const marker = 'codewhale-windows-isolation-probe'
  fs.writeFileSync(inside, marker, { flag: 'wx' })
  if (fs.readFileSync(inside, 'utf8') !== marker) throw new Error('sandbox data roundtrip failed')
  fs.unlinkSync(inside)
  if (!denied(() => fs.writeFileSync(outside, marker, { flag: 'wx' }))) throw new Error('sandbox allowed an outside write')
  for (const path of reads) {
    if (!denied(() => fs.readFileSync(path))) throw new Error('sandbox allowed an outside credential read')
  }
  // The Core keeps a real listening socket alive. A timeout, ENOENT, refusal,
  // or unreachable address cannot be mistaken for network isolation.
  await new Promise((resolve, reject) => {
    const socket = net.connect({ host: '127.0.0.1', port })
    const timer = setTimeout(() => { socket.destroy(); reject(new Error('sandbox network probe timed out')) }, 3000)
    socket.once('connect', () => { clearTimeout(timer); socket.destroy(); reject(new Error('sandbox allowed direct network')) })
    socket.once('error', (error) => {
      clearTimeout(timer); socket.destroy()
      if (error.code === 'EACCES' || error.code === 'EPERM') resolve()
      else reject(new Error(`network probe failed without an access denial: ${error.code}`))
    })
  })
  const receipt = { version: 1, data_roundtrip: true, outside_read_denied: true, outside_write_denied: true, network_denied: true, descendant_denied: true }
  if (env.CODEWHALE_WINDOWS_PROBE_DESCENDANT !== '1') {
    const args = JSON.parse(env.CODEWHALE_WINDOWS_PROBE_CHILD_ARGS ?? 'null')
    if (!Array.isArray(args) || args.some((arg) => typeof arg !== 'string')) throw new Error('missing Core child probe argv')
    await new Promise((resolve, reject) => {
      const child = childProcess.spawn(process.execPath, args, { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true, env: {
        ...env, CODEWHALE_WINDOWS_PROBE_DESCENDANT: '1', CODEWHALE_WINDOWS_PROBE_INSIDE: `${inside}.child`,
      } })
      let output = '', bytes = 0
      const finish = (error) => { clearTimeout(timer); if (error) { child.kill(); reject(error) } else resolve() }
      const timer = setTimeout(() => finish(new Error('sandbox descendant probe timed out')), 6000)
      child.stdout.on('data', (chunk) => {
        bytes += chunk.length
        if (bytes > 4096) finish(new Error('sandbox descendant output exceeds 4096 bytes'))
        else output += chunk.toString()
      })
      // Drain stderr without retaining arbitrary output.
      child.stderr.on('data', (chunk) => {
        bytes += chunk.length
        if (bytes > 4096) finish(new Error('sandbox descendant output exceeds 4096 bytes'))
      })
      child.once('error', (error) => finish(error))
      child.once('close', (code, signal) => {
        try {
          if (code !== 0 || signal || JSON.stringify(JSON.parse(output)) !== JSON.stringify(receipt)) throw new Error('sandbox descendant has no exact denial receipt')
          finish()
        } catch (error) { finish(error) }
      })
    })
  }
  return receipt
}
