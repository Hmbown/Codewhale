import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { PetEngineTelemetry } from '../dist/core/pet-engine.js';
import { PetNative } from '../dist/core/pet-native.js';
import { compilePetTelemetry, encodePetJSONL } from '../dist/core/pet-telemetry.js';
import { petDemoEvents } from '../dist/core/pet-demo.js';

const points = JSON.stringify(readFileSync(new URL('../public/whale-points.tsv', import.meta.url), 'utf8').trim().split('\n').map(row => row.trim().split(/\s+/).map(Number)));
test('action receipts distinguish tool names without inventing shell contents and expire', () => {
  const e = new PetEngineTelemetry();
  for (const [i, name, kind] of [[0,'read_file','reading'],[1,'search_files','searching'],[2,'apply_patch','editing'],[3,'exec_command','executing'],[4,'run_tests','testing'],[5,'browser_navigate','browsing']]) {
    e.observe({event:'turn_started'},i*1000);
    e.observe({event:'tool_call_started',tool_call_id:'private-id',tool_name:name},i*1000);
    const activity=e.activity(i*1000+100);
    assert.equal(activity.kind,kind); assert.equal(activity.tool,name);
    assert.equal(JSON.stringify(activity).includes('private-id'),false);
    assert.equal(e.activity(i*1000+801).observed,false);
  }
});
test('waiting and concurrent action receipts stay bounded and clear after disconnect', () => {
  const pet=new PetNative(points,'','[]',true);
  pet.observeEngineBatch(JSON.stringify([{event:'tool_call_started',tool_call_id:'a',tool_name:'read_file'},
    ...Array.from({length:3},(_,i)=>({event:'agent_spawned',id:'private-'+i}))]),0);
  pet.advanceEngine(200,true,false);
  const frame=JSON.parse(pet.presentation());
  assert.equal(frame.activity.kind,'reading');assert.equal(frame.activity.parallel,3);
  assert.equal(JSON.stringify(frame.activity).includes('private-'),false);
  pet.observeEngine(JSON.stringify({event:'approval_required',id:'private'}),200);
  pet.advanceEngine(600,true,true);assert.equal(JSON.parse(pet.presentation()).activity.kind,'waiting');
  const before=pet.recording(true);pet.presentation();assert.equal(pet.recording(true),before);
  pet.disconnectEngine();assert.equal(JSON.parse(pet.presentation()).activity.observed,false);
});
test('the incremental bucket range uses the same measured projection as full replay', () => {
  const events = petDemoEvents(), full = compilePetTelemetry(events, 80_000);
  for (let i = 0; i < full.length; i++) assert.deepEqual(compilePetTelemetry(events, 80_000, i)[0], full[i]);
});
test('Engine pulses expire; a late failed completion tears at receipt time without rewriting history', () => {
  const engine = new PetEngineTelemetry();
  engine.observe({ event: 'tool_call_started', tool_call_id: 'a', tool_name: 'exec_command' }, 0);
  engine.observe({ event: 'tool_call_heartbeat' }, 300);
  const first = engine.bucket(0);
  assert.equal(first.channel, 'code'); assert.equal(first.activeMs[3], 300);
  assert.equal(engine.bucket(3).observed, 0);
  engine.observe({ event: 'tool_call_complete', tool_call_id: 'a', tool_name: 'exec_command', failed: true }, 5900);
  assert.equal(engine.bucket(13).observed, 0);
  assert.equal(engine.bucket(14).channel, 'error'); assert.equal(engine.bucket(14).errors, 1);
  assert.deepEqual(engine.bucket(0), first);
});
test('an authoritative waiting request escalates, accepted tape replays, and silence stays unknown', () => {
  const pet = new PetNative(points, '', '[]', true);
  pet.observeEngine(JSON.stringify({ event: 'approval_required', id: 'permission' }), 100);
  for (let ms = 400; ms <= 30_000; ms += 400) pet.advanceEngine(ms, true, true);
  assert.equal(JSON.parse(pet.snapshot()).needs, 'call');
  assert.equal(JSON.parse(pet.snapshot()).state.channel, 'human');
  const saved = JSON.parse(pet.recording()), replay = new PetNative(points, encodePetJSONL(saved.tape), JSON.stringify(saved.interactions));
  for (let i = 0; i < 900; i++) replay.step(1 / 30, true);
  assert.deepEqual(JSON.parse(replay.snapshot()).state, JSON.parse(pet.snapshot()).state);
  assert.equal(JSON.parse(replay.snapshot()).digest, JSON.parse(pet.snapshot()).digest);
  pet.observeEngine(JSON.stringify({ event: 'turn_complete' }), 30_100);
  pet.advanceEngine(32_000, true, false);
  assert.equal(JSON.parse(pet.snapshot()).state.observed, 0);
});
test('Engine worker boundary rejects payloads, invalid clocks and oversized active sets', () => {
  const e = new PetEngineTelemetry();
  assert.throws(() => e.observe({ event: 'response_delta', index: 0, delta: 'private' }, 0));
  assert.throws(() => e.observe({ event: 'response_delta', index: 0, channel: 'fabricated' }, 0));
  e.observe({ event: 'thinking_started', index: 0 }, 100);
  assert.throws(() => e.observe({ event: 'thinking_complete', index: 0 }, 90));
  for (let i = 1; i < 256; i++) e.observe({ event: 'thinking_started', index: i }, 100);
  assert.throws(() => e.observe({ event: 'thinking_started', index: 257 }, 100));
});

