// src/dsh/shell-hooks.ts
import { createHash } from "node:crypto";
import { readFileSync, realpathSync, lstatSync } from "node:fs";
import { resolve, relative, isAbsolute, sep } from "node:path";

// src/dsh/upstream/hooks/hook-protocol/src/matcher.ts
function isMatchAll(matcher) {
  return matcher === void 0 || matcher === "" || matcher === "*";
}
var CLAUDE_LITERAL = /^[A-Za-z0-9_|]+$/;
function compileRegex(pattern) {
  try {
    return new RegExp(pattern);
  } catch (_syntaxError) {
    return void 0;
  }
}
function matcherDiagnostic(matcher, mode) {
  if (isMatchAll(matcher)) return void 0;
  const pattern = matcher;
  if (mode === "claude-code" && CLAUDE_LITERAL.test(pattern)) return void 0;
  return compileRegex(pattern) === void 0 ? `invalid ${mode} regex matcher ${JSON.stringify(pattern)}` : void 0;
}

// src/dsh/upstream/hooks/hooks-claude-code/src/config.ts
var CLAUDE_EVENTS = [
  "SessionStart",
  "UserPromptSubmit",
  "PreToolUse",
  "PostToolUse",
  "Stop",
  "SubagentStart",
  "SubagentStop"
];
function asObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? value : void 0;
}
function substituteCommand(command, vars) {
  let out = command;
  if (vars.pluginRoot !== void 0) out = out.split("${CLAUDE_PLUGIN_ROOT}").join(vars.pluginRoot);
  if (vars.projectDir !== void 0) out = out.split("${CLAUDE_PROJECT_DIR}").join(vars.projectDir);
  return out;
}
function parseClaudeCodeConfig(raw, vars = {}) {
  const config = {};
  const skipped = [];
  const root = asObject(raw);
  const hooksMap = root ? asObject(root.hooks) ?? root : void 0;
  if (!hooksMap) return { config, skipped };
  for (const event of CLAUDE_EVENTS) {
    const rawGroups = hooksMap[event];
    if (!Array.isArray(rawGroups)) continue;
    const groups = [];
    for (const rawGroup of rawGroups) {
      const group = asObject(rawGroup);
      if (!group || !Array.isArray(group.hooks)) continue;
      const commands = [];
      for (const rawHook of group.hooks) {
        const hook = asObject(rawHook);
        if (!hook) continue;
        const type = typeof hook.type === "string" ? hook.type : "command";
        if (type !== "command") {
          skipped.push({ event, type });
          continue;
        }
        if (typeof hook.command !== "string") continue;
        commands.push({
          command: substituteCommand(hook.command, vars),
          ...typeof hook.timeout === "number" ? { timeoutSec: hook.timeout } : {}
        });
      }
      if (commands.length === 0) continue;
      const matcher = event === "UserPromptSubmit" || event === "Stop" ? void 0 : typeof group.matcher === "string" ? group.matcher : void 0;
      const diagnostic = matcherDiagnostic(matcher, "claude-code");
      if (diagnostic !== void 0) throw new SyntaxError(`${diagnostic} on event ${JSON.stringify(event)}`);
      groups.push({
        ...matcher !== void 0 ? { matcher } : {},
        hooks: commands
      });
    }
    if (groups.length > 0) config[event] = groups;
  }
  return { config, skipped };
}

