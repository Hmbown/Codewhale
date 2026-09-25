import Link from "next/link";
import { LegalTabs } from "@/components/legal-tabs";
import { PageHeader } from "@/components/page-header";
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
    <>
      <PageHeader
        kicker={t.kicker}
        title={t.title}
        meta={fill(t.updated, { date: LEGAL_UPDATED })}
      />
      <div className="page-body">
        <div className="page-body-narrow">
          <LegalTabs locale={locale} current="terms" />
          <article className="prose legal-doc">
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
            <p className="status-line">
              <Link href={`/${locale}/legal/privacy`} className="link">{t.privacyLink}</Link>
              <Link href={`/${locale}`} className="link">{t.homeLink}</Link>
            </p>
          </article>
        </div>
      </div>
    </>
  );
}
