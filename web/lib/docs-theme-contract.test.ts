import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { resolveWhale } from "./whale-tokens";
import { siteCss } from "./site-css";

const CSS = siteCss();

function selectorBlock(selector: string): string {
  const match = CSS.match(new RegExp(`(?:^|\n)${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`, "s"));
  if (!match) throw new Error(`Missing CSS selector: ${selector}`);
  return match[1];
}

function selectorVars(selector: string): Record<string, string> {
  const block = selectorBlock(selector);
  const vars: Record<string, string> = {};
  // Values may be a literal hex or a `var(--whale-*)` reference into the
  // generated app/tokens.css; non-color values (channel triples, lengths) are
  // skipped, exactly as the hex-only regex used to skip them.
  for (const match of block.matchAll(/--([\w-]+):\s*([^;]+);/g)) {
    const value = resolveWhale(match[2].trim());
    if (/^#([0-9a-f]{3}|[0-9a-f]{6})$/i.test(value)) vars[match[1]] = value;
  }
  return vars;
}

function relativeLuminance(hex: string): number {
  const full = hex.length === 4 ? hex.slice(1).split("").map((c) => c + c).join("") : hex.slice(1);
  const channels = full
    .match(/.{2}/g)!
    .map((value) => Number.parseInt(value, 16) / 255)
    .map((value) => (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4));

  return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
}

function contrastRatio(foreground: string, background: string): number {
  const lighter = Math.max(relativeLuminance(foreground), relativeLuminance(background));
  const darker = Math.min(relativeLuminance(foreground), relativeLuminance(background));
  return (lighter + 0.05) / (darker + 0.05);
}

const PINNED_DARK = '.ocean-column,\n.site-footer,\n:root[data-theme="dark"]';

/** Every custom property declared in the OS-dark block (inside the media query). */
function osDarkVars(): Record<string, string> {
  const media = CSS.match(/@media \(prefers-color-scheme: dark\)\s*\{\s*:root:not\(\[data-theme="light"\]\)\s*\{([^}]*)\}/);
  if (!media) throw new Error("Missing OS-dark block");
  return Object.fromEntries([...media[1].matchAll(/--([\w-]+):\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]));
}

function allVars(selector: string): Record<string, string> {
  return Object.fromEntries(
    [...selectorBlock(selector).matchAll(/--([\w-]+):\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]),
  );
}

describe("site-wide theme contract", () => {
  it("follows the OS by default and repeats the pinned dark scheme exactly", () => {
    // No region-scoped dark sheet: the docs portal follows the root theme.
    expect(CSS).not.toMatch(/html\[data-theme="dark"\] \.docs-(portal|theme)/);
    expect(selectorBlock(":root")).toMatch(/color-scheme:\s*light/);
    expect(osDarkVars()).toEqual(allVars(PINNED_DARK));
    expect(selectorBlock(PINNED_DARK)).toMatch(/color-scheme:\s*dark/);
  });

  it("re-inks the navy nav wordmark under both dark selectors", () => {
    // wordmark.svg is fixed #142352 ink (~1.2:1 on the dark charcoal).
    expect(CSS).toMatch(/:root:not\(\[data-theme="light"\]\) \.paper-wordmark-logo\s*\{\s*filter:/);
    expect(CSS).toMatch(/:root\[data-theme="dark"\] \.paper-wordmark-logo\s*\{\s*filter:/);
  });

  it("shows the toggle on every page with one system|light|dark storage contract", () => {
    const toggle = readFileSync(new URL("../components/theme-toggle.tsx", import.meta.url), "utf8");
    expect(toggle).not.toMatch(/isDocsPath|return null/);
    expect(toggle).toMatch(/"system" \| "light" \| "dark"/);
    expect(toggle).toContain('const KEY = "cw-theme"');
    const layout = readFileSync(new URL("../app/[locale]/layout.tsx", import.meta.url), "utf8");
    // The boot script pins only an explicit light/dark; anything else (system,
    // a legacy "auto", nothing) is left to prefers-color-scheme.
    expect(layout).toContain("localStorage.getItem('cw-theme');if(t==='light'||t==='dark')");
  });
});

describe("docs theme contrast contract", () => {
  // The docs sheet follows the site theme. Light is the bare :root; dark is
  // :root overlaid with the pinned dark block (which the OS-dark block
  // repeats). Both are checked.
  const themes = () => [
    selectorVars(":root"),
    { ...selectorVars(":root"), ...selectorVars(PINNED_DARK) },
  ];

  it("keeps current and hover sidebar text at WCAG AA contrast", () => {
    for (const theme of themes()) {
      const accent = theme["docs-accent"];
      const background = theme["paper"];
      expect(contrastRatio(accent, background)).toBeGreaterThanOrEqual(4.5);
    }
    expect(CSS).toMatch(/\.docs-sidebar-link:hover,\s*\.docs-sidebar-link-current\s*{[^}]*color:\s*var\(--docs-accent\)/s);
  });

  it("keeps secondary button text at WCAG AA contrast", () => {
    for (const theme of themes()) {
      const text = theme["docs-button-text"];
      const background = theme["docs-button-bg"];
      expect(contrastRatio(text, background)).toBeGreaterThanOrEqual(4.5);
    }
    expect(selectorBlock(".docs-theme .portal-button-secondary")).toContain(
      "color: var(--docs-button-text)",
    );
  });
});
