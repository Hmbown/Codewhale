/**
 * Module resolution for plugin code: one Cordis, one schemastery, one cosmokit.
 *
 * DSH packages import these by bare specifier, as peers or (for schemastery,
 * sometimes) as plain dependencies. Two copies of Cordis break `instanceof`,
 * symbols and services, so every import of these specifiers — however the
 * package declared it, and ignoring any copy under the package's own
 * `node_modules` — resolves to the single instance bundled into this host.
 *
 * `@deepseek-ai/dsh-tools` resolves to the definition-side compat module.
 * Any other `@deepseek-ai/dsh-*` package fails the import loudly: the host
 * does not provide it, and a silent partial load would be worse.
 */
import { registerHooks } from 'node:module'

const SCHEME = 'codewhale-host:'
const REGISTRY_KEY = Symbol.for('codewhale.extension-host.modules')

/** Specifier → singleton key. */
const SINGLETONS: Record<string, string> = {
  '@deepseek-ai/cordis': 'cordis',
  '@deepseek-ai/schemastery': 'schemastery',
  '@deepseek-ai/cosmokit': 'cosmokit',
  cosmokit: 'cosmokit',
  '@deepseek-ai/dsh-tools': 'dsh-tools',
  '@deepseek-ai/dsh-util-values': 'dsh-util-values',
}

export class UnsupportedPeerError extends Error {
  constructor(readonly specifier: string) {
    super(`requires \`${specifier}\`, which the Codewhale extension host does not provide`)
    this.name = 'UnsupportedPeerError'
  }
}

function packageName(specifier: string): string {
  const parts = specifier.split('/')
  return specifier.startsWith('@') ? parts.slice(0, 2).join('/') : parts[0]
}

/** Map a bare specifier to a singleton key, `null` for "not ours", or throw for an unsupported DSH peer. */
export function classifySpecifier(specifier: string): string | null {
  if (specifier.startsWith('.') || specifier.startsWith('/') || specifier.includes(':')) return null
  const name = packageName(specifier)
  if (name in SINGLETONS) {
    if (specifier !== name) throw new UnsupportedPeerError(specifier)
    return SINGLETONS[name]
  }
  if (name.startsWith('@deepseek-ai/dsh-') || name.startsWith('@deepseek-ai/cordis-')) {
    throw new UnsupportedPeerError(name)
  }
  return null
}

function virtualSource(key: string, namespace: Record<string, unknown>): string {
  const lines = [`const ns = globalThis[Symbol.for(${JSON.stringify(REGISTRY_KEY.description)})][${JSON.stringify(key)}];`]
  for (const name of Object.keys(namespace)) {
    if (name === 'default') continue
    if (!/^[A-Za-z_$][\w$]*$/.test(name)) continue
    lines.push(`export const ${name} = ns[${JSON.stringify(name)}];`)
  }
  if ('default' in namespace) lines.push('export default ns.default;')
  return lines.join('\n')
}

let installed = false

/**
 * Install the hooks once, publishing `modules` (key → module namespace) as the
 * singletons plugin code will see.
 */
export function installResolveHooks(modules: Record<string, Record<string, unknown>>) {
  if (installed) return
  installed = true
  ;(globalThis as any)[REGISTRY_KEY] = modules
  registerHooks({
    resolve(specifier, context, nextResolve) {
      const key = classifySpecifier(specifier)
      if (key !== null) {
        if (!(key in modules)) throw new UnsupportedPeerError(specifier)
        return { url: `${SCHEME}${key}`, format: 'module', shortCircuit: true }
      }
      return nextResolve(specifier, context)
    },
    load(url, context, nextLoad) {
      if (url.startsWith(SCHEME)) {
        const key = url.slice(SCHEME.length)
        return { format: 'module', source: virtualSource(key, modules[key]), shortCircuit: true }
      }
      return nextLoad(url, context)
    },
  })
}
