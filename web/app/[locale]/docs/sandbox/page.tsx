import { DocArticle } from "../_components/doc-article";
import { getDocsSandbox } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsSandbox(locale);
  return buildPageMetadata({
    path: "/docs/sandbox",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function SandboxPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsSandbox(locale)} locale={locale} />;
}
