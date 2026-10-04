/** A value that survives `JSON.stringify` unchanged and is accepted on the wire. */
import type { Json } from './protocol.ts'

export function isJson(value: unknown, depth = 0): value is Json {
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
