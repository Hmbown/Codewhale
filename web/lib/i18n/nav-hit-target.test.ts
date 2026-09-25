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
  const locale = webText("components/locale-switcher.tsx");

  // Quiet icon controls are 2.5rem squares 2px apart (GPUI rail metrics).
  const icon = 2.5 * 16;
  const iconGap = 2;

  it("keeps the wordmark from shrinking to zero and the controls icon-only", () => {
    // The floor is `min-width`, not a zero shrink factor: flexbox will not
    // take an item below its `min-width`, so the box can still give space
    // back when the row is over budget.
    expect(css).toMatch(/\.paper-wordmark\s*\{[\s\S]*?flex:\s*0 1 auto;/);
    expect(css).toMatch(/\.paper-wordmark\s*\{[\s\S]*?min-width:\s*8\.75rem;/);
    expect(css).toMatch(/\.nav-icon-button\s*\{[\s\S]*?width:\s*2\.5rem;\s*height:\s*2\.5rem;/);
    expect(css).toMatch(/\.site-nav-actions\s*\{[\s\S]*?gap:\s*2px;/);
    // The theme and locale controls carry their state in the accessible
    // name, not in visible text, so no translated word can widen the strip.
    // The native <select> (an unbounded one once ate 124px, #5290) sits
    // transparent over the globe glyph and is clipped to the icon box.
    expect(theme).not.toMatch(/\{labels\[shown\]\}\s*</);
    expect(locale).toContain('className="nav-icon-button nav-locale"');
    expect(css).toMatch(/\.nav-locale select\s*\{[\s\S]*?inset:\s*0;[\s\S]*?opacity:\s*0;/);
  });

  it("fits every locale's desktop strip at the lg floor", () => {
    // At lg (1024px) --container is min(100% - 32px, 76rem) → 992px. The
    // strip is wordmark + links + [theme, locale, GitHub, Sign in, Install].
    const container = 1024 - 32;
    const wordmarkMin = 8.75 * 16;
    const gaps = 1.5 * 16 * 2;
    for (const code of locales) {
      const chrome = getChrome(code);
      const links = navLinks(code, chrome);
      const navWidth =
        links.reduce((sum, link) => sum + Math.max(advance(link.label, 0.78), 23), 0) +
        20 * Math.max(links.length - 1, 0);
      const signIn = 0.5 * 16 + 1.5 * 16 + advance(chrome.authSignIn, 0.8125);
      const install = 0.25 * 16 + 1.7 * 16 + advance(chrome.installCta, 0.8125);
      const actions = 3 * icon + 2 * iconGap + signIn + install;
      const used = wordmarkMin + navWidth + actions + gaps;
      expect(used, `${code} strip ${Math.round(used)}px`).toBeLessThanOrEqual(container);
      for (const link of links) {
        expect(link.href.startsWith(`/${code}/`), `${code} ${link.href}`).toBe(true);
      }
    }
  });

  it("fits the compact nav strip inside a 375px viewport", () => {
    // --container is min(100% - 2rem, 76rem) → 343px at 375. Below lg the
    // strip is wordmark + [theme, locale, menu]: the desktop nav, Sign in and
    // Install are display:none, and the GitHub icon goes at 900px.
    const container = 375 - 2 * 16;
    const innerGap = 0.5 * 16;
    const actions = 3 * icon + 2 * iconGap;
    // Even at the wordmark's phone cap the row fits without shrinking it.
    expect(9.75 * 16 + innerGap + actions).toBeLessThanOrEqual(container);
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
