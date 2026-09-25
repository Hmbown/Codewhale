import { DocArticle } from "../_components/doc-article";
import { getDocsTrust } from "@/lib/i18n/dictionaries";
import { buildPageMetadata, SITE_SECURITY_EMAIL } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getDocsTrust(locale);
  return buildPageMetadata({
    path: "/docs/trust",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

export default async function TrustPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return (
    <DocArticle
      t={getDocsTrust(locale)}
      locale={locale}
      extra={{
        // The contact is code-owned so every locale reports to the same inbox.
        report: (
          <div className="portal-actions">
            <a className="portal-button portal-button-primary" href={`mailto:${SITE_SECURITY_EMAIL}`}>
              {SITE_SECURITY_EMAIL}
            </a>
          </div>
        ),
      }}
    />
  );
}
