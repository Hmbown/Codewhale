import { DocArticle } from "../_components/doc-article";
import { getDocsConfiguration } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

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

export default async function ConfigurationPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsConfiguration(locale)} locale={locale} />;
}
