import { DocArticle } from "../_components/doc-article";
import { getDocsWork } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

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

export default async function WorkPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsWork(locale)} locale={locale} />;
}
