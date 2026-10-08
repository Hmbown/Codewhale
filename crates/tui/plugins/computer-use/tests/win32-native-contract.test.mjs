// Windows executes the real generated PowerShell and compiles its real INPUT
// structs. Every native import is replaced before execution with managed stubs:
// no test in this file can send keyboard/mouse input to the user's desktop.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { test } from "node:test";
import win32 from "../src/backends/win32.mjs";

const WINDOWS = { skip: process.platform !== "win32" && "Requires Windows PowerShell; managed stubs only, never live OS input", timeout: 60_000 };
const FRAME = "CU_MANAGED_FIXTURE:";
const NATIVE_IMPORT = /\[DllImport\("user32\.dll"(?:,\s*SetLastError\s*=\s*true)?\)\]\s+(?:public|private)\s+static\s+extern\s+[^;]+;/gu;
const FORBIDDEN = /DllImport|LibraryImport|\bextern\b|user32\.dll|GetDelegateForFunctionPointer|NativeLibrary|SendKeys/iu;

function decodedCommand(command, args) {
  assert.equal(command, "powershell.exe");
  assert.deepEqual(args.slice(0, 3), ["-NoProfile", "-NonInteractive", "-EncodedCommand"]);
  assert.equal(args.length, 4);
  return Buffer.from(args[3], "base64").toString("utf16le");
}

