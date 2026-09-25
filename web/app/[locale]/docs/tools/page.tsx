import Link from "next/link";
import { buildPageMetadata } from "@/lib/page-meta";
import { TOOLS_COPY } from "@/lib/content/tools";
import type { LocalizedText } from "@/lib/content/vocabulary";
import { pickText } from "@/lib/i18n/dictionaries";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return buildPageMetadata({
    path: "/docs/tools", locale,
    title: pickText(TOOLS_COPY.metaTitle, locale),
    description: pickText(TOOLS_COPY.metaDescription, locale),
  });
}

export default async function ToolsPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = (copy: LocalizedText) => pickText(copy, locale);
  return (
    <section className="space-y-10">
      <section id="overview" className="scroll-mt-32">
        <h1 className="font-display text-3xl mb-1">{t(TOOLS_COPY.title)}</h1>
        <p className="text-ink-soft mt-3 leading-relaxed">{t(TOOLS_COPY.lead)}</p>
        <Link href="https://github.com/Hmbown/CodeWhale/blob/main/docs/TOOL_SURFACE.md" className="body-link">
          {t(TOOLS_COPY.reference)}
        </Link>
        <div className="hairline-t hairline-b mt-6">
          {TOOLS_COPY.rows.map((row) => (
            <div key={row.name} className="grid md:grid-cols-12 gap-0 hairline-t py-3 px-4 hover:bg-paper-deep transition-colors min-w-0">
              <div className="md:col-span-3 font-display text-sm font-semibold break-words">{row.name}</div>
              <div className="md:col-span-9 font-mono text-[0.78rem] text-ink-soft leading-relaxed break-words min-w-0">{t(row.detail)}</div>
            </div>
          ))}
        </div>
      </section>
      <section id="compatibility" className="scroll-mt-32">
        <h2 className="font-display text-2xl mb-1">{t(TOOLS_COPY.compatibilityTitle)}</h2>
        <p className="text-ink-soft mt-3 leading-relaxed">{t(TOOLS_COPY.compatibility)}</p>
        <Link href="https://github.com/Hmbown/CodeWhale/blob/main/docs/RUNTIME_SIMPLIFICATION_DESIGN.md" className="inline-block mt-3 text-xs text-indigo hover:underline">
          docs/RUNTIME_SIMPLIFICATION_DESIGN.md →
        </Link>
      </section>
    </section>
  );
}
