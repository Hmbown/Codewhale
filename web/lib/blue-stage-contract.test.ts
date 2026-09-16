import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { resolveWhale } from "./whale-tokens";

const CSS = readFileSync(new URL("../app/globals.css", import.meta.url), "utf8");

function selectorBlock(selector: string): string {
  const match = CSS.match(
    new RegExp(`(?:^|\n)${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`, "s"),
  );
  if (!match) throw new Error(`Missing CSS selector: ${selector}`);
  return match[1];
}

// globals.css names the palette token (`--paper: var(--gpui-paper)`)
// rather than repeating its hex; resolve one hop through the generated
// app/tokens.css plus the hand-kept --gpui-* block.
function cssHexIn(block: string, name: string): string {
  const match = block.match(new RegExp(`--${name}:\\s*([^;]+);`, "i"));
  if (!match) throw new Error(`Missing CSS token in block: --${name}`);
  const value = resolveWhale(match[1].trim());
  if (!/^#[0-9a-f]{6}$/i.test(value)) {
    throw new Error(`--${name} is not a hex color: ${value}`);
  }
  return value.toLowerCase();
}

const ROOT = selectorBlock(":root");
const BELOW_WATERLINE = selectorBlock(
  '.ocean-column,\n.site-footer,\nhtml[data-theme="dark"] .docs-portal',
);