// src/dsh/upstream/hooks/hooks-codex/src/config.ts
var CODEX_EVENTS = ["PreToolUse", "PostToolUse", "SessionStart", "UserPromptSubmit", "Stop"];
function asObject2(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? value : void 0;
}
function parseCodexConfig(raw) {
  const config = {};
  const skipped = [];
  const root = asObject2(raw);
  const hooksMap = root ? asObject2(root.hooks) ?? root : void 0;
  if (!hooksMap) return { config, skipped };
  for (const event of CODEX_EVENTS) {
    const rawGroups = hooksMap[event];
    if (!Array.isArray(rawGroups)) continue;
    const groups = [];
    for (const rawGroup of rawGroups) {
      const group = asObject2(rawGroup);
      if (!group || !Array.isArray(group.hooks)) continue;
      const commands = [];
      for (const rawHook of group.hooks) {
        const hook = asObject2(rawHook);
        if (!hook) continue;
        const type = typeof hook.type === "string" ? hook.type : "command";
        if (type !== "command") {
          skipped.push({ event, reason: `unsupported "${type}" hook` });
          continue;
        }
        if (hook.async === true) {
          skipped.push({ event, reason: "async hook" });
          continue;
        }
        if (typeof hook.command !== "string") continue;
        const timeout = typeof hook.timeout === "number" ? hook.timeout : typeof hook.timeoutSec === "number" ? hook.timeoutSec : void 0;
        commands.push({ command: hook.command, ...timeout !== void 0 ? { timeoutSec: timeout } : {} });
      }
      if (commands.length === 0) continue;
      const matcher = event === "UserPromptSubmit" || event === "Stop" ? void 0 : typeof group.matcher === "string" ? group.matcher : void 0;
      const diagnostic = matcherDiagnostic(matcher, "codex");
      if (diagnostic !== void 0) throw new SyntaxError(`${diagnostic} on event ${JSON.stringify(event)}`);
      groups.push({ ...matcher !== void 0 ? { matcher } : {}, hooks: commands });
    }
    if (groups.length > 0) config[event] = groups;
  }
  return { config, skipped };
}

// src/dsh/shell-hooks.ts
var EVENTS = { SessionStart: "session_start", UserPromptSubmit: "message_submit", PreToolUse: "tool_call_before", PostToolUse: "tool_call_after", Stop: "turn_end", SubagentStart: "subagent_spawn", SubagentStop: "subagent_complete" };
function reviewedHookModule(dialect, root, files) {
  return { name: `hooks-${dialect}`, inject: ["shellHooks"], apply(ctx, config) {
    if (!config || typeof config.configPath !== "string") throw new Error("hook bridge needs its reviewed configPath");
    const path = resolve(root, config.configPath);
    const inside = relative(root, path).split(sep).join("/");
    if (!inside || inside.startsWith("../") || isAbsolute(inside) || !files[inside] || realpathSync(path) !== path || !lstatSync(path).isFile()) throw new Error("hook config is absent from the reviewed regular-file closure");
    const bytes = readFileSync(path);
    if (bytes.length > 1024 * 1024 || createHash("sha256").update(bytes).digest("hex") !== files[inside]) throw new Error("hook config changed after review or exceeds 1 MiB");
    if (config.projectDir !== void 0) throw new Error("explicit projectDir is unsupported; each process uses its current core workspace");
    if (config.pluginRoot !== void 0 && config.pluginRoot !== "." && config.pluginRoot !== root) throw new Error("pluginRoot must be this reviewed bundle root");
    const raw = JSON.parse(bytes.toString("utf8"));
    const parsed = dialect === "claude-code" ? parseClaudeCodeConfig(raw, { pluginRoot: root }) : parseCodexConfig(raw);
    if (parsed.skipped.length) throw new Error("hook config contains unsupported non-command or asynchronous hooks");
    let count = 0;
    for (const [point, groups] of Object.entries(parsed.config)) {
      if (point === "Stop") ctx.logger.warn("Stop runs at the core TurnEnd observer; forced continuation is unsupported");
      for (const group of groups) for (const hook of group.hooks) {
        if (++count > 128) throw new Error("hook bridge exceeds 128 commands");
        const seconds = hook.timeoutSec ?? (config.defaultTimeoutMs ?? 6e5) / 1e3;
        if (!Number.isSafeInteger(seconds) || seconds < 1 || seconds > 86400) throw new Error("hook timeout must be 1–86400 whole seconds");
        ctx.shellHooks.register({ dialect, point, ...group.matcher === void 0 ? {} : { matcher: group.matcher }, hook: { event: EVENTS[point], command: hook.command, timeout_secs: seconds, background: false, continue_on_error: false, name: `${dialect}:${point}:${count}` } });
      }
    }
    if (count === 0) throw new Error("reviewed hook config has no supported command hooks");
  } };
}
export {
  reviewedHookModule
};
