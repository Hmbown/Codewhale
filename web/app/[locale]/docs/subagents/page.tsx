import { DocArticle } from "../_components/doc-article";
import { getDocsSubagents } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsSubagents(locale);
  return buildPageMetadata({
    path: "/docs/subagents",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function SubagentsPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsSubagents(locale)} locale={locale} />;
}
