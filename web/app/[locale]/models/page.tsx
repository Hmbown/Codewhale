import Link from "next/link";
import { getFacts } from "@/lib/facts";
import { buildPageMetadata } from "@/lib/page-meta";
import { MODELS_COPY } from "@/lib/content/models";
import { ModelsTable } from "@/components/models-table";
import type { LocalizedText } from "@/lib/content/vocabulary";
import { fill, pickText } from "@/lib/i18n/dictionaries";

export const revalidate = 300;

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return buildPageMetadata({ path: "/models", locale,
    title: pickText(MODELS_COPY.metaTitle, locale),
    description: pickText(MODELS_COPY.metaDescription, locale) });
}

export default async function ModelsPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const facts = await getFacts();
  const t = (copy: LocalizedText) => fill(pickText(copy, locale), {
    version: facts.version ?? "—", model: facts.defaultModel ?? "—",
  });
  const providerDocs = "https://github.com/Hmbown/CodeWhale/blob/main/docs/PROVIDERS.md";

  return (
    <div className="models-page">
      <section className="hero">
        <div className="portal-current" aria-hidden="true" />
        <div className="portal-container community-welcome-inner">
          <div className="eyebrow">{t(MODELS_COPY.kicker)}</div>
          <h1>{t(MODELS_COPY.title)}</h1>
          <p>{t(MODELS_COPY.lead)}</p>
          <div className="portal-actions">
            <Link href={providerDocs} className="portal-button portal-button-primary">{t(MODELS_COPY.providerDocs)}</Link>
            <Link href={`/${locale}/install`} className="portal-button portal-button-secondary">{t(MODELS_COPY.install)}</Link>
          </div>
        </div>
      </section>

      <section className="portal-section">
        <div className="portal-container portal-section-grid">
          <div className="portal-section-copy">
            <span>{t(MODELS_COPY.setupLabel)}</span>
            <h2>{t(MODELS_COPY.setupTitle)}</h2>
            <p>{t(MODELS_COPY.setupLead)}</p>
          </div>
          <div className="portal-topic-list">
            {MODELS_COPY.patterns.map((pattern) => (
              <Link key={pattern.reference} href={providerDocs}>
                <strong>{t(pattern.title)}</strong>
                <span>{t(pattern.detail)}</span>
                <span className="font-mono break-all">{pattern.reference}</span>
              </Link>
            ))}
          </div>
        </div>
      </section>

      <section className="portal-section portal-section-muted" aria-labelledby="models-title">
        <div className="portal-container">
          <div className="portal-docs-heading">
            <h2 id="models-title">{t(MODELS_COPY.modelsTitle)}</h2>
            <Link href={providerDocs}>{t(MODELS_COPY.providerDocs)}</Link>
          </div>
          <p className="mb-6 max-w-3xl text-ink-soft leading-relaxed">{t(MODELS_COPY.modelsLead)}</p>
          <ModelsTable models={facts.models} locale={locale} />
        </div>
      </section>

      <section className="portal-section" aria-labelledby="providers-title">
        <div className="portal-container">
          <div className="portal-docs-heading">
            <h2 id="providers-title">{t(MODELS_COPY.listTitle)}</h2>
            <Link href={providerDocs}>{t(MODELS_COPY.providerDocs)}</Link>
          </div>
          <p className="mb-6 max-w-3xl text-ink-soft leading-relaxed">{t(MODELS_COPY.listLead)}</p>
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm">
              <caption className="sr-only">{t(MODELS_COPY.listTitle)}</caption>
              <thead>
                <tr className="hairline-b">
                  <th scope="col" className="py-3 pr-4">{t(MODELS_COPY.provider)}</th>
                  <th scope="col" className="py-3 pr-4">{t(MODELS_COPY.id)}</th>
                  <th scope="col" className="py-3">{t(MODELS_COPY.credential)}</th>
                </tr>
              </thead>
              <tbody>
                {facts.providers.map((provider) => (
                  <tr key={provider.id} className="hairline-b">
                    <th scope="row" className="py-3 pr-4 font-medium">{provider.label}</th>
                    <td className="py-3 pr-4"><code className="break-all">{provider.id}</code></td>
                    <td className="py-3"><code className="break-all">{provider.env}</code></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="mt-6 text-ink-soft">
            {t(MODELS_COPY.missing)}{" "}
            <Link href="https://github.com/Hmbown/CodeWhale/issues/new/choose" className="body-link">{t(MODELS_COPY.request)}</Link>
          </p>
        </div>
      </section>
    </div>
  );
}
