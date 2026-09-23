// Win32 backend tests that run on ANY host via the injectable runner seam
// (`create({ exec })`): the fake runner captures the PowerShell scripts so the
// generated commands can be asserted directly, and no real powershell.exe is
// ever spawned. These pin the failure-truthful and self-contained-action
// behavior ported from the codewhale-side hardening (injectable runner +
// truthful PowerShell failure reporting) without needing a Windows host or a
// fake-powershell.exe-on-PATH fixture.
import { test } from "node:test";
import assert from "node:assert/strict";

function decodeScript(args) {
  const i = args.indexOf("-EncodedCommand");
  if (i === -1) return null;
  return Buffer.from(args[i + 1], "base64").toString("utf16le");
}

function mockExec({ fail = false } = {}) {
  const calls = [];
  const run = async (_cmd, args) => {
    calls.push({ script: decodeScript(args) });
    if (fail) return { code: 1, stdout: "", stderr: "simulated powershell failure" };
    return { code: 0, stdout: '{"ok": true}\n', stderr: "" };
  };
  return { run, calls };
}

test("win32: actions run through an injected runner (no powershell needed)", async () => {
  const { run, calls } = mockExec();
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run, persistentInputOwner: true } });
  const r = await b.left_click({ target: { x: 5, y: 6 } });
  assert.equal(r.action_sent, true);
  assert.ok(calls.length >= 1, "the injected runner must receive the action command");
});

test("win32: input actions fail truthfully on a nonzero exit", async () => {
  const { run } = mockExec({ fail: true });
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run, persistentInputOwner: true } });
  await assert.rejects(() => b.left_click({ target: { x: 1, y: 2 } }), /exited 1/);
  await assert.rejects(() => b.left_mouse_down({ target: { x: 1, y: 2 } }), /exited 1/);
});

test("win32: coordinate clicks refuse strategy=a11y instead of silently degrading", async () => {
  const { run, calls } = mockExec();
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run, persistentInputOwner: true } });
  // left_click refuses synchronously (it is not async), like every fail-closed
  // guard in this backend — see backends.test.mjs for the same convention.
  assert.throws(() => b.left_click({ target: { x: 1, y: 2 }, strategy: "a11y" }), /macOS-only/);
  assert.equal(calls.length, 0, "no command may be spawned for a refused strategy");
  // auto and event remain accepted on this backend.
  assert.equal((await b.left_click({ target: { x: 1, y: 2 }, strategy: "event" })).action_sent, true);
});

test("win32: targeted left_mouse_down both moves and presses, self-contained", async () => {
  const { run, calls } = mockExec();
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run, persistentInputOwner: true } });
  await b.left_mouse_down({ target: { x: 12, y: 34 } });
  const script = calls.at(-1).script;
  // Self-contained: the User32 P/Invoke type travels with the action.
  assert.ok(script.includes("public static class User32"), "action must define User32 in its own process");
  assert.ok(script.includes("SetCursorPos(12, 34)"), "must move the cursor to the target");
  assert.ok(script.includes("LEFTDOWN"), "must press the left button");
});

test("win32: every User32 action carries the type prelude in-process", async () => {
  const { run, calls } = mockExec();
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run, persistentInputOwner: true } });
  await b.mouse_move({ target: { x: 3, y: 4 } });
  await b.key({ text: "a" });
  assert.ok(calls.length >= 2, "two actions should have run through the injected runner");
  for (const call of calls) {
    assert.ok(call.script.includes("public static class User32"), "each action must redefine User32 in-process");
  }
});

test("win32: typing rejects timeout, cancellation and process errors even with success output", async () => {
  const mod = await import("../src/backends/win32.mjs");
  for (const [result, expected] of [
    [{ code: 0, timedOut: true }, /timed out/],
    [{ code: 0, aborted: true }, /cancelled/],
    [{ code: 1, stderr: "SendInput inserted 0 of 2 events" }, /inserted 0 of 2/],
  ]) {
    const b = mod.create({ exec: { run: async () => ({ stdout: '{"ok":true}', stderr: "", ...result }) } });
    await assert.rejects(b.type({ text: "A中😀" }), expected);
  }
});

