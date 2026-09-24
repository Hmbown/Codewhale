import { Fragment } from "react";
import { getDocsModes, splitTokens } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

function renderModeToken(token: string) {
  if (token === "tab") {
    return <kbd className="font-mono text-xs px-1.5 py-0.5 hairline">Tab</kbd>;
  }
  if (token === "shiftTab") {
    return <kbd className="font-mono text-xs px-1.5 py-0.5 hairline">Shift+Tab</kbd>;
  }
  if (token === "configCommand") {
    return <code className="inline">/config</code>;
  }
  return `{${token}}`;
}

function renderRichText(text: string) {
  return splitTokens(text).map((part, i) =>
    "token" in part ? (
      <Fragment key={`${i}-${part.token}`}>{renderModeToken(part.token)}</Fragment>
    ) : (
      <Fragment key={`${i}-text`}>{part.text}</Fragment>
    ),
  );
}

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsModes(locale);
  return buildPageMetadata({
    path: "/docs/modes",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function ModesPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsModes(locale);

  return (
    <section className="space-y-10">
      <section id="overview" className="scroll-mt-32">
        <h1 className="font-display text-3xl mb-1">{t.overviewTitle}</h1>
        <p className={`${t.bodyClassName} mt-3`}>{t.overviewLead}</p>
        <div className="hairline-t mt-6">
          {t.modes.map(([name, description]) => (
            <section key={name} className="py-4 hairline-b">
              <h3 className="font-display text-xl">{name}</h3>
              <p className={`${t.bodyClassName} mt-1 text-sm`}>{description}</p>
            </section>
          ))}
        </div>
      </section>

      <section id="switching" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.switchingTitle}</h2>
        <p className={`${t.bodyClassName} mt-3`}>{renderRichText(t.switchingLead)}</p>
        <p className={`${t.bodyClassName} mt-3`}>{t.switchingCommandLead}</p>
        <pre className="code-block mt-4">{`/mode plan
/mode work
/mode operate`}</pre>
      </section>

      <section id="permissions" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.permissionsTitle}</h2>
        <p className={`${t.bodyClassName} mt-3`}>{renderRichText(t.permissionsLead)}</p>
        <div className="hairline-t mt-6">
          {t.postures.map(([name, description]) => (
            <section key={name} className="py-4 hairline-b">
              <h3 className="font-display text-lg">{name}</h3>
              <p className={`${t.bodyClassName} mt-1 text-sm`}>{description}</p>
            </section>
          ))}
        </div>
      </section>

      {/* Maintainer pointer: kept out of the rendered copy (experience mark 5). */}
      <div hidden data-source-note={t.sourceNote} />
    </section>
  );
}
