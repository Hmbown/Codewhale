import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { create } from '../src/backends/darwin.mjs';

test('native Unicode encoding round-trips through the actual CoreGraphics event', {skip:process.platform!=='darwin'}, t=>{
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),'cu-native-test-'));t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));
  const binary=path.join(dir,'native');
  const build=spawnSync('clang',['-DCU_TEST=1','-fobjc-arc','-Os','-framework','Cocoa','-framework','ApplicationServices','-framework','ScreenCaptureKit','-framework','AVFoundation','-framework','CoreMedia','src/backends/darwin-accessibility.m','-o',binary],{encoding:'utf8'});
  assert.equal(build.status,0,build.stderr);
  for(const text of ['Hello 世界 🐋','quote " slash \\ newline\n','e\u0301 👨‍👩‍👧‍👦']){
    const r=spawnSync(binary,[JSON.stringify({tool:'inspect_text_event',args:{text}})],{encoding:'utf8'});
    assert.equal(r.status,0,r.stderr);assert.equal(JSON.parse(r.stdout).text,text);
  }
});
test('macOS backend binds native input to the opened process and reports denied permissions honestly', async t=>{
  const bundle=fs.mkdtempSync(path.join(os.tmpdir(),'cu-bundle-test-'));const old=process.env.CODEWHALE_CU_APP_BUNDLE;
  t.after(()=>{if(old===undefined)delete process.env.CODEWHALE_CU_APP_BUNDLE;else process.env.CODEWHALE_CU_APP_BUNDLE=old;fs.rmSync(bundle,{recursive:true,force:true});});
  fs.mkdirSync(path.join(bundle,'Contents','MacOS'),{recursive:true});fs.writeFileSync(path.join(bundle,'Contents','MacOS','accessibility'),'');process.env.CODEWHALE_CU_APP_BUNDLE=bundle;
  const calls=[];
  const backend=create({exec:{async run(cmd,args,opts){
    assert.ok(Number.isFinite(opts.timeoutMs));
    if(cmd==='open')return {code:0,stdout:'',stderr:''};
    if(cmd==='screencapture')return {code:1,stdout:'',stderr:'denied'};
    const request=JSON.parse(args[0]);calls.push(request);
    return {code:0,stderr:'',stdout:JSON.stringify(request.tool==='app_info'?{found:true,pid:123,bundle_id:'test.app'}:request.tool==='permissions'?{trusted:false}:{action_sent:true})};
  }}});
  await backend.open_application({name:'TextEdit',activate:false});await backend.key({text:'cmd+n'});await backend.type({text:'Hello 世界 🐋'});
  const events=calls.filter(c=>c.tool==='key_event');assert.equal(events.length,2);assert.equal(events[0].args.code,45);assert.equal(events[0].args.flags,1<<20);assert.equal(events[0].args.input_app_ref.pid,123);assert.equal(events[1].args.down,false);
  assert.equal(calls.find(c=>c.tool==='type').args.text,'Hello 世界 🐋');
  const probe=await backend.probe();assert.equal(probe.permissions.accessibility,'denied');assert.equal(probe.capabilities.raw_input,false);assert.equal(probe.capabilities.screenshot,false);
});

test('macOS background binding avoids reopen and releases at the agent pointer, not the user pointer', async t=>{
  const bundle=fs.mkdtempSync(path.join(os.tmpdir(),'cu-quiet-test-'));const old=process.env.CODEWHALE_CU_APP_BUNDLE;
  t.after(()=>{if(old===undefined)delete process.env.CODEWHALE_CU_APP_BUNDLE;else process.env.CODEWHALE_CU_APP_BUNDLE=old;fs.rmSync(bundle,{recursive:true,force:true});});
  fs.mkdirSync(path.join(bundle,'Contents','MacOS'),{recursive:true});fs.writeFileSync(path.join(bundle,'Contents','MacOS','accessibility'),'');process.env.CODEWHALE_CU_APP_BUNDLE=bundle;
  const calls=[];
  const backend=create({exec:{async run(cmd,args){
    assert.notEqual(cmd,'open','binding a running app must not reopen its windows');
    const request=JSON.parse(args[0]);calls.push(request);
    assert.notEqual(request.tool,'cursor_position','release must not sample the physical pointer');
    const body=request.tool==='app_info'?{found:true,pid:123,bundle_id:'test.app'}
      :request.tool==='window_at_point'?{found:true,owner_pid:123,owner_name:'TextEdit',window_id:9,layer:0}
      :{action_sent:true};
    return {code:0,stderr:'',stdout:JSON.stringify(body)};
  }}});
  await backend.open_application({name:'TextEdit'});
  assert.equal(calls[0].args.activate,false);
  await assert.rejects(backend.left_mouse_up({}),/no agent pointer/);
  await backend.left_mouse_down({target:{x:100,y:200}});
  await backend.mouse_move({target:{x:140,y:250}});
  await backend.left_mouse_up({});
  const release=calls.at(-1);
  assert.equal(release.tool,'pointer_sequence');
  assert.equal(release.args.steps.length,1);
  assert.equal(release.args.steps[0].type,2,'left mouse up');
  assert.deepEqual([release.args.steps[0].x,release.args.steps[0].y],[140,250],'release lands at the agent pointer');
  assert.equal(release.args.restore,false,'a held button is not put back');
  assert.equal(release.args.input_app_ref.pid,123);
  assert.ok(!calls.some(c=>c.tool==='preview_notify'),'background actions do not open preview');
});

