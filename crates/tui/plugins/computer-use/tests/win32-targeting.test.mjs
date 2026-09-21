// Actual backend dispatch with an injected runner. Windows-only cases execute
// source-derived PowerShell after replacing every desktop access with fixtures.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { test } from "node:test";
import win32 from "../src/backends/win32.mjs";

const WINDOWS = { skip: process.platform !== "win32" && "Requires Windows PowerShell; synthetic window metadata only", timeout: 60_000 };
const FORBIDDEN = /DllImport|LibraryImport|\bextern\b|user32\.dll|GetDelegateForFunctionPointer|NativeLibrary|SendKeys|UIAutomationClient|UIAutomationTypes|System\.Windows\.(?:Automation|Forms)|CopyFromScreen/iu;
const FRAME = "CU_TARGETING_FIXTURE:";

function decode(command, args) {
  assert.equal(command, "powershell.exe");
  assert.deepEqual(args.slice(0, 3), ["-NoProfile", "-NonInteractive", "-EncodedCommand"]);
  assert.equal(args.length, 4);
  return Buffer.from(args[3], "base64").toString("utf16le");
}

function capture(result = { found: true, name: "Fixture", elements: [], windows: [] }) {
  const scripts = [];
  const backend = win32.create({ exec: { run: async (command, args) => {
    scripts.push(decode(command, args));
    return { code: 0, stdout: JSON.stringify(result), stderr: "" };
  } } });
  return { backend, scripts };
}

function replaceOne(script, from, to) {
  assert.equal(script.split(from).length, 2, `Expected exactly one fixture boundary: ${from}`);
  return script.replace(from, to);
}

