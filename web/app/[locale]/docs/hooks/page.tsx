import { Fragment } from "react";
import { getDocsHooks, splitTokens } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

/**
 * Config syntax the overview sentence typesets as inline `<code>`. These are
 * literals, not copy, so they stay here rather than in the dictionaries —
 * `configIntro` carries a `{token}` for each one.
 */
const CODE_SPANS: Record<string, string> = {
  hooksTable: "[[hooks.hooks]]",
  hooksCommand: "/hooks",
  enabledKey: "[hooks].enabled",
};

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsHooks(locale);
  return buildPageMetadata({
    path: "/docs/hooks",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function HooksPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsHooks(locale);

  return (
    <section className="space-y-10">
      <section id="overview" className="scroll-mt-32">
        <h1 className="font-display text-3xl mb-1">{t.overviewTitle}</h1>
        <p className={`${t.bodyClassName} mt-3`}>{t.overviewLead}</p>
        <p className={`${t.bodyClassName} mt-3`}>
          {splitTokens(t.configIntro).map((part, i) =>
            "token" in part ? (
              <code key={`${i}-${part.token}`} className="inline">
                {CODE_SPANS[part.token] ?? `{${part.token}}`}
              </code>
            ) : (
              <Fragment key={`${i}-text`}>{part.text}</Fragment>
            ),
          )}
        </p>
        <div className="hairline-t mt-6">
          {t.events.map(([name, detail]) => (
            <section key={name} className="py-4 hairline-b">
              <h3 className="font-display text-lg">{name}</h3>
              <p className={`${t.bodyClassName} mt-1 text-sm`}>{detail}</p>
            </section>
          ))}
        </div>
      </section>

      <section id="project" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.projectTitle}</h2>
        <p className={`${t.bodyClassName} mt-3`}>{t.projectLead}</p>
      </section>

      {/* Maintainer pointer: kept out of the rendered copy (experience mark 5). */}
      <div hidden data-source-note={t.sourceNote} />
    </section>
  );
}
