// app_script policy: what a script may do before it reaches osascript.
//
// app_script is the programmatic interface into apps with a scripting
// dictionary, not a shell. By default this module refuses the ways a script
// escapes into one (`do shell script`, JXA `doShellScript`, the Objective-C
// bridge and NSTask, script loading/eval, raw Apple event codes) and extracts
// every application the script names, so the per-app consent ledger gates
// `tell application "X"` — System Events and the processes it drives included —
// exactly as it gates clicks. A target the text cannot name statically (a
// computed application, a computed JXA member) is refused rather than guessed.
//
// This is a lexical gate, not a sandbox: it is defense in depth under the
// host's exact-script approval, which is the real floor. It fails closed —
// anything it cannot read confidently is refused with a reason the model can
// act on.
//
// Operators choose the mode with CODEWHALE_CU_APP_SCRIPT:
//   (unset) | "apps"  — the default described above
//   "off"             — refuse every app_script call
//   "unrestricted"    — skip the lexical refusals (the ledger still gates the
//                       apps a script names). A human decision in the host's
//                       MCP config; nothing a model can set from a tool call.

const MODES = new Set(["apps", "off", "unrestricted"]);

export function appScriptMode(env = process.env) {
  const raw = String(env.CODEWHALE_CU_APP_SCRIPT ?? "").trim().toLowerCase();
  if (!raw) return "apps";
  // An unknown value is a misconfiguration; fail closed rather than open.
  return MODES.has(raw) ? raw : "off";
}

const refuse = (reason) => ({ refused: reason, targets: [] });

/** Remove string literals so structure checks cannot be fooled by quoted text. */
function stripStrings(src, quotes) {
  let out = "";
  for (let i = 0; i < src.length; i++) {
    const q = src[i];
    if (!quotes.includes(q)) { out += q; continue; }
    out += q + q;
    for (i++; i < src.length && src[i] !== q; i++) if (src[i] === "\\") i++;
  }
  return out;
}

function refFor(value, { bundle = false } = {}) {
  const s = String(value).trim();
  if (!s) return null;
  if (bundle || (/^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$/.test(s) && !/\.app$/i.test(s))) return { bundle_id: s };
  return { name: s.replace(/\.app$/i, "") };
}

