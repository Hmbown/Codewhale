import { DocArticle } from "../_components/doc-article";
import { getDocsWeb } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsWeb(locale);
  return buildPageMetadata({
    path: "/docs/web",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function WebClientPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsWeb(locale)} locale={locale} />;
}
