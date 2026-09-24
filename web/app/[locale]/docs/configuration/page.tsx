import { Fragment } from "react";
import { getDocsConfiguration, splitTokens } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

const CODE_SPANS: Record<string, string> = {
  auditCommand: "/config audit",
  apiKey: "--api-key",
  authStatus: "codewhale auth status",
  providerConfig: 'provider = "<id>"',
  providerFlag: "codewhale --provider <id>",
};

function RichCopy({ text }: { text: string }) {
  return splitTokens(text).map((part, i) =>
    "token" in part ? (
      <code key={`${i}-${part.token}`} className="inline">
        {CODE_SPANS[part.token] ?? `{${part.token}}`}
      </code>
    ) : (
      <Fragment key={`${i}-text`}>{part.text}</Fragment>
    ),
  );
}

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsConfiguration(locale);
  return buildPageMetadata({
    path: "/docs/configuration",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function ConfigurationPage({
  params,
}: {
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  const t = getDocsConfiguration(locale);

  return (
    <section className="space-y-10">
      <section id="overview" className="scroll-mt-32">
        <h1 className="font-display text-3xl mb-1">{t.overviewTitle}</h1>
        <p className={`${t.bodyClassName} mt-3`}>{t.overviewLead}</p>
        <pre className="code-block mt-4">{`codewhale --config /path/to/config.toml
CODEWHALE_CONFIG_PATH=/path/to/config.toml`}</pre>
        <p className={`${t.bodyClassName} mt-3`}>
          <RichCopy text={t.auditLead} />
        </p>
      </section>

      <section id="project-overlay" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.overlayTitle}</h2>
        <p className={`${t.bodyClassName} mt-3`}>{t.overlayLead}</p>
        <p className={`${t.bodyClassName} mt-3`}>{t.overlayLimits}</p>
      </section>

      <section id="credentials" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.credentialsTitle}</h2>
        <p className={`${t.bodyClassName} mt-3`}>
          <RichCopy text={t.credentialsLead} />
        </p>
      </section>

      <section id="legacy-paths" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.legacyTitle}</h2>
        <p className={`${t.bodyClassName} mt-3`}>{t.legacyLead}</p>
      </section>

      {/* Maintainer pointer: kept out of the rendered copy (experience mark 5). */}
      <div hidden data-source-note={t.sourceNote} />
    </section>
  );
}
