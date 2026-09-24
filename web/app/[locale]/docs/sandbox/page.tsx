import { Fragment } from "react";
import { getDocsSandbox, splitTokens } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

/**
 * The config values the policies paragraph typesets as inline `<code>`.
 * `docs/VOICE.md` keeps key names code-owned, so the dictionaries carry
 * `{token}`s rather than the literals.
 */
const CODE_SPANS: Record<string, string> = {
  sandboxMode: "sandbox_mode",
  readOnly: "read-only",
  workspaceWrite: "workspace-write",
  dangerFullAccess: "danger-full-access",
  externalSandbox: "external-sandbox",
};

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsSandbox(locale);
  return buildPageMetadata({
    path: "/docs/sandbox",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function SandboxPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsSandbox(locale);

  return (
    <section className="space-y-10">
      <section id="overview" className="scroll-mt-32">
        <h1 className="font-display text-3xl mb-1">{t.overviewTitle}</h1>
        <p className={`${t.bodyClassName} mt-3`}>{t.overviewLead}</p>
        <div className="hairline-t mt-6">
          {t.platforms.map(([name, detail]) => (
            <section key={name} className="py-4 hairline-b">
              <h3 className="font-display text-lg">{name}</h3>
              <p className={`${t.bodyClassName} mt-1 text-sm`}>{detail}</p>
            </section>
          ))}
        </div>
      </section>

      <section id="policies" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.policiesTitle}</h2>
        <p className={`${t.bodyClassName} mt-3`}>
          {splitTokens(t.policiesLead).map((part, i) =>
            "token" in part ? (
              <code key={`${i}-${part.token}`} className="inline">
                {CODE_SPANS[part.token] ?? `{${part.token}}`}
              </code>
            ) : (
              <Fragment key={`${i}-text`}>{part.text}</Fragment>
            ),
          )}
        </p>
        <pre className="code-block mt-4">{`# config.toml
sandbox_mode = "workspace-write"
prefer_bwrap = true            # Linux opt-in

# Canonical environment overrides
CODEWHALE_SANDBOX_MODE
CODEWHALE_SANDBOX_BACKEND
CODEWHALE_SANDBOX_URL
CODEWHALE_SANDBOX_API_KEY`}</pre>
      </section>

      <section id="diagnostics" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.diagnosticsTitle}</h2>
        <p className={`${t.bodyClassName} mt-3`}>{t.diagnosticsLead}</p>
        <p className={`${t.bodyClassName} mt-3`}>{t.diagnosticsLimits}</p>
      </section>

      {/* Maintainer pointer: kept out of the rendered copy (experience mark 5). */}
      <div hidden data-source-note={t.sourceNote} />
    </section>
  );
}
