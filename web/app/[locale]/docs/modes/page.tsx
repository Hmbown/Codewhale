import { DocArticle } from "../_components/doc-article";
import { getDocsModes } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsModes(locale);
  return buildPageMetadata({
    path: "/docs/modes",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function ModesPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsModes(locale)} locale={locale} />;
}
