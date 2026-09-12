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
 * Only storage completion is controlled: this tests the async caller contract,
 * not IndexedDB's transaction implementation or browser rendering. */
async function browser() {
  const nodes = new Map(), saves = [], intervals = [];
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
      if (saves.length === 1) return new Promise((resolve, reject) => { complete = resolve; fail = reject; });
      return Promise.resolve(saves.length);
    }
  }
  const points = await readFile(new URL('../public/whale-points.tsv', import.meta.url), 'utf8');
  // Imports bind to the real core above, while the controller body stays intact.
  const controller = (await readFile(new URL('../dist/ui/pet.js', import.meta.url), 'utf8')).replace(/^import .*;\r?\n/gm, '');
  const environment = { ...world, ...telemetry, ...demo, ...ingest, ...sim, ...audio, TraceLibrary,
    document: { hidden: false, getElementById: node, addEventListener() {} },
    window: { addEventListener() {}, confirm() { confirmations++; return false; } },
    matchMedia: () => ({ matches: false, addEventListener() {} }),
    fetch: async () => ({ ok: true, text: async () => points }),
    Option: class {}, requestAnimationFrame() {}, setInterval: callback => intervals.push(callback),
    setTimeout, clearTimeout, structuredClone, TextEncoder, devicePixelRatio: 1 };
  await vm.runInNewContext(`(async () => { ${controller}\n })()`, environment);
  assert.equal(intervals.length, 1, 'Controller starts its autosave after loading');
  return { node, saves, autosave: intervals[0], complete: () => complete(1), fail: () => fail(new Error('Disk unavailable')),
    confirmations: () => confirmations };
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
