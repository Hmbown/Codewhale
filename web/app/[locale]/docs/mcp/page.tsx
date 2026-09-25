import { DocArticle } from "../_components/doc-article";
import { getDocsMcp } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsMcp(locale);
  return buildPageMetadata({
    path: "/docs/mcp",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function McpPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsMcp(locale)} locale={locale} />;
}
