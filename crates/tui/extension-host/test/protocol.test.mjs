// Protocol conformance: the shared corpus that the Rust side also parses.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { FIXTURES } from './harness.mjs'
import { FrameDecoder, encodeFrame, validateMessage, MAGIC, MAX_FRAME } from '../dist/protocol.mjs'

const corpusDir = join(FIXTURES, 'protocol')
const corpus = readdirSync(corpusDir)
  .filter((name) => name.endsWith('.json'))
  .sort()
  .map((name) => ({ name, ...JSON.parse(readFileSync(join(corpusDir, name), 'utf8')) }))

test('the corpus is non-trivial in both directions', () => {
  for (const direction of ['host_to_core', 'core_to_host']) {
    assert.ok(corpus.some((c) => c.direction === direction && c.valid), `${direction} valid cases`)
    assert.ok(corpus.some((c) => c.direction === direction && !c.valid), `${direction} invalid cases`)
  }
})

for (const entry of corpus) {
  test(`corpus ${entry.name}: ${entry.valid ? 'parses and round-trips' : 'is rejected'}`, () => {
    if (!entry.valid) {
      assert.throws(() => validateMessage(entry.frame, entry.direction))
      return
    }
    validateMessage(entry.frame, entry.direction)
    const [decoded] = new FrameDecoder().push(encodeFrame(entry.frame))
    assert.deepEqual(decoded, entry.frame)
    validateMessage(decoded, entry.direction)
  })
}

test('frames split across chunks decode once, in order', () => {
  const bytes = Buffer.concat([encodeFrame({ a: 1 }), encodeFrame({ b: 'ü' })])
  const decoder = new FrameDecoder()
  const out = []
  for (let i = 0; i < bytes.length; i += 3) out.push(...decoder.push(bytes.subarray(i, i + 3)))
  assert.deepEqual(out, [{ a: 1 }, { b: 'ü' }])
})

test('bad magic, oversized length, and non-JSON payloads are framing errors', () => {
  assert.throws(() => new FrameDecoder().push(Buffer.from('NOPE\x00\x00\x00\x00')), /magic/)
  const huge = Buffer.alloc(8)
  MAGIC.copy(huge)
  huge.writeUInt32LE(MAX_FRAME + 1, 4)
  assert.throws(() => new FrameDecoder().push(huge), /MAX_FRAME/)
  const bad = Buffer.alloc(9)
  MAGIC.copy(bad)
  bad.writeUInt32LE(1, 4)
  bad.write('{', 8)
  assert.throws(() => new FrameDecoder().push(bad), /not JSON/)
})
