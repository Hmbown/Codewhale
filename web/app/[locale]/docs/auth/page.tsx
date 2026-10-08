import { DocArticle } from "../_components/doc-article";
import { getDocsAuth } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsAuth(locale);
  return buildPageMetadata({
    path: "/docs/auth",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function AuthPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsAuth(locale)} locale={locale} />;
}