test("win32: Unicode text is data in the shared input script, including shell-looking text", async () => {
  const { run, calls } = mockExec();
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run } });
  const text = "A中😀'; throw 'should stay text'; $env:SECRET";
  assert.deepEqual(await b.type({ text }), { action_sent: true, chars: text.length, strategy: "unicode-sendinput" });
  const script = calls[0].script;
  assert.ok(!script.includes(text), "text must not become PowerShell source");
  const encoded = script.match(/FromBase64String\('([^']+)'\)/)?.[1];
  assert.equal(Buffer.from(encoded, "base64").toString("utf16le"), text);
  assert.match(script, /\[User32\]::SendString\(\$text\)/);
  assert.ok(!script.includes("class TypeText"), "typing must use the same full INPUT union as keys");
  const count = calls.length;
  assert.equal((await b.type({ text: "" })).action_sent, false);
  assert.equal(calls.length, count);
});

test("win32: unsupported characters cannot become unrelated virtual keys", async () => {
  const { run, calls } = mockExec();
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run, persistentInputOwner: true } });
  for (const text of ["!", "_", "中", "ß", "ctrl+!", "bogus+a"]) {
    await assert.rejects(b.key({ text }), /unknown key combination/);
    await assert.rejects(b.hold_key({ text, duration: 0.05 }), /unknown key combination/);
  }
  assert.equal(calls.length, 0, "refused keys must not spawn input or cleanup");
  for (const text of ["ctrl+A", "9", "alt+tab", "shift"]) assert.equal((await b.key({ text })).action_sent, true);
});

test("win32: failed chord delivery releases only owned keys, with no replay", async () => {
  const mod = await import("../src/backends/win32.mjs");
  const calls = [];
  const b = mod.create({ exec: { run: async (_cmd, args) => {
    calls.push(decodeScript(args));
    return calls.length === 1
      ? { code: 1, stdout: "", stderr: "SendInput inserted 1 of 6 events" }
      : { code: 0, stdout: "", stderr: "" };
  } } });
  await assert.rejects(b.key({ text: "ctrl+shift+a" }), /inserted 1 of 6/);
  assert.equal(calls.length, 2);
  const cleanup = calls[1].split("'@ -ErrorAction Stop;")[1];
  assert.match(cleanup, /SendKey\(65, 2\).*SendKey\(16, 2\).*SendKey\(17, 2\)/s);
  assert.doesNotMatch(cleanup, /SendKey\(\d+, 0\)|SendString|\$seq/, "recovery must not repeat down events");
  await b.releaseInput();
  assert.equal(calls.length, 2, "successful cleanup clears ownership");
});

test("win32: failed cleanup retains ownership and both failures for a later release", async () => {
  const mod = await import("../src/backends/win32.mjs");
  const calls = [];
  const run = async (_cmd, args) => {
    calls.push(decodeScript(args));
    return calls.length <= 2
      ? { code: 1, stdout: "", stderr: calls.length === 1 ? "SendInput inserted 1 of 4 events" : "SendInput inserted 0 of 1 events" }
      : { code: 0, stdout: "", stderr: "" };
  };
  const b = mod.create({ exec: { run } });
  await assert.rejects(b.key({ text: "ctrl+a" }), (err) => {
    assert.equal(err.name, "ExecError");
    assert.match(err.message, /inserted 1 of 4.*release failed:.*inserted 0 of 1/);
    assert.match(err.cause.message, /inserted 1 of 4/);
    assert.match(err.cleanupError.message, /inserted 0 of 1/);
    return true;
  });
  await mod.create({ exec: { run } }).releaseInput();
  assert.equal(calls.length, 2, "another backend must not release this owner's keys");
  await b.releaseInput();
  assert.equal(calls.length, 3, "failed releases remain owned for retry");
  assert.match(calls[2], /SendKey\(65, 2\).*SendKey\(17, 2\)/s);
  await b.releaseInput();
  assert.equal(calls.length, 3);
});

