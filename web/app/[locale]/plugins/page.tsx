import Link from "next/link";
import { buildPageMetadata } from "@/lib/page-meta";
import { PLUGINS_COPY, PLUGIN_COMMANDS } from "@/lib/content/plugins";
import type { LocalizedText } from "@/lib/content/vocabulary";
import { pickText } from "@/lib/i18n/dictionaries";

export const revalidate = 300;

const MARKETPLACE_REPO = "https://github.com/Hmbown/codewhale-plugin-marketplace";
const AUTHORING_DOC = "https://github.com/Hmbown/CodeWhale/blob/main/docs/PLUGIN_AUTHORING.md";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return buildPageMetadata({ path: "/plugins", locale,
    title: pickText(PLUGINS_COPY.metaTitle, locale),
    description: pickText(PLUGINS_COPY.metaDescription, locale) });
}

export default async function PluginsPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = (copy: LocalizedText) => pickText(copy, locale);

  return (
    <div className="plugins-page">
      <section className="hero">
        <div className="portal-current" aria-hidden="true" />
        <div className="portal-container community-welcome-inner">
          <div className="eyebrow">{t(PLUGINS_COPY.kicker)}</div>
          <h1>{t(PLUGINS_COPY.title)}</h1>
          <p>{t(PLUGINS_COPY.lead)}</p>
          <div className="portal-actions">
            <Link href={MARKETPLACE_REPO} className="portal-button portal-button-primary">{t(PLUGINS_COPY.marketplaceCta)}</Link>
            <Link href={AUTHORING_DOC} className="portal-button portal-button-secondary">{t(PLUGINS_COPY.docsCta)}</Link>
          </div>
        </div>
      </section>

      <section className="portal-section portal-section-muted" aria-labelledby="cu-title">
        <div className="portal-container portal-section-grid">
          <div className="portal-section-copy">
            <span>{t(PLUGINS_COPY.cuLabel)}</span>
            <h2 id="cu-title">{t(PLUGINS_COPY.cuTitle)}</h2>
            <p>{t(PLUGINS_COPY.cuLead)}</p>
            <p className="mt-4"><code className="font-mono text-[0.85rem]">{t(PLUGINS_COPY.cuInstall)}</code></p>
            <p className="mt-3 text-ink-soft text-sm">{t(PLUGINS_COPY.cuToolsNote)}</p>
          </div>
          <dl className="folio-fact-list">
            {PLUGINS_COPY.cuFeatures.map((feature) => (
              <div key={t(feature.title)}>
                <dt>{t(feature.title)}</dt>
                <dd>{t(feature.detail)}</dd>
              </div>
            ))}
          </dl>
        </div>
      </section>

      <section className="portal-section" aria-labelledby="catalog-title">
        <div className="portal-container portal-section-grid">
          <div className="portal-section-copy">
            <span>{t(PLUGINS_COPY.catalogLabel)}</span>
            <h2 id="catalog-title">{t(PLUGINS_COPY.catalogTitle)}</h2>
            <p>{t(PLUGINS_COPY.catalogLead)}</p>
            <p className="mt-4"><code className="font-mono text-[0.85rem]">{t(PLUGINS_COPY.catalogBrowse)}</code></p>
          </div>
          <div className="portal-topic-list">
            {PLUGINS_COPY.catalogEntries.map((entry) => (
              <Link key={entry.reference} href={MARKETPLACE_REPO}>
                <strong>{t(entry.title)}</strong>
                <span>{t(entry.detail)}</span>
                <span className="font-mono break-all">{entry.reference}</span>
              </Link>
            ))}
          </div>
        </div>
        <div className="portal-container">
          <p className="mt-6 text-ink-soft text-sm">{t(PLUGINS_COPY.catalogNote)}</p>
        </div>
      </section>

      <section className="portal-section portal-section-muted" aria-labelledby="sources-title">
        <div className="portal-container portal-section-grid">
          <div className="portal-section-copy">
            <span>{t(PLUGINS_COPY.sourcesLabel)}</span>
            <h2 id="sources-title">{t(PLUGINS_COPY.sourcesTitle)}</h2>
            <p>{t(PLUGINS_COPY.sourcesLead)}</p>
          </div>
          <div className="portal-topic-list">
            {PLUGINS_COPY.sources.map((source) => (
              <div key={source.reference}>
                <strong>{t(source.title)}</strong>
                <span>{t(source.detail)}</span>
                <span className="font-mono break-all">{source.reference}</span>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="portal-section" aria-labelledby="trust-title">
        <div className="portal-container portal-section-grid">
          <div className="portal-section-copy">
            <span>{t(PLUGINS_COPY.trustLabel)}</span>
            <h2 id="trust-title">{t(PLUGINS_COPY.trustTitle)}</h2>
            <p>{t(PLUGINS_COPY.trustLead)}</p>
          </div>
          <div className="portal-topic-list">
            {PLUGINS_COPY.trustSteps.map((step) => (
              <div key={t(step.title)}>
                <strong>{t(step.title)}</strong>
                <span>{t(step.detail)}</span>
              </div>
            ))}
          </div>
        </div>
        <div className="portal-container">
          <p className="mt-6 text-ink-soft text-sm">{t(PLUGINS_COPY.trustNote)}</p>
        </div>
      </section>

      <section className="portal-section portal-section-muted" aria-labelledby="commands-title">
        <div className="portal-container">
          <div className="portal-docs-heading">
            <h2 id="commands-title">{t(PLUGINS_COPY.commandsTitle)}</h2>
            <Link href={AUTHORING_DOC}>{t(PLUGINS_COPY.docsCta)}</Link>
          </div>
          <p className="mb-6 max-w-3xl text-ink-soft leading-relaxed">{t(PLUGINS_COPY.commandsLead)}</p>
          <pre className="code-block text-[0.78rem]">{PLUGIN_COMMANDS.join("\n")}</pre>
        </div>
      </section>
    </div>
  );
}
