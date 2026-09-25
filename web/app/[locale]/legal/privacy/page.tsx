import Link from "next/link";
import { LegalTabs } from "@/components/legal-tabs";
import { PageHeader } from "@/components/page-header";
import { UsagePreferenceControl } from "@/components/usage-counting";
import { USAGE_COUNTING_COPY } from "@/lib/content/usage-counting";
import { BUILD_FACTS } from "@/lib/facts";
import { fill, getLegalPrivacy, pickText } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";
import { LEGAL_UPDATED, PRIVACY_SECTIONS } from "@/lib/legal-copy";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getLegalPrivacy(locale);
  return buildPageMetadata({
    path: "/legal/privacy",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function PrivacyPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getLegalPrivacy(locale);
  return (
    <>
      <PageHeader
        kicker={t.kicker}
        title={t.title}
        meta={fill(t.updated, { date: LEGAL_UPDATED })}
      />
      <div className="page-body">
        <div className="page-body-narrow">
          <LegalTabs locale={locale} current="privacy" />
          <article className="prose legal-doc">
            <p>This policy explains how Shannon Labs handles information when you use Codewhale.</p>
            {PRIVACY_SECTIONS.map((section) => (
              <section key={section.title}>
                <h2>{section.title}</h2>
                <p>{section.body}</p>
              </section>
            ))}
            <section id="usage-counting" className="scroll-mt-32">
              <h2>{pickText(USAGE_COUNTING_COPY.heading, locale)}</h2>
              <UsagePreferenceControl locale={locale} appVersion={BUILD_FACTS.version ?? "0.0.0"} />
            </section>
            <p className="status-line">
              <Link href={`/${locale}/legal/terms`} className="link">{t.termsLink}</Link>
              <Link href={`/${locale}`} className="link">{t.homeLink}</Link>
            </p>
          </article>
        </div>
      </div>
    </>
  );
}
