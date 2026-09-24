/**
 * Interaction contract for #5290 — clickable chrome on non-English routes.
 *
 * Reproduction (Chromium, codewhale.net, 1536×900): `.paper-wordmark` was
 * 0×42px on `/de` and `/pt-BR`, 35×42px on `/id`, and 98×42px on
 * `/de/docs/guide` at 1280. The 2xl companion labels plus an unbounded
 * locale <select> ate the 76rem strip; `overflow-x: clip` then left the
 * home control with no hit target. English still had a 216px wordmark.
 *
 * This file pins the layout and route-handler decisions that keep a
 * usable hit target, for every routed locale, without a browser.
 */
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { locales } from "./config";
import { getChrome } from "./dictionaries";
import { navLinks } from "./links";
import { isDocsPath, replacePathLocale } from "./path";
import { siteCss } from "../site-css";

const webRoot = new URL("../../", import.meta.url);

function webText(path: string): string {
  return readFileSync(new URL(path, webRoot), "utf8");
}

/** Rough Latin/CJK advance at the nav's 0.78rem body size (16px root). */
function advance(text: string, fontRem: number): number {
  let px = 0;
  for (const ch of text) {
    const code = ch.codePointAt(0) ?? 0;
    const wide = code > 0x2e80 || (code >= 0x1100 && code <= 0x11ff);
    px += (wide ? 1.05 : 0.58) * fontRem * 16;
  }
  return px;
}

describe("localized chrome keeps a clickable home control", () => {
  const css = siteCss();
  const theme = webText("components/theme-toggle.tsx");

  it("keeps the wordmark and locale switcher from shrinking to zero", () => {
    // The floor is `min-width`, not a zero shrink factor: flexbox will not
    // take an item below its `min-width`, so the box can still give space
    // back when the row is over budget.
    expect(css).toMatch(/\.paper-wordmark\s*\{[\s\S]*?flex:\s*0 1 auto;/);
    expect(css).toMatch(/\.paper-wordmark\s*\{[\s\S]*?min-width:\s*8\.75rem;/);
    expect(css).toMatch(
      /\.site-nav-actions select\s*\{\s*width:\s*6\.75rem;\s*max-width:\s*6\.75rem;/,
    );
    expect(theme).toContain("hidden 2xl:inline");
    expect(theme).not.toContain("hidden sm:inline");
  });

  it("fits every locale's desktop strip inside the 76rem container", () => {
    // --container: min(100% - 2rem, 76rem). The desktop strip is the 76rem
    // cap (1216px). Wordmark 8.75rem, select 6.75rem, remaining actions
    // measured on the 1536 deployed pass, minus the 124px the unbounded
    // select used to steal and the theme word that no longer shows below 2xl.
    const container = 76 * 16;
    const wordmarkMin = 8.75 * 16;
    const gaps = 1.5 * 16 * 2;
    const select = 6.75 * 16;
    const actionsBesidesSelect = 542 - 232;
    const actionBudget = select + actionsBesidesSelect;

    for (const locale of locales) {
      const links = navLinks(locale, getChrome(locale));
      const navWidth =
        links.reduce((sum, link) => sum + Math.max(advance(link.label, 0.78), 23), 0) +
        20 * Math.max(links.length - 1, 0);
      const used = wordmarkMin + navWidth + actionBudget + gaps;
      expect(used, `${locale} strip ${Math.round(used)}px`).toBeLessThanOrEqual(container);
      for (const link of links) {
        expect(link.href.startsWith(`/${locale}/`), `${locale} ${link.href}`).toBe(true);
      }
    }
  });

  it("fits the compact nav strip inside a 375px viewport", () => {
    // --container is min(100% - 2rem, 76rem) → 343px at 375. Below 520px the
    // strip is wordmark + [theme, select, menu]: the desktop nav is
    // display:none, the wordmark tag and install CTA are hidden, and the
    // GitHub/Discord links went at 900px. Widths are the CSS boxes — border
    // plus padding plus a 1em glyph for the theme control, which is
    // glyph-only below 2xl.
    const container = 375 - 2 * 16;
    const innerGap = 0.5 * 16;
    const actionGap = 0.35 * 16;
    const themeToggle = 16 + 2 * 0.375 * 16 + 2;
    const select = 6.75 * 16;
    const menuToggle = 2.25 * 16;
    const actions = themeToggle + actionGap + select + actionGap + menuToggle;

    // At the width its own content asks for, the row does not fit. Every
    // page carries the theme control, so every page is the tight case.
    expect(9.75 * 16 + innerGap + actions).toBeGreaterThan(container);

    // It fits because the wordmark gives space back down to its floor. With
    // `flex-shrink: 0` the overflow landed on the menu toggle instead, and
    // `overflow-x: clip` on <body> then ate the edge of it.
    expect(8.75 * 16 + innerGap + actions).toBeLessThanOrEqual(container);
    expect(css).toMatch(/\.paper-wordmark\s*\{[\s\S]*?flex:\s*0 1 auto;/);
    expect(css).toMatch(
      /@media \(max-width: 520px\)[\s\S]*?\.paper-wordmark\s*\{\s*max-width:\s*9\.75rem;/,
    );
  });

  it("keeps locale switching on the shared path helpers and the theme control on every page", () => {
    expect(replacePathLocale("/pt-BR/docs/guide", "ja")).toBe("/ja/docs/guide");
    expect(replacePathLocale("/de", "zh")).toBe("/zh");
    expect(isDocsPath("/pt-BR/docs/guide")).toBe(true);
    expect(isDocsPath("/id/install")).toBe(false);
    expect(webText("components/locale-switcher.tsx")).toContain("replacePathLocale");
    expect(theme).not.toContain("isDocsPath");
  });
});
