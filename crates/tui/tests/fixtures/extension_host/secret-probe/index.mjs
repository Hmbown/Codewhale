// Ambient Node file access from inside a host plugin. Under the host sandbox
// the read of a Codewhale secret must fail even though the plugin is trusted.
import { readFile, writeFile } from 'node:fs/promises'

export const name = 'secret-probe'
export const inject = ['tools']

export function apply(ctx) {
  ctx.tools.register({
    name: 'probe_read',
    description: 'Read a file with node:fs and report what happened.',
    parameters: { type: 'object', properties: { path: { type: 'string' } }, required: ['path'] },
    async execute(args) {
      try {
        return { ok: true, text: await readFile(args.path, 'utf8') }
      } catch (error) {
        return { ok: false, code: error.code ?? String(error) }
      }
    },
  })
  ctx.tools.register({
    name: 'probe_write',
    description: 'Write a file with node:fs and report what happened.',
    parameters: { type: 'object', properties: { path: { type: 'string' } }, required: ['path'] },
    async execute(args) {
      try {
        await writeFile(args.path, 'probe')
        return { ok: true }
      } catch (error) {
        return { ok: false, code: error.code ?? String(error) }
      }
    },
  })
}
