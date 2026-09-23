import { readFileSync } from "node:fs";

/**
 * The design palette as the site actually resolves it.
 *
 * `app/tokens.css` is generated from `crates/palette/src/tokens.rs` by
 * `scripts/export-design-tokens.py`, and `globals.css` carries the hand-kept
 * `--gpui-*` block that mirrors the GPUI client's theme. Site variables state
 * which token each uses (`--paper: var(--gpui-paper)`) instead of repeating
 * the hex. The contract tests still need the literal color to check parity
 * and contrast, so this reads both files and flattens the alias chains
 * (`--whale-success` -> `--whale-working-green` -> `#9bd66f`,
 * `--paper` -> `--gpui-paper` -> `#f5f0e9`). The Blue Stage light preset's
 * `LIGHT_*` consts export as `--light-*` beside them, and the Shoreline
 * redesign's dark/light pair exports as `--shoreline-*` /
 * `--shoreline-light-*`.
 *
 * Node-only (`node:fs`): imported by the contract tests, never by a component.
 */
const RAW: Record<string, string> = (() => {
  const generated = readFileSync(new URL("../app/tokens.css", import.meta.url), "utf8");
  const globals = readFileSync(new URL("../app/globals.css", import.meta.url), "utf8");
  const raw: Record<string, string> = {};
  for (const match of generated.matchAll(/--((?:whale|light|shoreline-light|shoreline)-[\w-]+):\s*([^;]+);/g)) {
    raw[match[1]] = match[2].trim();
  }
  for (const match of globals.matchAll(/--(gpui-[\w-]+):\s*([^;]+);/g)) {
    raw[match[1]] = match[2].trim();
  }
  if (Object.keys(raw).length === 0) {
    throw new Error("no palette properties found in tokens.css/globals.css");
  }
  return raw;
})();

function flatten(name: string, seen = new Set<string>()): string {
  const value = RAW[name];
  if (value === undefined) throw new Error(`Unknown design token: --${name}`);
  const alias = value.match(/^var\(--([\w-]+)\)$/);
  if (!alias) return value;
  if (seen.has(name)) throw new Error(`Cyclic design token alias: --${name}`);
  return flatten(alias[1], seen.add(name));
}

/** Resolve a `var(--whale-*)`, `var(--light-*)`, `var(--shoreline-*)`, or `var(--gpui-*)` reference to its literal value; pass anything else through. */
export function resolveWhale(value: string): string {
  const match = value.match(/^var\(--((?:whale|light|shoreline-light|shoreline|gpui)-[\w-]+)\)$/);
  return match ? flatten(match[1]) : value;
}