// ---- AppleScript ----
const AS_DENY = [
  [/\bdo\s+shell\s+script\b/i, "`do shell script` runs a shell"],
  [/\b(run|load|store)\s+script\b/i, "`run/load/store script` executes code the policy cannot read"],
  [/\buse\s+framework\b/i, "AppleScriptObjC (`use framework`) reaches Cocoa directly"],
  [/\bcurrent\s+application\s*'s\b/i, "AppleScriptObjC (`current application's`) reaches Cocoa directly"],
  [/\bNS(Task|UserUnixTask|UserScriptTask|AppleScript|Workspace)\b/i, "Cocoa process and script classes are not app scripting"],
  [/\bcall\s+method\b/i, "`call method` reaches Objective-C"],
  [/«/, "raw Apple event codes («event …») bypass the dictionary the policy reads"],
  [/\bosascript\b/i, "nested osascript is refused"],
  [/\bdo\s+script\b/i, "`do script` runs a shell command in a terminal"],
  // System Events keystrokes and coordinate clicks land on whatever app is
  // frontmost, whichever process the script names; use the type/key/click
  // tools, which carry the per-app gates.
  [/\b(keystroke|key\s+code)\b/i, "System Events keystrokes go to the frontmost app, not the named one — use the type or key tool"],
  [/\bclick\s+at\b/i, "coordinate clicks through System Events go to whatever is on screen — use the click tool"],
];

function checkAppleScript(script) {
  // Join ¬ continuations so a phrase split across lines is still one phrase.
  const src = script.replace(/¬[ \t]*\r?\n/g, " ");
  const targets = [];
  const refused = (reason) => ({ refused: reason, targets });
  // application "X", app "X", application id "com.x", plus System Events'
  // `process "X"` / `application process "X"` GUI-scripting targets.
  const literal = /\b(?:application|app)\s+(id\s+)?"((?:[^"\\]|\\.)*)"/gi;
  for (const m of src.matchAll(literal)) { const ref = refFor(m[2], { bundle: !!m[1] }); if (ref) targets.push(ref); }
  for (const m of src.matchAll(/\b(?:application\s+)?process\s+"((?:[^"\\]|\\.)*)"/gi)) { const ref = refFor(m[1]); if (ref) targets.push(ref); }
  for (const [re, why] of AS_DENY) if (re.test(src)) return refused(why);
  // Any other use of `application`/`app` must be a form the policy knows:
  // `current application`, `application "X"`, `application id "X"`,
  // `application process "X"`, or `application file`/`application support`
  // inside strings (already stripped). A computed target is refused.
  const bare = stripStrings(src, ['"']);
  for (const m of bare.matchAll(/\b(application|app)\b(\s*(?:id\s*)?)(.?)/gi)) {
    const before = bare.slice(Math.max(0, m.index - 20), m.index);
    if (/\bcurrent\s+$/i.test(before)) continue;
    if (m[3] === '"') continue;
    const rest = bare.slice(m.index + m[1].length);
    if (/^\s*process(es)?\b/i.test(rest)) continue;
    if (/^\s*support\b/i.test(rest)) continue; // path to application support
    return refused("the script names an application the policy cannot read statically — name it as a literal: tell application \"Name\"");
  }
  // A System Events process reached by index or predicate (process 1, first
  // process whose frontmost is true) is an app the ledger never saw. Only a
  // literal name, or listing names, is allowed.
  for (const m of bare.matchAll(/\bprocess(es)?\b/gi)) {
    const rest = bare.slice(m.index + m[0].length);
    const before = bare.slice(Math.max(0, m.index - 40), m.index);
    if (!m[1] && /^\s*""/.test(rest)) continue;
    if (/\bname\s+of\s+(every\s+)?(application\s+)?$/i.test(before)) continue;
    return refused("System Events processes must be named with a literal (process \"Name\") so the app can be consented");
  }
  return { refused: null, targets };
}

// ---- JXA ----
// Checked against the script with string literals removed: text inside a
// string cannot run unless something evaluates it or indexes by it, and both
// of those are refused below.
const JXA_DENY = [
  [/doShellScript/i, "`doShellScript` runs a shell"],
  [/\bdoScript\b/, "`doScript` runs a shell command in a terminal"],
  [/\.\s*(keystroke|keyCode)\s*\(/, "System Events keystrokes go to the frontmost app, not the named one — use the type or key tool"],
  [/\.\s*click\s*\(\s*\{/, "coordinate clicks through System Events go to whatever is on screen — use the click tool"],
  [/\bObjC\b/, "the Objective-C bridge (ObjC) reaches Cocoa directly"],
  [/\$\s*[.([]/, "the Objective-C bridge ($) reaches Cocoa directly"],
  [/\bNS(Task|UserUnixTask|UserScriptTask|AppleScript|Workspace)\b/, "Cocoa process and script classes are not app scripting"],
  [/includeStandardAdditions/, "StandardAdditions exposes doShellScript; use the app's own dictionary"],
  [/\b(eval|Function|Library|Ref|require|importScripts|constructor|prototype|__proto__|Reflect|Proxy)\b/, "dynamic code loading, evaluation and reflection are refused"],
  [/\bObject\s*\.\s*(getOwnProperty\w*|defineProperty|defineProperties|entries|values|assign|getPrototypeOf|setPrototypeOf)\b/, "reflection over objects is refused"],
  [/\bosascript\b/i, "nested osascript is refused"],
];

function checkJxa(script) {
  const code = stripStrings(script, ['"', "'", "`"]);
  const targets = [];
  const literal = /\bApplication\s*\(\s*(["'])((?:(?!\1)[^\\]|\\.)*)\1\s*\)/g;
  for (const m of script.matchAll(literal)) { const ref = refFor(m[2]); if (ref) targets.push(ref); }
  const byName = /\b(?:applicationProcesses|processes)\s*\.\s*byName\s*\(\s*(["'])((?:(?!\1)[^\\]|\\.)*)\1\s*\)/g;
  for (const m of script.matchAll(byName)) { const ref = refFor(m[2]); if (ref) targets.push(ref); }
  const refused = (reason) => ({ refused: reason, targets });
  if (/doShellScript/i.test(script)) return refused("`doShellScript` runs a shell");
  if (/\\u|\\x/.test(script)) return refused("escape sequences are refused so names cannot be spelled around the policy");
  for (const [re, why] of JXA_DENY) if (re.test(code)) return refused(why);
  // Computed member access could spell doShellScript at runtime; only numeric
  // indexes are allowed. Collections take .at(i) and .byName("x") instead.
  for (const m of code.matchAll(/[\w$)\]]\s*\[([^\]]*)\]/g)) {
    if (!/^\s*\d+\s*$/.test(m[1])) return refused("computed member access (x[expr]) is refused — use .at(i), .byName(\"Name\") or a literal property");
  }
  // Computed keys in object literals and destructuring patterns ({[k]: v}).
  if (/[{,]\s*\[/.test(code)) return refused("computed keys ({[expr]: …}) and nested array literals are refused");
  // System Events processes: a literal .byName("X"), or listing names.
  for (const m of code.matchAll(/\b(applicationProcesses|processes)\b/g)) {
    const rest = code.slice(m.index + m[0].length);
    if (/^\s*\.\s*byName\s*\(\s*(""|'')\s*\)/.test(rest)) continue;
    if (/^\s*\.\s*name\s*\(\s*\)/.test(rest)) continue;
    return refused("System Events processes must be named with .byName(\"Name\") so the app can be consented");
  }
  // Application must be called with one literal, or be .currentApplication().
  for (const m of code.matchAll(/\bApplication\b/g)) {
    const rest = code.slice(m.index + "Application".length);
    if (/^\s*\.\s*currentApplication\s*\(\s*\)/.test(rest)) continue;
    if (/^\s*\(\s*``/.test(rest)) return refused("Application(`…`) may interpolate — name the app with a plain string");
    if (/^\s*\(\s*(""|'')\s*\)/.test(rest)) continue;
    return refused("the script names an application the policy cannot read statically — use Application(\"Name\")");
  }
  return { refused: null, targets };
}

// Apps whose scripting dictionary is itself a shell or a script runner.
// Driving them through app_script is arbitrary command execution by another
// name, so they are refused as targets in the default mode.
const SHELL_HOSTS = new Set([
  "terminal", "iterm", "iterm2", "warp", "alacritty", "kitty", "ghostty", "wezterm", "hyper", "tabby",
  "script editor", "automator", "shortcuts", "shortcuts events", "osascript",
  "com.apple.terminal", "com.googlecode.iterm2", "dev.warp.warp-stable", "org.alacritty", "net.kovidgoyal.kitty",
  "com.mitchellh.ghostty", "com.github.wez.wezterm", "co.zeit.hyper", "com.apple.scripteditor2", "com.apple.automator",
  "com.apple.shortcuts", "com.apple.shortcuts.events",
]);
const shellHost = (ref) => SHELL_HOSTS.has(String(ref.bundle_id ?? ref.name ?? "").trim().toLowerCase());

/**
 * Check one app_script call. Returns {refused: string|null, targets: ref[]}
 * where each ref is {name} or {bundle_id} for the consent ledger.
 */
export function checkAppScript(script, language = "applescript", env = process.env) {
  const mode = appScriptMode(env);
  if (mode === "off") return refuse("app_script is turned off on this computer (CODEWHALE_CU_APP_SCRIPT=off)");
  const checked = language === "javascript" ? checkJxa(String(script)) : checkAppleScript(String(script));
  if (mode === "unrestricted") return { refused: null, targets: checked.targets };
  if (checked.refused) return checked;
  const host = checked.targets.find(shellHost);
  if (host) return { refused: `${host.name ?? host.bundle_id} runs shell commands or scripts — app_script does not drive it`, targets: checked.targets };
  return checked;
}
