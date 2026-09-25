import { DocArticle } from "../_components/doc-article";
import { getDocsReview } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsReview(locale);
  return buildPageMetadata({
    path: "/docs/review",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function ReviewPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsReview(locale)} locale={locale} />;
}
