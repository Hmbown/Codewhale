import { Fragment } from "react";
import { getDocsWork, splitTokens } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

const CODE_SPANS: Record<string, string> = {
  todoWrite: "todo_write",
  checklistAlias: "checklist_*",
  todoAlias: "todo_*",
  pending: "[ ]",
  inProgress: "[~]",
  completed: "[✓]",
  cancelled: "[-]",
};

function withCodeSpans(template: string) {
  return splitTokens(template).map((part, i) =>
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
  const t = getDocsWork(locale);
  return buildPageMetadata({
    path: "/docs/work",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function WorkSurfacePage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsWork(locale);
  const bodyClass = t.bodyClassName;

  return (
    <section className="space-y-10">
      <section id="overview" className="scroll-mt-32">
        <h1 className="font-display text-3xl mb-1">{t.overviewTitle}</h1>
        <p className={`${bodyClass} mt-3`}>{t.overviewLead}</p>
      </section>

      <section id="checklist" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.checklistTitle}</h2>
        <p className={`${bodyClass} mt-3`}>{withCodeSpans(t.checklistBody)}</p>
      </section>

      <section id="strategy" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.strategyTitle}</h2>
        <p className={`${bodyClass} mt-3`}>{t.strategyLead}</p>
      </section>

      <section id="continuity" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.continuityTitle}</h2>
        <p className={`${bodyClass} mt-3`}>{t.continuityLead}</p>
      </section>

      <section id="capture" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.captureTitle}</h2>
        <p className={`${bodyClass} mt-3`}>{t.captureLead}</p>
        <pre className="code-block mt-4">{`To-do
◆ Goal: Land the v0.9.2 website docs cluster
elapsed: 18m
[█████████░░░░░░░░░░░] 45%
50% settled (2/4)
[✓] #1 Read docs-map.ts and the Modes page pattern
[✓] #2 Draft the Fleet and Sandbox pages
[~] #3 Write the Work surface page
[ ] #4 Run check:docs, tests, and the build`}</pre>
        <p className={`${bodyClass} mt-3`}>{withCodeSpans(t.captureLegend)}</p>
      </section>

      <section id="model-facing" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t.modelFacingTitle}</h2>
        <p className={`${bodyClass} mt-3`}>{t.modelFacingLead}</p>
        <p className={`${bodyClass} mt-3`}>{t.modelFacingBoundaries}</p>
      </section>

      {/* Maintainer pointer: kept out of the rendered copy (experience mark 5). */}
      <div hidden data-source-note={t.sourceNote} />
    </section>
  );
}
