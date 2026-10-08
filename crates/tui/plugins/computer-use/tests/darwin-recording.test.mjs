import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { create } from '../src/backends/darwin.mjs';
import { withSignal } from '../src/exec.mjs';

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cu-recording-owner-'));
  const oldBundle = process.env.CODEWHALE_CU_APP_BUNDLE;
  const oldDir = process.env.CODEWHALE_CU_RECORDINGS_DIR;
  t.after(() => {
    if (oldBundle === undefined) delete process.env.CODEWHALE_CU_APP_BUNDLE; else process.env.CODEWHALE_CU_APP_BUNDLE = oldBundle;
    if (oldDir === undefined) delete process.env.CODEWHALE_CU_RECORDINGS_DIR; else process.env.CODEWHALE_CU_RECORDINGS_DIR = oldDir;
    fs.rmSync(root, { recursive: true, force: true });
  });
  const helper = path.join(root, 'Contents', 'MacOS', 'accessibility');
  fs.mkdirSync(path.dirname(helper), { recursive: true });
  fs.writeFileSync(helper, `#!${process.execPath}\n` + `
import fs from 'node:fs';
import path from 'node:path';
const {args} = JSON.parse(process.argv[2]);
const stop = () => { if(args.durationSec === 8) return; fs.writeFileSync(args.file, 'finalized'); process.exit(0); };
process.on('SIGINT', stop); process.stdin.resume(); process.stdin.on('end', stop);
fs.writeFileSync(path.join(path.dirname(args.file), '..', 'handler-installed'), '');
fs.writeFileSync(path.join(path.dirname(args.file), '..', 'last-request.json'), process.argv[2]);
fs.writeFileSync(args.file, 'partial');
if(args.durationSec !== 7) console.log(JSON.stringify({ready:true}));
setInterval(()=>{},1000);
`);
  // No extension: force ESM through package metadata for the fixture launcher.
  fs.writeFileSync(path.join(root, 'package.json'), '{"type":"module"}');
  fs.chmodSync(helper, 0o700);
  process.env.CODEWHALE_CU_APP_BUNDLE = root;
  process.env.CODEWHALE_CU_RECORDINGS_DIR = path.join(root, 'recordings');
  const make = (ownerPipe = 1) => create({ exec: { async run(_cmd, args) {
    const tool = JSON.parse(args[0]).tool;
    if (tool === 'input_capabilities') return {code:0,stdout:JSON.stringify({record_owner_pipe:ownerPipe}),stderr:''};
    assert.equal(tool, 'displays');
    return { code: 0, stdout: JSON.stringify([{index:1,id:1}]), stderr: '' };
  } } });
  return { root, make };
}

test('an old recorder helper is refused before any recording process starts', {skip:process.platform==='win32'}, async t => {
  const { make, root } = fixture(t);
  await assert.rejects(make(0).recordingStart(), /update Computer Use before recording/);
  assert.deepEqual(fs.readdirSync(path.join(root, 'recordings')), []);
});

test('closing a session stops only its recorders and retains finalized files', {skip:process.platform==='win32'}, async t => {
  const { make } = fixture(t);
  const owner = make(), other = make();
  const first = await owner.recordingStart();
  const second = await other.recordingStart();
  t.after(() => Promise.allSettled([owner.closeSession(), other.closeSession()]));
  await owner.releaseInput();
  assert.equal((await owner.recordingStatus({id:first.id})).running, true, 'input cancellation does not stop recording');
  await owner.closeSession();
  assert.equal(fs.readFileSync(first.file, 'utf8'), 'finalized');
  assert.equal((await other.recordingStatus({id:second.id})).running, true);
  await other.closeSession();
  assert.equal(fs.readFileSync(second.file, 'utf8'), 'finalized');
});

test('recording startup cancellation owns and stops the pending child', {skip:process.platform==='win32'}, async t => {
  const { make, root } = fixture(t);
  const owner = make(), controller = new AbortController();
  const started = withSignal(controller.signal, () => owner.recordingStart({durationSec:7}));
  const rejected = assert.rejects(started, error => error.code === 'cancelled');
  const dir = path.join(root, 'recordings');
  const handlersReady = path.join(root, 'handler-installed');
  const deadline = Date.now() + 2000;
  while ((!fs.existsSync(handlersReady) || !fs.existsSync(dir) || !fs.readdirSync(dir).length) && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 10));
  assert.ok(fs.existsSync(handlersReady), 'fake helper installed cancellation handlers');
  controller.abort();
  await rejected;
  assert.equal((await owner.recordingList()).running.length, 0);
  const files = fs.readdirSync(dir);
  assert.equal(files.length, 1);
  assert.equal(fs.readFileSync(path.join(dir, files[0]), 'utf8'), 'finalized');
});

