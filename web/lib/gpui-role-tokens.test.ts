import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { resolveWhale } from "./whale-tokens";
import { siteCss } from "./site-css";

// The role layer (app/styles/tokens-roles.css): every role in every scheme,
// inks from the versioned GPUI artifact, the navy ocean grounds measured
// against every ink at WCAG AA.
const CSS = siteCss();
const DESIGN = JSON.parse(readFileSync(new URL("../../vendor/codewhale-design/tokens.json", import.meta.url), "utf8"));
const ROLES = [
  "bg", "surface", "panel", "text", "muted", "line", "line-strong", "accent", "on-accent",
  "hover", "selected", "selection", "ring", "live", "attention", "danger",
];
const GROUNDS = ["bg", "surface", "panel", "hover", "selected"];
const TEXT_INKS = ["text", "muted", "accent", "live", "attention", "danger"];

function declarations(block: string): Record<string, string> {
  const vars: Record<string, string> = {};
  for (const match of block.matchAll(/--([\w-]+):\s*([^;]+);/g)) vars[match[1]] = match[2].trim();
  return vars;
}

/** The first block for `selector` (at any indent) that declares `--bg`. */
function roleBlock(selector: string): Record<string, string> {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  for (const match of CSS.matchAll(new RegExp(`(?:^|\\n)\\s*${escaped}\\s*\\{([^}]*)\\}`, "g"))) {
    const vars = declarations(match[1]);
    if ("bg" in vars) return vars;
  }
  throw new Error(`No role block for ${selector}`);
}

function hex(value: string): string {
  const resolved = resolveWhale(value);
  if (!/^#[0-9a-f]{6}$/i.test(resolved)) throw new Error(`not a hex color: ${value} -> ${resolved}`);
  return resolved.toLowerCase();
}

function luminance(color: string): number {
  const [r, g, b] = color
    .slice(1)
    .match(/.{2}/g)!
    .map((v) => Number.parseInt(v, 16) / 255)
    .map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const light = () => roleBlock(":root");
const osDark = () => roleBlock(':root:not([data-theme="light"])');
const pinnedDark = () => roleBlock(':root[data-theme="dark"]');
const stage = () => roleBlock(".stage,\n.site-footer");

describe("role tokens", () => {
  it("defines every role in light, OS-dark, pinned-dark and the stage", () => {
    for (const [name, scheme] of [["light", light()], ["os-dark", osDark()], ["pinned", pinnedDark()], ["stage", stage()]] as const) {
      for (const role of ROLES) expect(scheme, `${name}.${role}`).toHaveProperty(role);
    }
    // The OS-dark scheme is guarded so a pinned light page stays light, and
    // the pinned dark block repeats it exactly.
    expect(CSS).toMatch(/@media \(prefers-color-scheme: dark\)\s*\{\s*:root:not\(\[data-theme="light"\]\)\s*\{/);
    expect(pinnedDark()).toEqual(osDark());
  });

  it("inks with the versioned GPUI artifact in both appearances", () => {
    const inks: Record<string, string> = {
      text: "foreground", muted: "muted_foreground", accent: "primary", "on-accent": "primary_foreground",
      ring: "primary", live: "live", attention: "attention", danger: "danger",
    };
    for (const [mode, scheme] of [["light", light()], ["dark", osDark()], ["dark", stage()]] as const) {
      for (const [role, key] of Object.entries(inks)) {
        expect(scheme[role], `${mode}.${role}`).toMatch(/^var\(--gpui-(light|dark)-[\w-]+\)$/);
        expect(hex(scheme[role]), `${mode}.${role}`).toBe(`#${DESIGN.colors[mode][key]}`);
      }
      expect(scheme.selection).toBe(`rgb(var(--gpui-${mode}-primary-rgb) / var(--gpui-selection-opacity))`);
    }
    // Light keeps the artifact's paper grounds; dark reaches for the ocean.
    expect(hex(light().bg)).toBe(`#${DESIGN.colors.light.background}`);
    expect(hex(light().panel)).toBe(`#${DESIGN.colors.light.surface}`);
    for (const role of GROUNDS) expect(osDark()[role], role).toMatch(/^var\(--ocean-[\w-]+\)$/);
  });

  it("keeps every text ink at WCAG AA on every ground, in every scheme", () => {
    for (const [name, scheme] of [["light", light()], ["dark", osDark()], ["stage", stage()]] as const) {
      for (const ground of GROUNDS) {
        for (const ink of TEXT_INKS) {
          expect(contrast(hex(scheme[ink]), hex(scheme[ground])), `${name}: ${ink} on ${ground}`).toBeGreaterThanOrEqual(4.5);
        }
      }
      // Control edges and state marks clear the 3:1 non-text floor.
      for (const ground of ["bg", "panel"]) {
        expect(contrast(hex(scheme["line-strong"]), hex(scheme[ground])), `${name}: line-strong on ${ground}`).toBeGreaterThanOrEqual(3);
      }
      expect(contrast(hex(scheme["on-accent"]), hex(scheme.accent)), `${name}: on-accent`).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("puts white text on the logo gradient only where it clears 4.5:1", () => {
    const fill = CSS.match(/--brand-fill:\s*linear-gradient\(([^;]+)\);/)?.[1] ?? "";
    const stops = [...fill.matchAll(/#[0-9a-f]{6}|var\(--brand-[\w-]+\)/gi)].map((m) => hex(m[0]));
    expect(stops.length).toBeGreaterThanOrEqual(2);
    for (const stop of stops) expect(contrast("#ffffff", stop), stop).toBeGreaterThanOrEqual(4.5);
    expect(hex("var(--brand-deep)")).toBe("#0b48bb");
    expect(hex("var(--brand-light)")).toBe("#1e8fd8");
  });

  it("keeps footer text readable on the sea below its waterline", () => {
    const sea = CSS.match(/--sea:\s*linear-gradient\(([^;]+)\);/)?.[1] ?? "";
    const stops = [...sea.matchAll(/#[0-9a-f]{6}/gi)].map((m) => m[0].toLowerCase());
    // The first stop is the bright waterline; content starts below it.
    for (const stop of stops.slice(1)) {
      expect(contrast(hex(stage().muted), stop), `muted on ${stop}`).toBeGreaterThanOrEqual(4.5);
      expect(contrast(hex(stage().text), stop), `text on ${stop}`).toBeGreaterThanOrEqual(4.5);
    }
  });
});
