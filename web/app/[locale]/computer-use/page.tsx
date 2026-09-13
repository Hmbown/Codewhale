import Image from "next/image";
import { COMPUTER_USE_COPY as COPY } from "@/lib/content/computer-use";
import type { LocalizedText } from "@/lib/content/vocabulary";
import { pickText } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";
import { COMPUTER_USE_REPO, getComputerUseRelease } from "@/lib/computer-use-release";

export const revalidate = 300;

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  return buildPageMetadata({ path: "/computer-use", locale,
    title: pickText(COPY.metaTitle, locale), description: pickText(COPY.metaDescription, locale) });
}

export default async function ComputerUsePage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = (copy: LocalizedText) => pickText(copy, locale);
  const release = await getComputerUseRelease();
  return (
    <div>
      <section className="hero">
        <div className="portal-container community-welcome-inner">
          <div className="flex items-center gap-5 mb-5">
            <Image src="/brand/computer-use.png" width={80} height={80} alt="" priority className="shrink-0" />
            <h1>{t(COPY.title)}</h1>
          </div>
          <p className="max-w-3xl">{t(COPY.lead)}</p>
          <p className="text-sm mt-4">{t(COPY.publisher)}</p>
          <div className="mt-8 max-w-3xl">
            {release.status === "ready" ? <>
              <a className="portal-button portal-button-primary" href={release.downloadUrl}>{t(COPY.download)}</a>
              <p className="text-sm mt-3">v{release.version} · {Math.ceil(release.size / 1024 / 1024)} MB</p>
              <a href={release.receiptUrl} className="body-link text-sm">{t(COPY.receipt)}</a>
            </> : <>
              <h2 className="text-xl">{t(release.status === "pending" ? COPY.pendingTitle : COPY.unavailableTitle)}</h2>
              <p className="mt-2">{t(release.status === "pending" ? COPY.pendingBody : COPY.unavailableBody)}</p>
              <a href={`${COMPUTER_USE_REPO}/releases`} className="body-link mt-3 inline-block">{t(COPY.releases)}</a>
            </>}
          </div>
          <p className="text-sm mt-6">{t(COPY.requirements)}<br />{t(COPY.included)}</p>
        </div>
      </section>

      <section className="portal-section">
        <div className="portal-container">
          <h2 className="mb-8">{t(COPY.setup)}</h2>
          <ol className="grid md:grid-cols-2 gap-x-12 gap-y-8">
            {COPY.steps.map((step, index) => <li key={step.title.en}>
              <h3 className="mb-3">{index + 1}. {t(step.title)}</h3>
              <p className="text-ink-soft leading-relaxed max-w-2xl">{t(step.body)}</p>
            </li>)}
          </ol>
        </div>
      </section>

      <section className="portal-section portal-section-muted">
        <div className="portal-container grid md:grid-cols-2 gap-12">
          <div><h2 className="mb-4">{t(COPY.controlsTitle)}</h2>
            <p className="text-ink-soft leading-relaxed">{t(COPY.controlsBody)}</p>
            <a href={`${COMPUTER_USE_REPO}/blob/main/docs/DEMO.md`} className="body-link mt-4 inline-block">{t(COPY.demo)}</a>
          </div>
          <div><h2 className="mb-4">{t(COPY.updateTitle)}</h2>
            <p className="text-ink-soft leading-relaxed">{t(COPY.updateBody)}</p>
            <a href={`${COMPUTER_USE_REPO}/blob/main/CHANGELOG.md`} className="body-link mt-4 inline-block">{t(COPY.notes)}</a>
          </div>
        </div>
      </section>

      <section className="portal-section">
        <div className="portal-container">
          <p className="text-ink-soft max-w-3xl">{t(COPY.platforms)}</p>
          <div className="portal-actions">
            <a href={`${COMPUTER_USE_REPO}/blob/main/docs/TROUBLESHOOTING.md`} className="body-link">{t(COPY.help)}</a>
            <a href={COMPUTER_USE_REPO} className="body-link">{t(COPY.source)}</a>
          </div>
        </div>
      </section>
    </div>
  );
}
