import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';
import * as world from '../dist/core/pet-world.js';
import * as telemetry from '../dist/core/pet-telemetry.js';
import * as demo from '../dist/core/pet-demo.js';
import * as ingest from '../dist/core/ingest.js';
import * as sim from '../dist/core/pet-sim.js';
import * as audio from '../dist/core/pet-audio.js';

/** Run the compiled browser controller and real world with minimal DOM sinks.
 * Storage and file-read completion are controlled: this tests async caller
 * contracts, not IndexedDB transactions, file-picker grants or rendering. */
async function browser({ deferFirstSave = true, handle } = {}) {
  const nodes = new Map(), saves = [], intervals = [], timers = [], listeners = new Map();
  let complete, fail, confirmations = 0;
  const context = new Proxy({}, { get: (object, key) => object[key] ?? (() => {}) });
  const node = id => {
    if (!nodes.has(id)) nodes.set(id, { value: id === 'mode' ? 'wild' : '', textContent: '', checked: false,
      clientWidth: 800, clientHeight: 400, getContext: () => context,
      setAttribute() {}, addEventListener() {}, replaceChildren() {}, add() {},
      querySelector: () => node('replay-option'), getBoundingClientRect: () => ({ left: 0, top: 0, width: 800, height: 400 }) });
    return nodes.get(id);
  };
  class TraceLibrary {
    async getHabitat() { return undefined; }
    async petArchives() { return []; }
    saveHabitat(habitat, revision, archive) {
      saves.push(structuredClone({ habitat, revision, archive }));
      if (deferFirstSave && saves.length === 1) return new Promise((resolve, reject) => { complete = resolve; fail = reject; });
      return Promise.resolve(saves.length);
    }
  }
  // Windows checkouts can use CRLF; exercise that asset in the actual controller.
  const points = (await readFile(new URL('../public/whale-points.tsv', import.meta.url), 'utf8')).replace(/\r?\n/g, '\r\n');
  // Imports bind to the real core above, while the controller body stays intact.
  const controller = (await readFile(new URL('../dist/ui/pet.js', import.meta.url), 'utf8')).replace(/^import .*;\r?\n/gm, '');
  const environment = { ...world, ...telemetry, ...demo, ...ingest, ...sim, ...audio, TraceLibrary,
    document: { hidden: false, getElementById: node, addEventListener: (name, callback) => listeners.set(name, callback) },
    window: { addEventListener() {}, confirm() { confirmations++; return false; },
      showOpenFilePicker: handle ? async () => [handle] : undefined,
      setTimeout: callback => timers.push(callback) },
    matchMedia: () => ({ matches: false, addEventListener() {} }),
    fetch: async () => ({ ok: true, text: async () => points }),
    Option: class {}, requestAnimationFrame() {}, setInterval: callback => intervals.push(callback),
    setTimeout, clearTimeout, structuredClone, TextEncoder, devicePixelRatio: 1 };
  const inspect = await vm.runInNewContext(`(async () => { ${controller}\n return () => world.recording(true); })()`, environment);
  assert.equal(intervals.length, 1, 'Controller starts its autosave after loading');
  return { node, saves, autosave: intervals[0], complete: () => complete(1), fail: () => fail(new Error('Disk unavailable')),
    confirmations: () => confirmations, recording: () => structuredClone(inspect()),
    poll: () => { assert.ok(timers.length); return timers.shift()(); },
    visibility: hidden => { environment.document.hidden = hidden; listeners.get('visibilitychange')(); } };
}

test('browser source change waits for autosave and archives inputs accepted during that save', async () => {
  const page = await browser();
  page.autosave();
  assert.equal(page.saves.length, 1);
  page.node('feed').onclick();
  page.node('mode').value = 'demo';
  let finished = false;
  const changing = page.node('mode').onchange().then(() => { finished = true; });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(page.confirmations(), 0, 'An in-flight save is not a failed save');
  assert.equal(finished, false);
  assert.equal(page.saves.length, 1, 'Wait for the current revision before archiving');
  page.complete(); await changing;
  assert.equal(page.saves.length, 2);
  assert.equal(page.saves[1].revision, 1);
  assert.equal(page.saves[1].archive.interactions.length, 1);
  assert.equal(page.saves[1].archive.interactions[0].kind, 'food');
  assert.equal(page.saves[1].archive.source, 'wild');
  assert.equal(page.node('mode').value, 'demo');
});

test('browser failed autosave asks before leaving and cancel preserves the current source', async () => {
  const page = await browser();
  page.autosave(); page.node('mode').value = 'demo';
  const changing = page.node('mode').onchange();
  page.fail(); await changing;
  assert.equal(page.confirmations(), 1);
  assert.equal(page.node('mode').value, 'wild');
  assert.equal(page.saves.length, 1, 'No archive or replacement after the failed write');
});

test('browser live follow discards a read crossing suspension and primes the resumed file', async () => {
  const human = telemetry.compilePetTelemetry(demo.petDemoEvents()).find(b => b.waiting);
  const file = sequence => {
    const text = telemetry.encodePetJSONL([{ ...human, sequence, simTimeMs: sequence * 400 }]);
    return { name: 'local.jsonl', size: text.length, slice: () => ({ text: async () => text }) };
  };
  let current = file(10), pending;
  const handle = { getFile: async () => current };
  const page = await browser({ deferFirstSave: false, handle });
  await page.node('follow').onclick();
  assert.equal(page.recording().tape.length, 1, 'Existing file is not a live observation');
  handle.getFile = () => new Promise(resolve => { pending = resolve; });
  const reading = page.poll();
  page.visibility(true); page.visibility(false);
  pending(file(11)); await reading;
  assert.equal(page.recording().tape.length, 1, 'Do not prime from a read started before suspension');
  handle.getFile = async () => current;
  current = file(12); await page.poll();
  assert.equal(page.recording().tape.length, 1, 'The first resumed read establishes the new baseline');
  current = file(13); await page.poll();
  assert.equal(page.recording().tape.at(-1).channel, 'human');
  assert.equal(page.recording().tape.length, 2);
  await page.poll(); assert.equal(page.recording().tape.length, 2, 'Duplicate reads add no onsets');
});
