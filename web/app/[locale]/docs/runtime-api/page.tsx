import { DocArticle } from "../_components/doc-article";
import { getDocsRuntimeApi } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsRuntimeApi(locale);
  return buildPageMetadata({
    path: "/docs/runtime-api",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function RuntimeApiPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsRuntimeApi(locale)} locale={locale} />;
}