/** Backend wired to a scripted native helper; returns the requests it made. */
function stubBackend(t, reply) {
  const bundle = fs.mkdtempSync(path.join(os.tmpdir(), 'cu-hit-test-'));
  const old = process.env.CODEWHALE_CU_APP_BUNDLE;
  t.after(() => { if (old === undefined) delete process.env.CODEWHALE_CU_APP_BUNDLE; else process.env.CODEWHALE_CU_APP_BUNDLE = old; fs.rmSync(bundle, { recursive: true, force: true }); });
  fs.mkdirSync(path.join(bundle, 'Contents', 'MacOS'), { recursive: true });
  fs.writeFileSync(path.join(bundle, 'Contents', 'MacOS', 'accessibility'), '');
  process.env.CODEWHALE_CU_APP_BUNDLE = bundle;
  const calls = [];
  const backend = create({ exec: { async run(cmd, args) {
    const request = JSON.parse(args[0]);
    calls.push(request);
    const body = request.tool === 'app_info' ? { found: true, pid: 321, bundle_id: 'test.app' }
      : reply(request)
      ?? (request.tool === 'window_at_point' ? { found: true, owner_pid: 321, owner_name: 'TextEdit', window_id: 9, layer: 0 }
        : { action_sent: true, restored: true });
    return { code: 0, stderr: '', stdout: JSON.stringify(body) };
  } } });
  return { backend, calls };
}

const PRESSABLE = { found: true, element: { role: 'AXButton', label: 'Tab B', actions: ['AXPress'] }, action: 'AXPress', action_sent: true };
const NOT_PRESSABLE = { found: false, reason: 'no_pressable_element' };

test('macOS coordinate left_click prefers the accessibility element under the point', async (t) => {
  const { backend, calls } = stubBackend(t, (r) => (r.tool === 'hit_test' ? PRESSABLE : null));
  await backend.open_application({ name: 'TextEdit' });
  const receipt = await backend.left_click({ target: { x: 220, y: 180 } });
  assert.equal(receipt.strategy, 'a11y');
  assert.equal(receipt.action, 'AXPress');
  assert.equal(receipt.element.label, 'Tab B');
  const hit = calls.find((c) => c.tool === 'hit_test');
  assert.deepEqual([hit.args.x, hit.args.y, hit.args.perform], [220, 180, true]);
  assert.equal(hit.args.input_app_ref.pid, 321);
  assert.ok(!calls.some((c) => c.tool === 'mouse_event'), 'a semantic press must not also post raw pointer events');
});

test('macOS coordinate left_click falls back to a guarded global gesture when no element is pressable', async (t) => {
  const { backend, calls } = stubBackend(t, (r) => (r.tool === 'hit_test' ? NOT_PRESSABLE : null));
  await backend.open_application({ name: 'TextEdit' });
  const receipt = await backend.left_click({ target: { x: 40, y: 90 } });
  assert.equal(receipt.strategy, 'event');
  assert.equal(receipt.pointer_moved, true, 'the receipt admits the real cursor moved');
  assert.equal(receipt.a11y_reason, 'no_pressable_element');

  const guard = calls.find((c) => c.tool === 'window_at_point');
  assert.deepEqual([guard.args.x, guard.args.y], [40, 90], 'ownership of the landing point is checked first');
  const seq = calls.find((c) => c.tool === 'pointer_sequence');
  assert.deepEqual(seq.args.steps.map((s) => s.type), [5, 1, 2], 'move, down, up in one gesture');
  assert.deepEqual([seq.args.steps[1].x, seq.args.steps[1].y, seq.args.steps[1].clickState], [40, 90, 1]);
  assert.equal(seq.args.restore, true, 'the user gets their pointer back');
});