function managedScript(original, { returns = [], mutate = (value) => value } = {}) {
  assert.ok(returns.every((value) => Number.isInteger(value) && value >= 0 && value <= 0xffffffff));
  const stubs = {
    SetCursorPos: "public static bool SetCursorPos(int X, int Y) { Calls.Add(new Call { kind = \"SetCursorPos\", x = X, y = Y }); return true; }",
    mouse_event: "public static void mouse_event(uint dwFlags, int dx, int dy, uint dwData, UIntPtr dwExtraInfo) { Calls.Add(new Call { kind = \"mouse_event\", flags = dwFlags, x = dx, y = dy, data = dwData, extra = dwExtraInfo.ToUInt64() }); }",
    GetCursorPos: "public static bool GetCursorPos(out POINT lpPoint) { lpPoint = new POINT(); Calls.Add(new Call { kind = \"GetCursorPos\" }); return true; }",
    SetForegroundWindow: "public static bool SetForegroundWindow(IntPtr hWnd) { Calls.Add(new Call { kind = \"SetForegroundWindow\" }); return true; }",
    SendInput: `private static uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize) {
      uint inserted = nextReturn < ReturnPlan.Length ? ReturnPlan[nextReturn++] : nInputs;
      var events = new KeyEvent[pInputs.Length];
      for (int i = 0; i < pInputs.Length; i++) {
        events[i] = new KeyEvent { type = pInputs[i].type, vk = pInputs[i].u.ki.wVk, scan = pInputs[i].u.ki.wScan, flags = pInputs[i].u.ki.dwFlags };
      }
      Calls.Add(new Call { kind = "SendInput", count = nInputs, size = cbSize, arrayType = pInputs.GetType().FullName, events = events, inserted = inserted });
      return inserted;
    }`,
  };
  const replaced = new Set();
  let script = original.replace(NATIVE_IMPORT, (declaration) => {
    const name = declaration.match(/\b(\w+)\s*\(/gu)?.at(-1)?.replace(/\s*\($/u, "");
    assert.ok(Object.hasOwn(stubs, name), `Refusing unknown native declaration: ${name}`);
    assert.equal(replaced.has(name), false, `Duplicate native declaration: ${name}`);
    replaced.add(name);
    return stubs[name].replace(/^(?:public|private)/u, declaration.match(/\]\s+(public|private)\b/u)[1]);
  });
  assert.deepEqual([...replaced].sort(), Object.keys(stubs).sort(), "Every expected native import must be replaced");
  assert.match(script, /public static class User32 \{/u);
  script = script.replace("public static class User32 {", `public static class User32 {
    public sealed class KeyEvent { public uint type; public ushort vk; public ushort scan; public uint flags; }
    public sealed class Call {
      public string kind; public uint count; public int size; public string arrayType; public KeyEvent[] events;
      public uint inserted; public uint flags; public uint data; public int x; public int y; public ulong extra;
    }
    public static readonly System.Collections.Generic.List<Call> Calls = new System.Collections.Generic.List<Call>();
    private static readonly uint[] ReturnPlan = new uint[] { ${returns.map((value) => `${value}u`).join(", ")} };
    private static int nextReturn;
  `);
  script = mutate(script);
  assert.doesNotMatch(script, FORBIDDEN, "Refusing to execute a fixture containing a native import or alternate input sink");
  return script;
}

function encodedFixture(script) {
  const wrapped = `[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false);
$fixtureOutput = New-Object 'System.Collections.Generic.List[string]'; $fixtureError = $null;
try {
  & {
${script}
  } | ForEach-Object { [void]$fixtureOutput.Add([string]$_); };
} catch { $fixtureError = $_.Exception.ToString(); }
$fixtureCalls = @(); $fixtureLayout = $null;
if ('User32' -as [type]) {
  $fixtureCalls = @([User32]::Calls);
  $fixtureLayout = @{
    pointerSize = [IntPtr]::Size;
    inputSize = [Runtime.InteropServices.Marshal]::SizeOf([type]'User32+INPUT');
    mouseSize = [Runtime.InteropServices.Marshal]::SizeOf([type]'User32+MOUSEINPUT');
    keySize = [Runtime.InteropServices.Marshal]::SizeOf([type]'User32+KEYBDINPUT');
    unionOffset = [Runtime.InteropServices.Marshal]::OffsetOf([type]'User32+INPUT', 'u').ToInt32();
    keyExtraOffset = [Runtime.InteropServices.Marshal]::OffsetOf([type]'User32+KEYBDINPUT', 'dwExtraInfo').ToInt32();
    keyScanOffset = [Runtime.InteropServices.Marshal]::OffsetOf([type]'User32+KEYBDINPUT', 'wScan').ToInt32();
    keyFlagsOffset = [Runtime.InteropServices.Marshal]::OffsetOf([type]'User32+KEYBDINPUT', 'dwFlags').ToInt32();
  };
}
Write-Output ('${FRAME}' + (@{ output = @($fixtureOutput.ToArray()); error = $fixtureError; calls = $fixtureCalls; layout = $fixtureLayout } | ConvertTo-Json -Depth 10 -Compress));
if ($null -ne $fixtureError) { exit 86; }
`;
  assert.doesNotMatch(wrapped, FORBIDDEN, "Final executable fixture must contain no native import");
  const encoded = Buffer.from(wrapped, "utf16le").toString("base64");
  assert.ok(encoded.length < 30_000, "Keep the encoded fixture under Windows' command-line limit");
  return encoded;
}

function fixture({ returns, mutate } = {}) {
  const invocations = [];
  const backend = win32.create({ exec: { persistentInputOwner: true, run: async (command, args) => {
    // This assertion is inside the only spawn path, not just in test options.
    assert.equal(process.platform, "win32", "Real PowerShell is Windows-only");
    const encoded = encodedFixture(managedScript(decodedCommand(command, args), { returns, mutate }));
    const executable = path.join(process.env.SystemRoot || "C:\\Windows", "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
    const result = spawnSync(executable, ["-NoProfile", "-NonInteractive", "-EncodedCommand", encoded], {
      encoding: "utf8", windowsHide: true, timeout: 25_000, maxBuffer: 4 * 1024 * 1024,
    });
    assert.ifError(result.error);
    assert.equal(result.signal, null);
    const frames = result.stdout.split(/\r?\n/u).filter((line) => line.startsWith(FRAME));
    assert.equal(frames.length, 1, `Fixture must produce one trace, even on failure: ${result.stderr}`);
    const trace = JSON.parse(frames[0].slice(FRAME.length));
    invocations.push({ ...trace, code: result.status });
    // Forward actual script output and actual process failure. Do not return a
    // fabricated success JSON: that would conceal PowerShell conversion errors.
    return { code: result.status, stdout: trace.output.join("\n"), stderr: trace.error || result.stderr };
  } } });
  return { backend, invocations };
}

async function sampleScript() {
  let script;
  await win32.create({ exec: { run: async (command, args) => {
    script = decodedCommand(command, args);
    return { code: 0, stdout: "", stderr: "" };
  } } }).type({ text: "A" });
  return script;
}

function assertLayout(layout) {
  assert.ok([4, 8].includes(layout.pointerSize));
  // Windows ABI expectations, independent of the structs being compiled.
  const is64 = layout.pointerSize === 8;
  assert.equal(layout.inputSize, is64 ? 40 : 28);
  assert.equal(layout.mouseSize, is64 ? 32 : 24);
  assert.equal(layout.keySize, is64 ? 24 : 16);
  assert.equal(layout.unionOffset, is64 ? 8 : 4);
  assert.equal(layout.keyExtraOffset, is64 ? 16 : 12);
  assert.equal(layout.keyScanOffset, 2);
  assert.equal(layout.keyFlagsOffset, 4);
}

function onlyTrace(invocations) {
  assert.equal(invocations.length, 1, "An input action must not replay its PowerShell script");
  return invocations[0];
}

function noSuccess(trace) {
  assert.notEqual(trace.code, 0);
  assert.ok(trace.error);
  assert.equal(trace.output.some((line) => /"ok"\s*:\s*true/u.test(line)), false, "No success output after a compiler, conversion, or delivery error");
}

test("win32 native fixture: all generated imports become managed methods before execution", async () => {
  const script = managedScript(await sampleScript());
  assert.doesNotMatch(script, FORBIDDEN);
  assert.match(script, /public static void SendString\(string text\)/u);
  assert.ok(encodedFixture(script).length < 30_000, "The actual executable envelope fits Windows' command-line limit");
});

test("win32 native fixture: unknown and reintroduced imports fail closed", async () => {
  const script = await sampleScript();
  for (const unsafe of [
    '[DllImport("kernel32.dll")] public static extern void Unsafe();',
    '[DllImport("user32.dll")] public static extern void Unsafe();',
    "public static extern void Unsafe();",
    '[LibraryImport("user32.dll")] public static partial void Unsafe();',
  ]) {
    assert.throws(() => managedScript(`${script}\n${unsafe}`), /Refusing/);
    assert.throws(() => managedScript(script, { mutate: (safe) => `${safe}\n${unsafe}` }), /Refusing/);
  }
});

test("win32 native contract: typed chord arrays compile and obey INPUT ABI", WINDOWS, async () => {
  const { backend, invocations } = fixture();
  assert.equal((await backend.key({ text: "ctrl+shift+a", repeat: 2 })).action_sent, true);
  const trace = onlyTrace(invocations);
  assertLayout(trace.layout);
  assert.equal(trace.calls.length, 1);
  const call = trace.calls[0];
  assert.equal(call.kind, "SendInput");
  assert.equal(call.arrayType, "User32+INPUT[]");
  assert.equal(call.count, 8);
  assert.equal(call.size, trace.layout.pointerSize === 8 ? 40 : 28);
  assert.deepEqual(call.events, [
    [17, 0], [16, 0], [65, 0], [65, 2], [65, 0], [65, 2], [16, 2], [17, 2],
  ].map(([vk, flags]) => ({ type: 1, vk, scan: 0, flags })));
});

test("win32 native contract: Unicode A, CJK and a surrogate pair use complete down/up pairs", WINDOWS, async () => {
  const { backend, invocations } = fixture();
  const result = await backend.type({ text: "A漢😀" });
  assert.equal(result.action_sent, true);
  assert.equal(result.chars, 4);
  const trace = onlyTrace(invocations);
  assertLayout(trace.layout);
  assert.equal(trace.calls.length, 4);
  for (const [index, scan] of [0x0041, 0x6f22, 0xd83d, 0xde00].entries()) {
    const call = trace.calls[index];
    assert.equal(call.kind, "SendInput");
    assert.equal(call.count, 2);
    assert.equal(call.inserted, 2);
    assert.equal(call.size, trace.layout.pointerSize === 8 ? 40 : 28);
    assert.deepEqual(call.events, [{ type: 1, vk: 0, scan, flags: 4 }, { type: 1, vk: 0, scan, flags: 6 }]);
  }
});

test("win32 native contract: held chords bind SendKey and release in reverse order", WINDOWS, async () => {
  const { backend, invocations } = fixture();
  assert.equal((await backend.hold_key({ text: "ctrl+a", duration: 0.05 })).action_sent, true);
  const trace = onlyTrace(invocations);
  assertLayout(trace.layout);
  assert.deepEqual(trace.calls.map((call) => [call.kind, call.count, call.inserted]), Array(4).fill(["SendInput", 1, 1]));
  assert.deepEqual(trace.calls.flatMap((call) => call.events), [
    [17, 0], [65, 0], [65, 2], [17, 2],
  ].map(([vk, flags]) => ({ type: 1, vk, scan: 0, flags })));
  assert.ok(trace.calls.every((call) => call.arrayType === "User32+INPUT[]" && call.size === trace.layout.inputSize));
});

test("win32 native contract: zero delivery rejects before later text or success", WINDOWS, async () => {
  const { backend, invocations } = fixture({ returns: [0] });
  await assert.rejects(backend.type({ text: "AB" }), /SendInput inserted 0 of 2/u);
  const trace = onlyTrace(invocations);
  noSuccess(trace);
  assert.deepEqual(trace.calls.map((call) => call.inserted), [0]);
  assert.deepEqual(trace.calls[0].events.map((event) => event.scan), [65, 65]);
});

for (const cleanupResult of [1, 0]) {
  test(`win32 native contract: partial Unicode delivery 1 -> ${cleanupResult} releases only its matching up`, WINDOWS, async () => {
    const { backend, invocations } = fixture({ returns: [1, cleanupResult] });
    await assert.rejects(backend.type({ text: "AB" }), /SendInput inserted 1 of 2/u);
    const trace = onlyTrace(invocations);
    noSuccess(trace);
    assert.deepEqual(trace.calls.map((call) => call.count), [2, 1]);
    assert.deepEqual(trace.calls.map((call) => call.inserted), [1, cleanupResult]);
    assert.deepEqual(trace.calls[0].events, [{ type: 1, vk: 0, scan: 65, flags: 4 }, { type: 1, vk: 0, scan: 65, flags: 6 }]);
    assert.deepEqual(trace.calls[1].events, [{ type: 1, vk: 0, scan: 65, flags: 6 }]);
    if (cleanupResult === 0) {
      assert.match(trace.error, /key-up recovery failed/u);
      assert.match(trace.error, /SendInput inserted 0 of 1/u);
    } else assert.doesNotMatch(trace.error, /key-up recovery failed/u);
  });
}

test("win32 native contract: C# compiler errors stop before script success", WINDOWS, async () => {
  const { backend, invocations } = fixture({ mutate: (script) => script.replace("public static class User32 {", "public static class User32 { invalid C# declaration;") });
  await assert.rejects(backend.type({ text: "A" }));
  const trace = onlyTrace(invocations);
  noSuccess(trace);
  assert.equal(trace.layout, null);
  assert.deepEqual(trace.calls, []);
});

test("win32 native contract: PowerShell argument conversion errors stop before script success", WINDOWS, async () => {
  const { backend, invocations } = fixture({ mutate: (script) => {
    assert.match(script, /\[User32\]::SendString\(\$text\);/u);
    return script.replace("[User32]::SendString($text);", "[User32]::SendKey('not-a-number', 0);");
  } });
  await assert.rejects(backend.type({ text: "A" }));
  const trace = onlyTrace(invocations);
  noSuccess(trace);
  assertLayout(trace.layout);
  assert.deepEqual(trace.calls, []);
});

for (const [direction, vertical, horizontal] of [
  ["down", 0xfffffe98, 0], ["up", 360, 0], ["left", 0, 0xfffffe98], ["right", 0, 360],
]) {
  test(`win32 native contract: ${direction} wheel binds the correct signed DWORD bits`, WINDOWS, async () => {
    const { backend, invocations } = fixture();
    assert.equal((await backend.scroll({ target: { x: 12, y: 34 }, direction, amount: 3 })).action_sent, true);
    const trace = onlyTrace(invocations);
    assert.equal(trace.calls.length, 3);
    assert.deepEqual([trace.calls[0].kind, trace.calls[0].x, trace.calls[0].y], ["SetCursorPos", 12, 34]);
    assert.deepEqual(trace.calls.slice(1).map(({ kind, flags, data, extra }) => ({ kind, flags, data, extra })), [
      { kind: "mouse_event", flags: 0x0800, data: vertical, extra: 0 },
      { kind: "mouse_event", flags: 0x1000, data: horizontal, extra: 0 },
    ]);
  });
}
