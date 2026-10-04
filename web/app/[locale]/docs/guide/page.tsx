import Link from "next/link";
import { GettingStartedSteps } from "@/components/getting-started-steps";
import { SessionMedia } from "@/components/session-media";
import { NativeTerminalGallery } from "@/components/native-terminal-gallery";
import { GUIDE_NEXT_LINKS } from "@/lib/content/getting-started";
import { fill, getDocsGuide, getHome, pickText } from "@/lib/i18n/dictionaries";
import { getMediaAsset, TERMINAL_SCREENSHOT } from "@/lib/media-manifest";
import { buildPageMetadata } from "@/lib/page-meta";
import { getNativeTerminalCopy } from "@/lib/content/native-terminal";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsGuide(locale);
  return buildPageMetadata({
    path: "/docs/guide",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function GuidePage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsGuide(locale);
  const session = getMediaAsset("first-fleet-session");
  const terminal = getNativeTerminalCopy(locale);
  const home = getHome(locale);

  return (
    <section className="space-y-10">
      <section id="overview" className="scroll-mt-32">
        <h1 className="font-display text-3xl mb-1">{t.overviewTitle}</h1>
        <p className={`${t.bodyClassName} mt-3`}>{t.overviewLead}</p>
      </section>

      <section id="path" className="scroll-mt-32">
        <GettingStartedSteps locale={locale} headingLevel={2} />
      </section>

      <section id="terminal-views" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{terminal.title}</h2>
        <p className={`${t.bodyClassName} mt-3 mb-4`}>{terminal.description}</p>
        <div className="figure-frame">
          <NativeTerminalGallery
            locale={locale}
            defaultFrame="home"
            regionLabel={home.shotPreview}
            label={fill(home.screenshotAlt, { version: TERMINAL_SCREENSHOT.version })}
          />
        </div>
      </section>

      {session?.status === "published" && (
        <section id="session-media" className="scroll-mt-32">
          <h2 className="font-display text-2xl mb-1">{t.sessionTitle}</h2>
          <p className={`${t.bodyClassName} mt-3 mb-4`}>{t.sessionLead}</p>
          <SessionMedia asset={session} locale={locale} />
        </section>
      )}

      <section id="next" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.nextTitle}</h2>
        <div className="hairline-t mt-4">
          {GUIDE_NEXT_LINKS.map((item) => (
            <div key={item.href} className="py-4 hairline-b">
              <h3 className="font-display text-xl">
                <Link href={`/${locale}${item.href}`} className="hover:text-indigo transition-colors">
                  {pickText(item.label, locale)}
                </Link>
              </h3>
              <p className={`${t.bodyClassName} mt-1 text-sm`}>{pickText(item.note, locale)}</p>
            </div>
          ))}
        </div>
      </section>

      {/* Maintainer pointer: kept out of the rendered copy (experience mark 5). */}
      <div hidden data-source-note={t.sourceNote} />
    </section>
  );
}
