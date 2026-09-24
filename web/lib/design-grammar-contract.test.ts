import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { resolveWhale } from "./whale-tokens";
import { siteCss } from "./site-css";

// Radii, state roles and motion follow the GPUI client (DESIGN.md, set_theme,
// motion.rs): one radius grammar, one focus ring, spring-fitted easing, and a
// still pose under reduced motion.
const CSS = siteCss();
const TAILWIND = readFileSync(new URL("../tailwind.config.ts", import.meta.url), "utf8");
const WHALE = readFileSync(new URL("../components/whale.tsx", import.meta.url), "utf8");

describe("design grammar contract", () => {
  it("draws every radius from the 6/10/14/pill grammar", () => {
    const defined = [...CSS.matchAll(/--radius-[\w-]+:\s*([^;]+);/g)].map((m) => resolveWhale(m[1].trim()));
    expect(new Set(defined)).toEqual(new Set(["6px", "10px", "14px", "999px"]));
    const used = [...CSS.matchAll(/border-radius:\s*([^;]+);/g)].map((m) => m[1].trim());
    expect(used.length).toBeGreaterThan(0);
    for (const value of used) expect(value).toMatch(/^var\(--radius-(control|surface|sheet|pill)\)$/);
    const scale = TAILWIND.match(/borderRadius:\s*\{([\s\S]*?)\}/)?.[1] ?? "";
    expect(scale).not.toMatch(/\d+(px|rem)/);
  });

  it("has one focus ring and the set_theme selection", () => {
    expect(CSS.match(/outline:/g)).toHaveLength(1);
    expect(CSS).toContain(":focus-visible { outline: var(--gpui-focus-width) solid var(--ring); outline-offset: var(--gpui-focus-offset); }");
    expect(resolveWhale("var(--gpui-focus-width)")).toBe("2px");
    expect(resolveWhale("var(--gpui-focus-offset)")).toBe("3px");
    expect(CSS).not.toMatch(/outline:\s*none/);
    expect(CSS).toMatch(/::selection \{ background: var\(--selection\); \}/);
    // The light ring on the dark stage would fall under 3:1.
    expect(CSS).toMatch(/\.site-footer,[\s\S]*?\{ --ring: var\(--gpui-dark-primary\); \}/);
  });

  it("hovers a primary fill at the primary @ 0.9", () => {
    for (const cls of ["portal-button-primary", "folio-button-primary", "paper-install-cta"]) {
      const hover = CSS.match(new RegExp(`\\.${cls}:hover \\{([^}]*)\\}`))?.[1] ?? "";
      expect(hover, cls).toMatch(/background: var\(--indigo-deep\)/);
    }
  });

  it("times motion with the spring tokens and stills it under reduced motion", () => {
    expect(CSS).not.toMatch(/\d+ms ease[,;]/);
    expect(CSS).not.toMatch(/cubic-bezier\(0\.25, 0\.46/);
    expect(CSS).toMatch(/--ease-spring: cubic-bezier\(/);
    expect(CSS).toMatch(
      /@media \(prefers-reduced-motion: reduce\) \{\s*:root \{\s*--dur-state: 0ms;\s*--dur-spring: 0ms;/,
    );
    expect(TAILWIND).toMatch(/transitionDuration: \{ DEFAULT: "var\(--dur-state\)" \}/);
    expect(TAILWIND).toMatch(/transitionTimingFunction: \{ DEFAULT: "var\(--ease-spring\)" \}/);
    expect(CSS).not.toMatch(/caustic/);
    expect(WHALE).not.toMatch(/caustic|<animate|clipPath/);
  });

  it("keeps no unused presence shapes", () => {
    expect(existsSync(new URL("../components/presence.tsx", import.meta.url))).toBe(false);
  });
});
