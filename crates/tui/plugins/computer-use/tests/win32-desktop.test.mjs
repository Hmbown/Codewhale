// Opt in only on a disposable interactive Windows runner. Real UIA and screen
// APIs operate on our own WinForms fixture; no user app or raw input is touched.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import win32 from '../src/backends/win32.mjs';

test('Windows desktop: observe, set Unicode value, invoke and independently verify the selected window', {
  skip: process.platform !== 'win32' || process.env.CU_WINDOWS_DESKTOP_TESTS !== '1', timeout: 90_000,
}, async t => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'cu-win-desktop-'));
  const title = `CU fixture ${crypto.randomUUID()}`;
  const receipt = path.join(dir, 'state.json');
  const child = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', fileURLToPath(new URL('./fixtures/windows-desktop.ps1', import.meta.url)), title, receipt], { stdio: ['ignore', 'ignore', 'pipe'], windowsHide: false });
  let errors = ''; child.stderr.on('data', chunk => { errors += chunk; });
  const exited = new Promise(resolve => child.once('exit', resolve));
  t.after(async () => { if (child.exitCode === null) child.kill(); await exited; fs.rmSync(dir, { recursive: true, force: true }); });
  async function until(check) {
    const deadline = Date.now() + 15_000;
    while (Date.now() < deadline) {
      let result; try { result = check(); } catch {}
      if (result) return result;
      assert.equal(child.exitCode, null, errors);
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    assert.fail(`fixture deadline: ${errors}`);
  }
  const read = () => JSON.parse(fs.readFileSync(receipt, 'utf8'));
  await until(() => read().text === 'initial');
  const backend = win32.create();
  const displays = await backend.list_displays();
  assert.ok(displays.length >= 1);
  t.diagnostic(JSON.stringify({fixture:read(), windows:await backend.list_windows()}));
  const state = await backend.get_app_state({ app_ref: { name: title }, detail: 'full' });
  const edit = state.elements.find(e => e.role === 'Edit');
  const button = state.elements.find(e => e.label === 'Apply fixture');
  assert.ok(edit, JSON.stringify(state)); assert.ok(button, JSON.stringify(state));
  assert.equal(edit.value, 'initial');
  const target = el => ({ ...el, app_ref: { name: title }, windowIndex: 0 });
  const text = "A漢😀 O'Brien";
  assert.equal((await backend.set_value({ target: target(edit), value: text })).verified, true);
  await until(() => read().text === text);
  await backend.perform_action({ target: target(button), action: 'Invoke' });
  await until(() => read().clicks === 1);
  await assert.rejects(backend.set_value({ target: { ...target(edit), runtime_id: [123456789] }, value: 'wrong' }), /element_stale/);
  await assert.rejects(backend.set_value({ target: { ...target(edit), window_runtime_id: [123456789] }, value: 'wrong' }), /element_stale/);
  assert.equal(read().text, text); assert.equal(read().clicks, 1);
  const shot = await backend.screenshot({ display: 1, path: path.join(dir, 'screen.png') });
  const png = fs.readFileSync(shot.file);
  assert.equal(png.subarray(1, 4).toString(), 'PNG');
  assert.equal(png.readUInt32BE(16), shot.pixels.w); assert.equal(png.readUInt32BE(20), shot.pixels.h);
  assert.deepEqual(shot.points, displays[0].points);
  const region = [shot.points.x + 1, shot.points.y + 1, 20, 20];
  const crop = await backend.screenshot({ region, path: path.join(dir, 'crop.png') });
  assert.deepEqual(crop.points, { x: region[0], y: region[1], w: 20, h: 20 });
  t.diagnostic('Real WinForms UIA value/invoke/read-back, stale identity refusal, screen and region capture passed; no raw input or packaged Engine claim.');
});