test('session close bounds a stubborn recorder and retains its partial file', {skip:process.platform==='win32'}, async t => {
  const { make } = fixture(t);
  const owner = make();
  const recording = await owner.recordingStart({durationSec:8});
  const started = Date.now();
  await assert.rejects(owner.closeSession(), /partial file retained/);
  assert.ok(Date.now() - started < 3000);
  let alive = true;
  const deadline = Date.now() + 500;
  while (alive && Date.now() < deadline) {
    try { process.kill(recording.pid, 0); } catch (error) { assert.equal(error.code, 'ESRCH'); alive = false; }
    if (alive) await new Promise(resolve => setTimeout(resolve, 10));
  }
  assert.equal(alive, false, 'the stubborn capture process was terminated');
  assert.equal(fs.readFileSync(recording.file, 'utf8'), 'partial');
  assert.equal((await owner.recordingList()).running.length, 0);
});

test('app_ref recording crops to the window rect on its hosting display', {skip:process.platform==='win32'}, async t => {
  const { root } = fixture(t);
  const requests = [];
  const b = create({ exec: { async run(_cmd, args) {
    const req = JSON.parse(args[0]);
    requests.push(req.tool);
    if (req.tool === 'input_capabilities') return {code:0,stdout:JSON.stringify({record_owner_pipe:1}),stderr:''};
    if (req.tool === 'window_info') return {code:0,stdout:JSON.stringify({window_id:77,name:'FakeWin',points:{x:100,y:50,w:800,h:600}}),stderr:''};
    assert.equal(req.tool, 'displays');
    return { code: 0, stdout: JSON.stringify([{index:1,id:1,points:{x:0,y:0,w:2880,h:1620}},{index:2,id:2,points:{x:2880,y:0,w:1920,h:1080}}]), stderr: '' };
  } } });
  const rec = await b.recordingStart({ app_ref: { name: 'FakeApp' } });
  t.after(() => b.closeSession().catch(() => {}));
  assert.equal(rec.window.name, 'FakeWin');
  assert.equal(rec.display, 1, 'window center selects the hosting display');
  const sent = JSON.parse(fs.readFileSync(path.join(root, 'last-request.json'), 'utf8'));
  assert.equal(sent.tool, 'record');
  assert.deepEqual(sent.args.region, [100, 50, 800, 600]);
  assert.equal(sent.args.displayID, 1);
  await assert.rejects(b.recordingStart({ app_ref: { name: 'FakeApp' }, region: [0, 0, 10, 10] }), /not both/);
});

test('recording_list returns jpeg screenshots alongside recordings', {skip:process.platform==='win32'}, async t => {
  const { make, root } = fixture(t);
  const dir = path.join(root, 'recordings');
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'shot-a.jpg'), 'x');
  fs.writeFileSync(path.join(dir, 'shot-b.png'), 'x');
  fs.writeFileSync(path.join(dir, 'notes.txt'), 'x');
  const list = await make().recordingList();
  assert.deepEqual(list.recordings.map((r) => path.basename(r.file)).sort(), ['shot-a.jpg', 'shot-b.png']);
});

test('native recording startup notices owner pipe EOF without capturing a screen', {skip:process.platform!=='darwin'}, async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cu-recording-pipe-'));
  t.after(() => fs.rmSync(root, {recursive:true, force:true}));
  const source = path.join(root, 'probe.m'), binary = path.join(root, 'probe');
  const header = path.resolve('src/backends/darwin-recording.h');
  fs.writeFileSync(source, `#import <Cocoa/Cocoa.h>\n#import <ApplicationServices/ApplicationServices.h>\n#import ${JSON.stringify(header)}\nint main(){@autoreleasepool{cuRecordingOwnerPipe=YES;puts("ready");fflush(stdout);@try{cuWait(dispatch_semaphore_create(0),15,YES);return 2;}@catch(NSException *e){return [e.name isEqual:@"cancelled"]?0:3;}}}`);
  const build = spawnSync('clang', ['-fobjc-arc','-Os','-framework','Cocoa','-framework','ApplicationServices','-framework','ScreenCaptureKit','-framework','AVFoundation','-framework','CoreMedia',source,'-o',binary], {encoding:'utf8'});
  assert.equal(build.status, 0, build.stderr);
  const child = spawn(binary, [], {stdio:['pipe','pipe','pipe']});
  t.after(() => {if(child.exitCode===null)child.kill('SIGKILL');});
  await new Promise(resolve => child.stdout.once('data', resolve));
  const exited = new Promise(resolve => child.once('exit', resolve));
  child.stdin.end();
  assert.equal(await exited, 0);
});
