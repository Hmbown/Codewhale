import Link from "next/link";
import { InstallCodeBlock } from "@/components/install-code-block";
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

export default async function InstallPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const home = getHome(locale);
  return (
    <article className="install-guide" lang="en">
      <div className="install-guide-source" lang={pickTextLocale(locale)}>
        <a href="https://github.com/Hmbown/CodeWhale/blob/main/docs/INSTALL.md">
          {pickText(INSTALL_COPY.source, locale)}
        </a>
        <p>{pickText(INSTALL_COPY.translationNotice, locale)}</p>
      </div>
      {INSTALL_GUIDE.chunks.map((chunk, index) => chunk.kind === "code" ? (
        <InstallCodeBlock key={index} cmd={chunk.text} copyLabel={home.copy} copiedLabel={home.copied} copyLocale={locale} />
      ) : (
        // HTML is generated at build time with strict raw-HTML and URL guards.
        <div key={index} dangerouslySetInnerHTML={{ __html: chunk.text }} />
      ))}
      <p lang={pickTextLocale(locale)}>
        <Link href={`/${locale}/docs/configuration`}>{pickText(INSTALL_COPY.configuration, locale)}</Link>
      </p>
    </article>
  );
}
