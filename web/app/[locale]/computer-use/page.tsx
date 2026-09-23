import Image from "next/image";
import { getComputerUse } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";
import { COMPUTER_USE_REPO, getComputerUseRelease } from "@/lib/computer-use-release";
import { getEnv } from "@/lib/kv";

// Rendered per request rather than through ISR: the download state must be
// right the moment a release is published, the page must never serve a
// build-time snapshot, and OpenNext's in-isolate revalidation queue was seen
// leaving stale copies in place for many minutes. The GitHub reads inside
// getComputerUseRelease stay cached for five minutes via the fetch data cache.
export const dynamic = "force-dynamic";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const copy = getComputerUse(locale);
  return buildPageMetadata({ path: "/computer-use", locale, title: copy.metaTitle, description: copy.metaDescription });
}

export default async function ComputerUsePage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getComputerUse(locale);
  const release = await getComputerUseRelease((await getEnv()).GITHUB_TOKEN);
  // The disk image is the human download; the archive stays for the updater.
  const primary = release.status === "ready" ? (release.dmg ?? { downloadUrl: release.downloadUrl, size: release.size }) : null;
  return (
    <div>
      <section className="hero">
        <div className="portal-container community-welcome-inner">
          <div className="flex items-center gap-5 mb-5">
            <Image src="/brand/computer-use.png" width={80} height={80} alt="" priority className="shrink-0" />
            <h1>{t.title}</h1>
          </div>
          <p className="max-w-3xl">{t.lead}</p>
          <p className="text-sm mt-4">{t.publisher}</p>
          <div className="mt-8 max-w-3xl">
            {release.status === "ready" && primary ? <>
              <a className="portal-button portal-button-primary" href={primary.downloadUrl}>{t.download}</a>
              <p className="text-sm mt-3">v{release.version} · {Math.ceil(primary.size / 1024 / 1024)} MB</p>
              <p className="text-sm mt-1 flex flex-wrap gap-x-4">
                <a href={release.receiptUrl} className="body-link">{t.receipt}</a>
                {release.dmg ? <a href={release.downloadUrl} className="body-link">{t.downloadZip}</a> : null}
              </p>
            </> : <>
              <h2 className="text-xl">{release.status === "pending" ? t.pendingTitle : t.unavailableTitle}</h2>
              <p className="mt-2">{release.status === "pending" ? t.pendingBody : t.unavailableBody}</p>
              <a href={`${COMPUTER_USE_REPO}/releases`} className="body-link mt-3 inline-block">{t.releases}</a>
            </>}
          </div>
          <p className="text-sm mt-6">{t.requirements}<br />{t.included}</p>
        </div>
      </section>

      <section className="portal-section">
        <div className="portal-container">
          <h2 className="mb-8">{t.setup}</h2>
          <ol className="grid md:grid-cols-2 gap-x-12 gap-y-8">
            {t.steps.map((step, index) => <li key={index}>
              <h3 className="mb-3">{index + 1}. {step.title}</h3>
              <p className="text-ink-soft leading-relaxed max-w-2xl">{step.body}</p>
            </li>)}
          </ol>
        </div>
      </section>

      <section className="portal-section portal-section-muted">
        <div className="portal-container grid md:grid-cols-2 gap-12">
          <div><h2 className="mb-4">{t.controlsTitle}</h2>
            <p className="text-ink-soft leading-relaxed">{t.controlsBody}</p>
            <a href={`${COMPUTER_USE_REPO}/blob/main/docs/DEMO.md`} className="body-link mt-4 inline-block">{t.demo}</a>
          </div>
          <div><h2 className="mb-4">{t.updateTitle}</h2>
            <p className="text-ink-soft leading-relaxed">{t.updateBody}</p>
            <a href={`${COMPUTER_USE_REPO}/blob/main/CHANGELOG.md`} className="body-link mt-4 inline-block">{t.notes}</a>
          </div>
        </div>
      </section>

      <section className="portal-section">
        <div className="portal-container">
          <p className="text-ink-soft max-w-3xl">{t.platforms}</p>
          <div className="portal-actions">
            <a href={`${COMPUTER_USE_REPO}/blob/main/docs/TROUBLESHOOTING.md`} className="body-link">{t.help}</a>
            <a href={COMPUTER_USE_REPO} className="body-link">{t.source}</a>
          </div>
        </div>
      </section>
    </div>
  );
}
