import Link from "next/link";
import { getFacts } from "@/lib/facts";
import { WorkSurfaceLauncher } from "@/components/work-surface-launcher";
import { PRODUCT_COPY } from "@/lib/content/product";
import { fill, getHome, pickText } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export const revalidate = 300;

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return buildPageMetadata({
    path: "/product",
    locale,
    title: pickText(PRODUCT_COPY.metadata.title, locale),
    description: pickText(PRODUCT_COPY.metadata.description, locale),
  });
}

/**
 * /product — what Codewhale is and what a person gains, with availability
 * stated per surface. Copy is the `PRODUCT_COPY` content module; counts come
 * from the facts layer; the surface list is the same dictionary the homepage
 * renders, so the two never drift.
 */
export default async function ProductPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = (text: { en: string; zh: string }) => pickText(text, locale);
  const facts = await getFacts();
  const home = getHome(locale);
  const providerCount = facts.providers.length;

  return (
    <div className="portal-home product-page">
      <section className="hero">
        <div className="portal-container community-welcome-inner">
          <h1>{t(PRODUCT_COPY.title)}</h1>
          <p>{t(PRODUCT_COPY.lede)}</p>
          <div className="portal-actions">
            <Link href={`/${locale}/install`} className="portal-button portal-button-primary">
              {t(PRODUCT_COPY.actions.install)}
            </Link>
            <Link href={`/${locale}/docs`} className="portal-button portal-button-secondary">
              {t(PRODUCT_COPY.actions.docs)}
            </Link>
          </div>
        </div>
      </section>

      <div className="product-container product-launcher"><WorkSurfaceLauncher locale={locale} /></div>

      <section className="folio-section">
        <div className="product-container">
          <h2>{t(PRODUCT_COPY.gainHeading)}</h2>
          <div className="folio-gain-grid">
            {PRODUCT_COPY.gain.map((row) => (
              <div key={row.title.en}>
                <h3>{t(row.title)}</h3>
                <p>{fill(t(row.body), { count: providerCount })}</p>
              </div>
            ))}
          </div>
          <Link href={`/${locale}/models`} className="folio-link">
            {t(PRODUCT_COPY.actions.models)}
          </Link>
        </div>
      </section>

      <section className="folio-section product-page-surfaces">
        <div className="product-container folio-chapter-grid">
          <div>
            <h2>{t(PRODUCT_COPY.surfacesHeading)}</h2>
            <p className="folio-section-lede">{t(PRODUCT_COPY.surfacesLede)}</p>
            <Link href={`/${locale}/runtime`} className="folio-link">
              {t(PRODUCT_COPY.surfacesLink)}
            </Link>
          </div>
          <dl className="folio-fact-list">
            {home.surfaces.map(([name, description]) => (
              <div key={name}>
                <dt>{name}</dt>
                <dd>{description}</dd>
              </div>
            ))}
          </dl>
        </div>
      </section>

      <section className="folio-section">
        <div className="product-container folio-chapter-grid">
          <div>
            <h2>{t(PRODUCT_COPY.controlHeading)}</h2>
            <p className="folio-section-lede">{t(PRODUCT_COPY.controlLede)}</p>
          </div>
          <div>
            <dl className="folio-fact-list">
              {PRODUCT_COPY.modes.map((row) => (
                <div key={row.title.en}>
                  <dt>{t(row.title)}</dt>
                  <dd>{t(row.body)}</dd>
                </div>
              ))}
            </dl>
            <dl className="folio-fact-list mt-8">
              {PRODUCT_COPY.permissions.map((row) => (
                <div key={row.title.en}>
                  <dt>{t(row.title)}</dt>
                  <dd>{t(row.body)}</dd>
                </div>
              ))}
            </dl>
          </div>
        </div>
      </section>

      <section className="folio-section">
        <div className="product-container">
          <h2>{t(PRODUCT_COPY.availabilityHeading)}</h2>
          <p className="folio-section-lede">{t(PRODUCT_COPY.availabilityLede)}</p>
          <dl className="folio-availability-list product-availability-paper">
            {PRODUCT_COPY.availability.map((row) => (
              <div key={row.surface.en}>
                <dt>{t(row.surface)}</dt>
                <dd>
                  <strong>{t(row.status)}</strong>
                  {t(row.detail)}
                  {row.href && row.linkLabel && (
                    <>
                      {" "}
                      <Link href={`/${locale}${row.href}`} className="body-link">
                        {t(row.linkLabel)}
                      </Link>
                    </>
                  )}
                </dd>
              </div>
            ))}
          </dl>
        </div>
      </section>
    </div>
  );
}
