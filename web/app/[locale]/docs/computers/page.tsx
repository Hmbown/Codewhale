import { DocArticle } from "../_components/doc-article";
import { getDocsComputers } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsComputers(locale);
  return buildPageMetadata({
    path: "/docs/computers",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function ComputersPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsComputers(locale)} locale={locale} />;
}
