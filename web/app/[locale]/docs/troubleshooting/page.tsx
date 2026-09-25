import { DocArticle } from "../_components/doc-article";
import { getDocsTroubleshooting } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsTroubleshooting(locale);
  return buildPageMetadata({
    path: "/docs/troubleshooting",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function TroubleshootingPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsTroubleshooting(locale)} locale={locale} />;
}