test('failed shared metadata batches accept no partial spans or private input', () => {
  const control = new PetNative(points, '', '[]', true), pet = new PetNative(points, '', '[]', true);
  assert.throws(() => pet.observeEngineBatch(JSON.stringify([{event:'thinking_started',index:1},{event:'response_delta',index:1,content:'PRIVATE'}]),0));
  pet.advanceEngine(400,true,false); control.advanceEngine(400,true,false);
  assert.deepEqual(JSON.parse(pet.recording(true)),JSON.parse(control.recording(true)));
  pet.observeEngineBatch(JSON.stringify([{event:'thinking_started',index:1},{event:'response_delta',index:1,channel:'reasoning'}]),400);
  pet.advanceEngine(800,true,false);
  assert.equal(JSON.parse(pet.snapshot()).state.channel,'reasoning');
});

test('successive shared batches preserve active and waiting coverage exactly', () => {
  let shared = new PetEngineTelemetry();
  const direct = new PetEngineTelemetry();
  for (const [at, event, waiting] of [
    [0, {event:'tool_call_started', tool_call_id:'build', tool_name:'exec_command'}, false],
    [300, {event:'tool_call_heartbeat'}, false],
    [600, {event:'tool_call_heartbeat'}, false],
    [800, {event:'tool_call_complete', tool_call_id:'build'}, false],
    [900, {event:'approval_required', id:'permission'}, true],
    [1200, {event:'agent_spawned', id:'worker'}, true],
    [1500, {event:'agent_progress', id:'worker', worker_status:'running'}, true],
  ]) {
    shared = shared.clone();
    for (const engine of [shared, direct]) {
      engine.observe(event, at);
      engine.confirmWaiting(at, waiting);
    }
    for (let bucket=0; bucket<=Math.floor(at/400); bucket++)
      assert.deepEqual(shared.bucket(bucket), direct.bucket(bucket));
  }
});

test('shared presentation and still projections cannot change physics, clock, journal or score', () => {
  const pet = new PetNative(points, '', '[]', true), control = new PetNative(points, '', '[]', true);
  for(let i=1;i<=120;i++) {
    pet.advanceEngine(i*1000/30,true,false); control.advanceEngine(i*1000/30,true,false);
    const before=pet.recording(true), frame=JSON.parse(pet.presentation());
    assert.equal(frame.points.length,980);assert.equal(frame.still.points.length,980);
    for(let view=0;view<3;view++) pet.presentation();
    assert.equal(pet.recording(true),before);
  }
  assert.deepEqual(JSON.parse(pet.recording(true)),JSON.parse(control.recording(true)));
});
