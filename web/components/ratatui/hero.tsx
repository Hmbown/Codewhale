import Link from "next/link";
import type { RatatuiCopy } from "@/lib/content/ratatui";
import heroEntry from "@/public/ratatui/entries/showcase-work.json";
import type { EntryPreview } from "@/lib/ratatui/catalogue";
import "./hero.css";

const HERO_PROFILE = "dark-truecolor";
const HERO_SIZE = "native";

/**
 * The hub hero: one real library render, painted with the page HTML.
 *
 * The SVG is a generated Ratatui buffer, sanitized by the exporter and
 * validated by catalogue.test.ts (no scripts, handlers or external refs),
 * so it is safe to inline. Inlining keeps the hero visible with no client
 * fetch and no layout shift: the viewBox fixes the aspect ratio.
 */
export function RatatuiHero({ locale, copy }: { locale: string; copy: RatatuiCopy }) {
  const preview = (heroEntry as EntryPreview).previews[HERO_PROFILE][HERO_SIZE];
  const narrow = (heroEntry as EntryPreview).previews[HERO_PROFILE]["40"];
  const width = (heroEntry as EntryPreview).width;
  const height = (heroEntry as EntryPreview).height;
  return (
    <section className="rat-hero" aria-label={copy.heroLabel}>
      <figure className="rat-hero-frame">
        <div
          className="rat-hero-terminal rat-hero-wide"
          role="img"
          aria-label={copy.heroAlt}
          tabIndex={0}
          dir="ltr"
          dangerouslySetInnerHTML={{ __html: preview }}
        />
        <div
          className="rat-hero-terminal rat-hero-narrow"
          role="img"
          aria-label={copy.heroAlt}
          tabIndex={0}
          dir="ltr"
          dangerouslySetInnerHTML={{ __html: narrow }}
        />
        <figcaption className="rat-hero-caption">
          <span className="rat-hero-wide">{copy.heroCaption.replace("{width}", String(width)).replace("{height}", String(height))}</span>
          <span className="rat-hero-narrow">{copy.heroCaption.replace("{width}", "40").replace("{height}", String(height))}</span>
          <span className="rat-hero-links">
            <a href="#ratatui-explorer">{copy.heroOpenExplorer} ↓</a>
            <Link href={`/${locale}/ratatui/showcase-work`}>{copy.heroOpenComponent} →</Link>
          </span>
        </figcaption>
      </figure>
    </section>
  );
}