describe("GPUI public-surface contract", () => {
  it("grounds the paper sheet in the GPUI light theme's warm paper and inks", () => {
    // Above the waterline the field is the GPUI light background — warm
    // paper — and the ink is its plum-charcoal foreground. The literals are
    // the theme constants in codehwhale-gpui/src/workspace/mod.rs plus the
    // derived tones in codehwhale-gpui/mockups/style.css.
    expect(cssHexIn(ROOT, "paper")).toBe("#f5f0e9");
    expect(cssHexIn(ROOT, "paper-deep")).toBe("#eae3de");
    expect(cssHexIn(ROOT, "paper-edge")).toBe("#d7ced5");
    expect(cssHexIn(ROOT, "paper-card")).toBe("#fbf5ee");
    expect(cssHexIn(ROOT, "ink")).toBe("#302832");
    expect(cssHexIn(ROOT, "ink-soft")).toBe("#564e58");
    expect(cssHexIn(ROOT, "ink-mute")).toBe("#6b606e");
    // Action on paper is the GPUI light primary; hover sinks to its hover.
    expect(cssHexIn(ROOT, "indigo")).toBe("#245bc7");
    expect(cssHexIn(ROOT, "indigo-deep")).toBe("#174aa9");
    expect(cssHexIn(ROOT, "mark-ink")).toBe("#302832");
    // The deep field is always the stage's darkest, and code plates keep the
    // stage deep on either side of the waterline.
    expect(cssHexIn(ROOT, "ocean-deep")).toBe("#171618");
    expect(cssHexIn(ROOT, "action-on-dark")).toBe("#90b9ff");
    expect(cssHexIn(ROOT, "ocean-current")).toBe("#90b9ff");
    expect(cssHexIn(ROOT, "code-bg")).toBe("#171618");
  });

  it("re-inks every dark subtree with the GPUI charcoal tokens through one rule", () => {
    // The ocean column, the footer seabed, and the opt-in docs dark sheet
    // share one below-the-waterline rule, so a component never needs to know
    // which side of the surface it is on.
    expect(CSS).toMatch(/\.ocean-column,\s*\.site-footer,\s*html\[data-theme="dark"\] \.docs-portal\s*\{/);
    expect(cssHexIn(BELOW_WATERLINE, "paper")).toBe("#211f23");
    expect(cssHexIn(BELOW_WATERLINE, "paper-deep")).toBe("#2b282e");
    expect(cssHexIn(BELOW_WATERLINE, "paper-edge")).toBe("#49424d");
    expect(cssHexIn(BELOW_WATERLINE, "ink")).toBe("#f2ece5");
    expect(cssHexIn(BELOW_WATERLINE, "ink-soft")).toBe("#b0a7b2");
    expect(cssHexIn(BELOW_WATERLINE, "ink-mute")).toBe("#8d858f");
    expect(cssHexIn(BELOW_WATERLINE, "indigo")).toBe("#90b9ff");
    expect(cssHexIn(BELOW_WATERLINE, "jade")).toBe("#9ec7b2");
    expect(cssHexIn(BELOW_WATERLINE, "signal-gold")).toBe("#d6c78f");
  });

  it("draws the water from palette tokens only, never a hex of its own", () => {
    const strata = readFileSync(new URL("../components/strata.tsx", import.meta.url), "utf8");
    expect(strata).not.toMatch(/#[0-9a-f]{3,8}\b/i);
    for (const token of ["--gpui-primary-dark", "--gpui-stage-raised", "--gpui-stage-muted", "--gpui-stage-deep", "--gpui-tan", "--ink"]) {
      expect(strata, token).toContain(`var(${token})`);
    }
    // Static and decorative: no animation, hidden from assistive technology.
    expect(strata).not.toMatch(/animate|@keyframes/);
    expect(strata).toContain('aria-hidden="true"');
  });

  it("renders the whale mark in the sheet's mark ink while controls use action blue", () => {
    expect(CSS).toMatch(/\.codewhale-mark-primary \{ fill: var\(--mark-ink\); \}/);
    expect(CSS).toMatch(/\.portal-button-primary[\s\S]*background: var\(--indigo\)/);
    expect(CSS).toMatch(/\.nav-link::after[\s\S]*background: var\(--indigo\)/);
  });

  it("keeps localized navigation controls inside compact viewports", () => {
    const mobile = CSS.split("@media (max-width: 520px)")[1];

    expect(mobile).toMatch(/\.site-nav-inner\s*\{\s*gap:\s*0\.5rem/);
    expect(mobile).toMatch(/\.site-nav-actions\s*\{[\s\S]*?min-width:\s*0/);
    expect(mobile).toMatch(
      /\.site-nav-actions select\s*\{[\s\S]*?width:\s*6\.75rem;[\s\S]*?min-width:\s*0/,
    );
    expect(CSS).toMatch(/\.paper-wordmark-mark\s*\{[^}]*height:\s*22px;/);
    expect(CSS).toMatch(/\.paper-wordmark-logo\s*\{[^}]*height:\s*20px;/);
    expect(CSS).toMatch(/\.site-nav-actions\s*\{[\s\S]*?flex-shrink:\s*0/);
    expect(CSS).toMatch(/\.site-nav-actions\s*>\s*\*\s*\{\s*flex-shrink:\s*0/);
    expect(CSS).toMatch(/@media \(max-width: 900px\)[\s\S]*?\.site-github-link\s*\{\s*display:\s*none/);
    expect(mobile).not.toMatch(/body:has\(\.product-home\) \.site-nav-actions select/);
    // The locale <select> and the home wordmark must keep a usable hit
    // target on every viewport, not only below 520px. Long native option
    // labels and 2xl companion text used to collapse the wordmark to 0.
    expect(CSS).toMatch(
      /\.site-nav-actions select\s*\{\s*width:\s*6\.75rem;\s*max-width:\s*6\.75rem;\s*min-width:\s*0;/,
    );
    // `min-width` is the floor that keeps the wordmark clickable; the shrink
    // factor stays at 1 so the compact controls are never the ones pushed
    // past `overflow-x: clip` when the row is over budget.
    expect(CSS).toMatch(/\.paper-wordmark\s*\{[^}]*flex:\s*0 1 auto;[^}]*min-width:\s*8\.75rem;/);
    expect(CSS).not.toMatch(/\.paper-wordmark\s*\{[^}]*flex:\s*0 0 auto;/);
  });
});
