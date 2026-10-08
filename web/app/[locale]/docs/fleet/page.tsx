import { DocArticle } from "../_components/doc-article";
import { getDocsFleet } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsFleet(locale);
  return buildPageMetadata({
    path: "/docs/fleet",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

/** "Run a workflow": Fleet roles, Workflow files, Lanes, and Fleet runs. */
export default async function FleetPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return <DocArticle t={getDocsFleet(locale)} locale={locale} />;
}