function windowFixture(original, rows) {
  let script = replaceOne(original, "Add-Type -AssemblyName System.Windows.Forms;", "");
  const blocks = [...script.matchAll(/Add-Type -TypeDefinition @'\n([\s\S]*?)\n'@;/gu)];
  assert.equal(blocks.length, 1);
  const imports = [...blocks[0][1].matchAll(/\[DllImport\("user32\.dll"\)\] static extern [^;]+? (\w+)\([^;]+;/gu)].map((match) => match[1]);
  assert.deepEqual(imports.sort(), ["EnumWindows", "GetWindowRect", "GetWindowText", "GetWindowTextLength", "GetWindowThreadProcessId", "IsWindowVisible"].sort());
  // Replace the complete native declaration block, preserving the production
  // PowerShell parsing/serialization tail and backend JavaScript mapping.
  script = script.replace(blocks[0][0], `Add-Type -TypeDefinition @'
using System.Collections.Generic;
public static class WinEnum {
  public static List<string> List() { return new List<string> { ${rows.map((row) => JSON.stringify(row)).join(", ")} }; }
}
'@;`);
  assert.doesNotMatch(script, FORBIDDEN);
  return script;
}

function stateFixture(original, names) {
  const walk = "foreach ($t in $targets) {";
  assert.equal(original.split(walk).length, 2);
  // Keep the production name decoding, equality filter and ambiguity check.
  // Stop before the content walk: the fixture supplies root-window names only.
  let script = original.slice(0, original.indexOf(walk));
  script = replaceOne(script, "Add-Type -AssemblyName UIAutomationClient;", "");
  script = replaceOne(script, "Add-Type -AssemblyName UIAutomationTypes;", "");
  const provider = /Add-Type -ReferencedAssemblies[^\n]+[\s\S]*?\[CUAutomationProviders\]::Register\(\);/u;
  assert.match(script, provider);
  script = script.replace(provider, "");
  script = replaceOne(script, "$root = [System.Windows.Automation.AutomationElement]::RootElement;", "");
  const data = Buffer.from(JSON.stringify(names), "utf16le").toString("base64");
  script = replaceOne(script, "$targets = @($root.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition));",
    `$fixtureNames = [Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('${data}')) | ConvertFrom-Json;
$targets = @($fixtureNames | ForEach-Object { [pscustomobject]@{ Current = [pscustomobject]@{ Name = [string]$_ } } });`);
  script += `
$selected = @($targets | ForEach-Object { $_.Current.Name });
$name = $null; if ($selected.Count -gt 0) { $name = $selected[0] }
Write-Output (@{ found = $selected.Count -gt 0; name = $name; elements = @() } | ConvertTo-Json -Compress);`;
  assert.doesNotMatch(script, FORBIDDEN);
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
Write-Output ('${FRAME}' + (@{ output = @($fixtureOutput.ToArray()); error = $fixtureError } | ConvertTo-Json -Depth 6 -Compress));
if ($null -ne $fixtureError) { exit 86; }
`;
  assert.doesNotMatch(wrapped, FORBIDDEN, "No native import or desktop access may survive fixture preparation");
  const encoded = Buffer.from(wrapped, "utf16le").toString("base64");
  assert.ok(encoded.length < 30_000, "The fixture must fit the Windows command-line limit");
  return encoded;
}

function nativeFixture(prepare) {
  return win32.create({ exec: { run: async (command, args) => {
    assert.equal(process.platform, "win32", "Only Windows can execute the prepared fixture");
    const encoded = encodedFixture(prepare(decode(command, args)));
    const executable = path.join(process.env.SystemRoot || "C:\\Windows", "System32", "WindowsPowerShell", "v1.0", "powershell.exe");
    const result = spawnSync(executable, ["-NoProfile", "-NonInteractive", "-EncodedCommand", encoded], {
      encoding: "utf8", windowsHide: true, timeout: 25_000, maxBuffer: 4 * 1024 * 1024,
    });
    assert.ifError(result.error);
    assert.equal(result.signal, null);
    const frames = result.stdout.split(/\r?\n/u).filter((line) => line.startsWith(FRAME));
    assert.equal(frames.length, 1, `Expected one fixture receipt: ${result.stderr}`);
    const trace = JSON.parse(frames[0].slice(FRAME.length));
    return { code: result.status, stdout: trace.output.join("\n"), stderr: trace.error || result.stderr };
  } } });
}

test("Windows window listing and screenshots reject every explicit selector before a runner call", async () => {
  const { backend, scripts } = capture();
  for (const tool of ["list_windows", "screenshot"]) {
    for (const key of ["app_ref", "window_id"]) {
      for (const value of [undefined, null, false, 0, -1, 1, 1.5, "Fixture", [], {}, { name: "Fixture" }, { pid: 123 }]) {
        await assert.rejects(backend[tool]({ [key]: value }), { code: "unsupported_selector" });
      }
    }
  }
  assert.equal(scripts.length, 0, "Unsupported selection must not execute even a discovery or capture command");
});

test("Windows state rejects malformed and unsupported references before a runner call", async () => {
  const { backend, scripts } = capture();
  for (const app_ref of [undefined, null, false, 0, "Fixture", [], {}, { name: "" }, { name: " \n" }, { name: 123 }, { name: null }, { pid: 123 }, { bundle_id: "test.fixture" }, { name: "Fixture", pid: 123 }, { name: "Fixture", extra: true }, { app_ref: { name: "Fixture" } }]) {
    await assert.rejects(backend.get_app_state({ app_ref }), { code: "unsupported_selector" });
  }
  for (const window_id of [undefined, null, 0, 1, "0", {}]) {
    await assert.rejects(backend.get_app_state({ app_ref: { name: "Fixture" }, window_id }), { code: "unsupported_selector" });
  }
  assert.equal(scripts.length, 0);
});

test("Windows semantic actions refuse ignored target identities before any UIA mutation", async () => {
  const { backend, scripts } = capture();
  for (const tool of ["set_value", "perform_action"]) {
    for (const key of ["app_ref", "windowIndex", "window_id"]) {
      for (const value of [undefined, null, 0, 1, -1, {}, { name: "Fixture" }, { pid: 123 }]) {
        await assert.rejects(backend[tool]({ target: { path: [0], [key]: value }, value: "fixture", action: "Invoke" }), { code: "unsupported_selector" });
      }
    }
  }
  assert.equal(scripts.length, 0, "An unsupported element identity must not read or mutate another UIA window");
});

test("Windows state preserves omitted selection and carries exact window names as data", async () => {
  const { backend, scripts } = capture();
  await backend.get_app_state({});
  const name = "O'Brien [*]? | 中; $env:SECRET";
  await backend.get_app_state({ app_ref: { name } });
  const script = scripts.at(-1);
  const data = script.match(/FromBase64String\('([^']*)'\)/u)[1];
  assert.equal(Buffer.from(data, "base64").toString("utf16le"), name);
  assert.ok(!script.includes(name));
  assert.match(script, /\[string\]::Equals\(\$_\.Current\.Name, \$filter, \[StringComparison\]::OrdinalIgnoreCase\)/u);
  assert.doesNotMatch(script, /-notlike|-like/u);
  assert.ok(script.indexOf("$targets.Count -gt 1") < script.indexOf("foreach ($t in $targets)"));
});

test("Windows window-list script and JavaScript mapping retain complete window metadata", async () => {
  const { backend, scripts } = capture({ windows: { pid2: 123, geom: "-10,-20,300,400", title: "O'Brien | [*]?" } });
  assert.deepEqual(await backend.list_windows({}), { windows: [{ pid: 123, title: "O'Brien | [*]?", position: { x: -10, y: -20 }, size: { w: 300, h: 400 } }] });
  assert.match(scripts[0], /\$json = \[WinEnum\]::List\(\) \|/u);
});

test("Windows observation fixture removes native access and fits its encoded envelope", async () => {
  const { backend, scripts } = capture();
  await backend.list_windows({});
  await backend.get_app_state({ app_ref: { name: "Fixture" } });
  for (const script of [windowFixture(scripts[0], ["123|-10,-20,300,400|Fixture"]), stateFixture(scripts[1], ["Other", "Fixture"])]) {
    assert.doesNotMatch(script, FORBIDDEN);
    assert.ok(encodedFixture(script).length < 30_000);
  }
  assert.throws(() => windowFixture(scripts[0].replace("GetWindowRect(IntPtr", "UnknownNative(IntPtr"), []));
  assert.throws(() => encodedFixture(windowFixture(scripts[0], []) + "\n[System.Windows.Automation.AutomationElement]::RootElement"));
});

test("Windows PowerShell parses real window-list output for zero, one and multiple windows", WINDOWS, async () => {
  for (const rows of [[], ["123|-10,-20,300,400|O'Brien [*]? | 中"], ["123|0,0,300,400|First", "999|-500,10,200,100|Second"]]) {
    const backend = nativeFixture((script) => windowFixture(script, rows));
    const result = await backend.list_windows({});
    assert.equal(result.windows.length, rows.length);
    assert.deepEqual(result.windows, rows.map((row) => {
      const first = row.indexOf("|"); const second = row.indexOf("|", first + 1);
      const [x, y, w, h] = row.slice(first + 1, second).split(",").map(Number);
      return { pid: Number(row.slice(0, first)), title: row.slice(second + 1), position: { x, y }, size: { w, h } };
    }));
  }
});

test("Windows PowerShell matches literal complete window names and preserves omission", WINDOWS, async () => {
  const name = "O'Brien [*]? | 中; $env:SECRET";
  const backend = nativeFixture((script) => stateFixture(script, ["Other", name, "Longer " + name]));
  assert.equal((await backend.get_app_state({ app_ref: { name } })).name, name);
  assert.equal((await backend.get_app_state({})).name, "Other");
  const casing = nativeFixture((script) => stateFixture(script, ["Other", "FIXTURE"]));
  assert.equal((await casing.get_app_state({ app_ref: { name: "fixture" } })).name, "FIXTURE");
});

test("Windows PowerShell refuses substring-only and ambiguous exact names", WINDOWS, async () => {
  const missing = nativeFixture((script) => stateFixture(script, ["Other", "Fixture extended"]));
  await assert.rejects(missing.get_app_state({ app_ref: { name: "Fixture" } }), /application window not found/u);
  const ambiguous = nativeFixture((script) => stateFixture(script, ["Fixture", "FIXTURE"]));
  await assert.rejects(ambiguous.get_app_state({ app_ref: { name: "Fixture" } }), /More than one application window/u);
});
