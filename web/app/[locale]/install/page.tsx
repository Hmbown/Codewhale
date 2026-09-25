import Link from "next/link";
import { HeroInstall } from "@/components/hero-install";
import { Icon } from "@/components/icon";
import { InstallCodeBlock } from "@/components/install-code-block";
import { WhalePose } from "@/components/whale-pose";
import { INSTALL_COPY } from "@/lib/content/install";
import { INSTALL_GUIDE } from "@/lib/install-guide.generated";
import { getHome, pickText, pickTextLocale } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return buildPageMetadata({
    path: "/install",
    locale,
    title: pickText(INSTALL_COPY.metaTitle, locale),
    description: pickText(INSTALL_COPY.metaDescription, locale),
  });
}

/**
 * /install — the quick install plate first (the two one-line installs the
 * home page offers), then the verified guide from docs/INSTALL.md, which owns
 * the page's heading and every other channel. The guide stays in English
 * until its translation is verified; the notice says so.
 */
export default async function InstallPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const home = getHome(locale);
  const copyLocale = pickTextLocale(locale);
  return (
    <>
      <div className="install-head" lang={copyLocale}>
        <div className="install-head-inner">
          <WhalePose pose="run" className="install-head-pose" priority />
          <div className="install-head-text">
            <HeroInstall ariaLabel={home.heroInstallAria} copyLabel={home.copy} copiedLabel={home.copied} />
            <p className="install-head-source">
              <Icon name="check" className="icon" />
              <a href="https://github.com/Hmbown/CodeWhale/blob/main/docs/INSTALL.md" className="link">
                {pickText(INSTALL_COPY.source, locale)}
              </a>
            </p>
            {/* The notice speaks about the Chinese translation, so only the
                Chinese page shows it. */}
            {copyLocale === "zh" ? (
              <p className="install-head-note">{pickText(INSTALL_COPY.translationNotice, locale)}</p>
            ) : null}
          </div>
        </div>
      </div>
      <article className="install-guide" lang="en">
        {INSTALL_GUIDE.chunks.map((chunk, index) => chunk.kind === "code" ? (
          <InstallCodeBlock key={index} cmd={chunk.text} copyLabel={home.copy} copiedLabel={home.copied} copyLocale={locale} />
        ) : (
          // HTML is generated at build time with strict raw-HTML and URL guards.
          <div key={index} dangerouslySetInnerHTML={{ __html: chunk.text }} />
        ))}
        <p lang={copyLocale}>
          <Link href={`/${locale}/docs/configuration`} className="section-link">
            {pickText(INSTALL_COPY.configuration, locale)}
            <Icon name="arrow-right" className="icon icon-flip" />
          </Link>
        </p>
      </article>
    </>
  );
}
