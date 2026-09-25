import { DocArticle } from "../_components/doc-article";
import { getDocsHooks } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

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
  return <DocArticle t={getDocsHooks(locale)} locale={locale} />;
}
