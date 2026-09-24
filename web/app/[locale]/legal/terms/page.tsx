import Link from "next/link";
import { fill, getLegalTerms } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";
import { LEGAL_UPDATED, TERMS_SECTIONS } from "@/lib/legal-copy";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getLegalTerms(locale);
  return buildPageMetadata({
    path: "/legal/terms",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function TermsPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getLegalTerms(locale);
  return (
    <div className="portal-home">
      <article className="legal-doc">
        <p className="legal-doc-kicker">{t.kicker}</p>
        <h1>{t.title}</h1>
        <p className="legal-doc-updated">{fill(t.updated, { date: LEGAL_UPDATED })}</p>
        <p>
          These terms govern your use of Codewhale, a Shannon Labs product. By creating an
          account or using the service, you agree to them.
        </p>
        {TERMS_SECTIONS.map((section) => (
          <section key={section.title}>
            <h2>{section.title}</h2>
            <p>{section.body}</p>
          </section>
        ))}
        <p className="legal-doc-nav">
          <Link href={`/${locale}/legal/privacy`}>{t.privacyLink}</Link>
          <Link href={`/${locale}`}>{t.homeLink}</Link>
        </p>
      </article>
    </div>
  );
}