test("win32: wheel packets preserve signed DWORD bits in both axes and clamp notches", async () => {
  const { run, calls } = mockExec();
  const mod = await import("../src/backends/win32.mjs");
  const b = mod.create({ exec: { run } });
  for (const [direction, amount, vertical, horizontal] of [
    ["down", 3, "4294966936", "0"], ["up", 3, "360", "0"],
    ["left", 3, "0", "4294966936"], ["right", 3, "0", "360"],
    ["down", 100, "4294963696", "0"], ["up", 0, "120", "0"],
  ]) {
    await b.scroll({ target: { x: 1, y: 2 }, direction, amount });
    const script = calls.at(-1).script;
    assert.ok(script.includes(`::WHEEL, 0, 0, ${vertical}, [UIntPtr]::Zero)`));
    assert.ok(script.includes(`::HWHEEL, 0, 0, ${horizontal}, [UIntPtr]::Zero)`));
    assert.doesNotMatch(script, /-band 0xFFFFFFFF/);
  }
});

test('win32: single-monitor discovery preserves an array and negative raster origins', async t => {
  const fs = await import('node:fs'); const os = await import('node:os'); const path = await import('node:path');
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'cu-win-capture-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const file = path.join(dir, 'shot.png');
  const scripts = [];
  const mod = await import('../src/backends/win32.mjs');
  const b = mod.create({ exec: { run: async (_cmd, args) => {
    const script = decodeScript(args); scripts.push(script);
    if (script.includes('CopyFromScreen')) { fs.writeFileSync(file, 'fixture'); return { code: 0, stdout: JSON.stringify({x:-1920,y:-100,w:1920,h:1080}), stderr:'' }; }
    return { code:0, stdout: JSON.stringify({ displays:[{name:'one',primary:true,x:-1920,y:-100,w:1920,h:1080}] }), stderr:'' };
  } } });
  assert.equal((await b.list_displays()).length, 1);
  assert.match(scripts[0], /displays = \$arr/);
  const shot = await b.screenshot({path:file});
  assert.deepEqual(shot.points, {x:-1920,y:-100,w:1920,h:1080});
  await assert.rejects(b.screenshot({region:[0,0,-1,5]}), /region/);
  for (const result of [{code:0,timedOut:true},{code:0,aborted:true},{code:1}]) {
    const broken = mod.create({ exec:{run:async()=>({stdout:'{"x":0,"y":0,"w":1,"h":1}',stderr:'capture failure',...result})} });
    await assert.rejects(broken.screenshot({path:file}));
    await assert.rejects(broken.open_application({name:'notepad'}));
  }
});

test('win32: semantic mutations carry window and leaf identities and traverse zero child indices', async () => {
  const mod = await import('../src/backends/win32.mjs');
  const scripts=[];
  const b=mod.create({exec:{run:async(_cmd,args)=>{scripts.push(decodeScript(args));return {code:0,stdout:'{"ok":true,"verified":true}',stderr:''};}}});
  const target={path:[0,0,0],runtime_id:[42,10,1],window_runtime_id:[42,10],app_ref:{name:'Fixture'},windowIndex:0,role:'Edit',label:''};
  assert.equal((await b.set_value({target,value:'漢😀'})).verified,true);
  await b.perform_action({target,action:'Invoke'});
  for(const script of scripts){
    assert.match(script,/\$step = 1/); assert.doesNotMatch(script,/if \(\$i -eq 0\)/);
    assert.match(script,/observed element was replaced/); assert.match(script,/observed window no longer exists/);
    assert.ok(script.indexOf('element_stale') < script.indexOf('$vp.SetValue') || !script.includes('$vp.SetValue'));
  }
  await assert.rejects(b.perform_action({target,action:"';Invoke-Expression evil"}), /unsupported UIA action/);
});

test('win32: CLIXML reports the actual PowerShell error rather than its serialization envelope', async () => {
  const mod=await import('../src/backends/win32.mjs');
  const b=mod.create({exec:{run:async()=>({code:1,stdout:'',stderr:'#< CLIXML\n<Objs><S S="Error">native failure &lt;target&gt;_x000D__x000A_</S></Objs>'})}});
  await assert.rejects(b.type({text:'x'}), error => /native failure <target>/.test(error.message) && !/CLIXML|<Objs>/.test(error.message));
});