test('macOS refuses a global gesture whose landing point belongs to another application', async (t) => {
  const { backend, calls } = stubBackend(t, (r) => (r.tool === 'hit_test' ? NOT_PRESSABLE
    : r.tool === 'window_at_point' ? { found: true, owner_pid: 999, owner_name: 'Mail', window_id: 4, layer: 0 } : null));
  await backend.open_application({ name: 'TextEdit' });
  await assert.rejects(backend.left_click({ target: { x: 40, y: 90 } }), /covered by a window owned by Mail/);
  assert.ok(!calls.some((c) => c.tool === 'pointer_sequence'), 'nothing is posted into the other application');
});

test('macOS left_click strategies: event skips the tree, a11y fails closed, other clicks stay pointer-driven', async (t) => {
  const { backend, calls } = stubBackend(t, (r) => (r.tool === 'hit_test' ? NOT_PRESSABLE : null));
  await backend.open_application({ name: 'TextEdit' });

  const forced = await backend.left_click({ target: { x: 10, y: 20 }, strategy: 'event' });
  assert.equal(forced.strategy, 'event');
  assert.ok(!calls.some((c) => c.tool === 'hit_test'), 'strategy=event never hit-tests');

  await assert.rejects(backend.left_click({ target: { x: 10, y: 20 }, strategy: 'a11y' }), /no pressable accessibility element/);
  await assert.rejects(backend.left_click({ target: { x: 10, y: 20 }, strategy: 'sideways' }), /strategy must be auto, a11y or event/);

  calls.length = 0;
  const dbl = await backend.double_click({ target: { x: 10, y: 20 } });
  assert.equal(dbl.strategy, 'event');
  assert.deepEqual(calls.find((c) => c.tool === 'pointer_sequence').args.steps.map((s) => s.clickState), [0, 1, 1, 2, 2]);
  assert.equal((await backend.right_click({ target: { x: 10, y: 20 } })).strategy, 'event');
  assert.ok(!calls.some((c) => c.tool === 'hit_test'), 'only a left single click has an accessibility equivalent');
});

test('macOS drag and scroll travel as one gesture that puts the pointer back', async (t) => {
  const { backend, calls } = stubBackend(t, () => null);
  await backend.open_application({ name: 'TextEdit' });

  const drag = await backend.left_click_drag({ from_target: { x: 10, y: 10 }, to: { x: 110, y: 10 } });
  assert.equal(drag.pointer_moved, true);
  const dragSeq = calls.find((c) => c.tool === 'pointer_sequence');
  assert.equal(dragSeq.args.restore, true);
  assert.equal(dragSeq.args.steps.at(-1).type, 2, 'released at the destination');
  assert.deepEqual([dragSeq.args.steps.at(-1).x, dragSeq.args.steps.at(-1).y], [110, 10]);

  calls.length = 0;
  await backend.scroll({ target: { x: 10, y: 10 }, direction: 'down', amount: 3 });
  const scrollSeq = calls.find((c) => c.tool === 'pointer_sequence');
  const notches = scrollSeq.args.steps.filter((s) => s.scroll);
  assert.equal(notches.length, 3, 'one notch per unit of amount, like a real wheel');
  assert.deepEqual(notches[0].scroll, [0, -1]);
  assert.equal(scrollSeq.args.restore, true);
});

test('native hit_test fails closed without a bound application', { skip: process.platform !== 'darwin' }, (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'cu-hit-native-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const binary = path.join(dir, 'native');
  const build = spawnSync('clang', ['-DCU_TEST=1', '-fobjc-arc', '-Os', '-framework', 'Cocoa', '-framework', 'ApplicationServices', '-framework', 'ScreenCaptureKit', '-framework', 'AVFoundation', '-framework', 'CoreMedia', 'src/backends/darwin-accessibility.m', '-o', binary], { encoding: 'utf8' });
  assert.equal(build.status, 0, build.stderr);
  const r = spawnSync(binary, [JSON.stringify({ tool: 'hit_test', args: { x: 1, y: 1, perform: true } })], { encoding: 'utf8' });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /open_application first/);
});
