import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { getRatatuiCopy } from "@/lib/content/ratatui";
import heroEntry from "@/public/ratatui/entries/showcase-work.json";
import { RatatuiHero } from "./hero";

describe("Ratatui hero", () => {
  it("pairs both responsive captions with the unchanged truecolor buffers in each locale", () => {
    expect([heroEntry.width, heroEntry.height]).toEqual([104, 30]);
    const previews = heroEntry.previews["dark-truecolor"];
    expect(previews.native).toContain('viewBox="0 0 1040 600"');
    expect(previews["40"]).toContain('viewBox="0 0 400 600"');

    for (const locale of ["en", "zh"]) {
      const copy = getRatatuiCopy(locale);
      const markup = renderToStaticMarkup(createElement(RatatuiHero, { locale, copy }));
      const caption = markup.match(/<figcaption\b[^>]*>([\s\S]*?)<\/figcaption>/)?.[1];
      const label = (width: string) => copy.heroCaption.replace("{width}", width).replace("{height}", "30");
      expect(caption).toContain(`<span class="rat-hero-wide">${label("104")}</span>`);
      expect(caption).toContain(`<span class="rat-hero-narrow">${label("40")}</span>`);
      expect(caption).not.toMatch(/\{width\}|\{height\}/);
      expect(markup).toContain(`aria-label="${copy.heroLabel}"`);
      expect(markup).toContain(`href="/${locale}/ratatui/showcase-work"`);

      const buffers = [...markup.matchAll(/<div class="rat-hero-terminal rat-hero-(?:wide|narrow)"[^>]*>([\s\S]*?)<\/div>/g)]
        .map((match) => match[1]);
      expect(buffers).toHaveLength(2);
      expect(buffers[0] === previews.native).toBe(true);
      expect(buffers[1] === previews["40"]).toBe(true);
    }
  });
});
