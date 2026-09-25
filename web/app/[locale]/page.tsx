import { Fragment } from "react";
import Image from "next/image";
import Link from "next/link";
import { GettingStartedSteps } from "@/components/getting-started-steps";
import { HeroInstall } from "@/components/hero-install";
import { Icon, type IconName } from "@/components/icon";
import { InstallCodeBlock } from "@/components/install-code-block";
import { Status, type StatusTone } from "@/components/status-badge";
import { WhalePose } from "@/components/whale-pose";
import { getFacts } from "@/lib/facts";
import { GETTING_STARTED_STEPS } from "@/lib/content/getting-started";
import { fill, getHome, splitToken } from "@/lib/i18n/dictionaries";
import {
  APP_SIGNUP_URL,
  DISCORD_URL,
  REPO_ISSUES_URL,
  REPO_RELEASES_URL,
  REPO_URL,
} from "@/lib/i18n/links";
import { serializeJsonLd } from "@/lib/json-ld";
import { buildSoftwareApplicationJsonLd } from "@/lib/software-application-schema";
import { TERMINAL_SCREENSHOT } from "@/lib/media-manifest";

// Revalidate against source-proven runtime facts without giving up static edge
// caching. `getFacts()` rejects legacy or older KV snapshots.
export const revalidate = 300;

// Row order is shared by every locale's `gain`, `surfaces` and
// `availability` lists, so the marks and states follow the row, not a word.
const GAIN_ICONS: IconName[] = ["terminal", "repeat", "shield"];
const SURFACE_ICONS: IconName[] = ["terminal", "plug", "monitor", "folder", "users"];
// Released · development preview · development build · in development.
const AVAILABILITY_TONES: StatusTone[] = ["ready", "attention", "idle", "idle"];

/**
 * The whale-road homepage: the promise and the install plate in the sky,
 * the whale resting on one calm horizon, and everything else in the sea
 * below it, which continues into the footer.
 *
 * Every visible string resolves through `getHome(locale)`. The only literals
 * left here are code-owned per docs/VOICE.md: the product control vocabulary
 * (`Plan · Work · Operate`, `Ask · Auto-Review · Full Access`), package
 * channel proper nouns, and the screenshot path.
 */
export default async function HomePage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const d = getHome(locale);
  const facts = await getFacts();
  const sourceVersion = facts.version ?? "unknown";
  const publishedRelease = facts.latestPublishedRelease;
  const sourceIsPublished = publishedRelease?.version === sourceVersion;

  // The install URL resolves published artifacts, so its structured version
  // must come from the published-release receipt rather than source-candidate
  // facts. When no release is known, the schema omits softwareVersion.
  const jsonLd = buildSoftwareApplicationJsonLd(publishedRelease);

  // The lede typesets the brand in its own span. Splitting on the {brand}
  // token keeps the sentence a single translated unit — no concatenation of
  // fragments around a variable, and a locale may place the brand anywhere.
  const ledeParts = splitToken(d.heroIntro, "brand");

  return (
    <div className="home">
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: serializeJsonLd(jsonLd) }}
      />

      {/* THE SKY — the promise, the one primary action, the install plate,
          and the whale resting on the horizon. */}
      <section className="home-hero" aria-labelledby="home-title">
        <div className="home-sky">
          <div className="home-hero-inner">
            <div className="home-hero-copy">
              <h1 id="home-title" className="home-title">{d.heroTitle}</h1>
              <p className="home-lede">
                {ledeParts.map((part, index) => (
                  <Fragment key={index}>
                    {index > 0 && <strong>Codewhale</strong>}
                    {part}
                  </Fragment>
                ))}
              </p>
              <div className="actions">
                <Link href={`/${locale}/install`} className="btn btn-primary btn-lg">
                  {d.getCodewhale}
                </Link>
                <Link href={`/${locale}/product`} className="btn btn-ghost btn-lg">
                  {d.exploreProduct}
                  <Icon name="arrow-right" className="icon icon-flip" />
                </Link>
              </div>
              <HeroInstall ariaLabel={d.heroInstallAria} copyLabel={d.copy} copiedLabel={d.copied} />
            </div>
            <div className="home-whale">
              <WhalePose pose="rest" priority />
            </div>
          </div>
        </div>

        {/* THE HORIZON — one straight line; nothing stands on it but the whale. */}
        <div className="horizon" aria-hidden="true" />
      </section>

      {/* THE SEA — deep water in dark, continuing into the footer; shallow
          water settling back to paper in light. */}
      <div className="home-sea sea-continues">
        <div className="sea-texture" aria-hidden="true" />
        <div className="home-reflection" aria-hidden="true">
          <WhalePose pose="rest" />
        </div>

        <div className="home-sea-body">
          {/* The real terminal, just under the surface. Exact-build PTY
              capture of an empty session: no fabricated conversation,
              connected tools or completion metrics. */}
          <section className="home-section" aria-label={d.shotPreview}>
            <figure className="figure home-shot">
              <div className="figure-frame">
                <Image
                  src={TERMINAL_SCREENSHOT.src}
                  alt={fill(d.screenshotAlt, { version: TERMINAL_SCREENSHOT.version })}
                  width={TERMINAL_SCREENSHOT.width}
                  height={TERMINAL_SCREENSHOT.height}
                  sizes="(max-width: 62rem) calc(100vw - 2rem), 60rem"
                  unoptimized
                />
              </div>
              <figcaption className="figure-caption">
                <span>
                  {d.shotPreview} · {fill(d.shotBuild, { version: TERMINAL_SCREENSHOT.version })}
                </span>
                {/* Each fact is its own translated unit; nothing is
                    concatenated around a token. */}
                <span
                  className="status-line"
                  data-source-state={sourceIsPublished ? "published release" : "source candidate"}
                  data-source-state-label={sourceIsPublished ? d.publishedRelease : d.figcaptionSourceCandidate}
                >
                  <Status tone={publishedRelease ? "ready" : "idle"}>
                    {publishedRelease
                      ? fill(d.latestRelease, { tag: publishedRelease.tag })
                      : d.releaseUnavailable}
                  </Status>
                  <span>{`${sourceIsPublished ? d.currentSource : d.sourceCandidate} v${sourceVersion}`}</span>
                  <span>{facts.license ?? "MIT"}</span>
                </span>
              </figcaption>
            </figure>
          </section>

          {/* WHAT YOU CAN DO */}
          <section className="home-section" aria-labelledby="home-gain">
            <div className="section-head">
              <div className="section-head-text">
                <h2 className="section-title" id="home-gain">{d.gainHeading}</h2>
                <p className="section-scope">{d.gainLede}</p>
              </div>
            </div>
            <div className="grid-3">
              {d.gain.map(([title, body], index) => (
                <div key={title} className="tile">
                  <span className="tile-icon" aria-hidden="true">
                    <Icon name={GAIN_ICONS[index] ?? "terminal"} />
                  </span>
                  <h3>{title}</h3>
                  <p>{body}</p>
                </div>
              ))}
            </div>
          </section>

          {/* TOOLS, CONNECTED APPS, SAVED WORK */}
          <section className="home-section" aria-labelledby="home-surfaces">
            <div className="section-head">
              <div className="section-head-text">
                <h2 className="section-title" id="home-surfaces">{d.surfacesHeading}</h2>
              </div>
              <Link href={`/${locale}/runtime`} className="section-link">
                {d.runtimeLink}
                <Icon name="arrow-right" className="icon icon-flip" />
              </Link>
            </div>
            <ul className="dir-list dir-list-card" role="list">
              {d.surfaces.map(([name, description], index) => (
                <li key={name}>
                  <div className="dir-row">
                    <span className="dir-mark" aria-hidden="true">
                      <Icon name={SURFACE_ICONS[index] ?? "terminal"} />
                    </span>
                    <span className="dir-text">
                      <span className="dir-title">{name}</span>
                      <span className="dir-purpose">{description}</span>
                    </span>
                  </div>
                </li>
              ))}
            </ul>
          </section>

          {/* YOUR MODELS */}
          <section className="home-section" aria-labelledby="home-models">
            <div className="split">
              <div className="section-head-text">
                <h2 className="section-title" id="home-models">{d.modelsHeading}</h2>
                <p className="section-scope">{d.modelsBody}</p>
                <Link href={`/${locale}/models`} className="section-link">
                  {d.modelsLink}
                  <Icon name="arrow-right" className="icon icon-flip" />
                </Link>
              </div>
              <dl className="def-rows">
                {d.modelsFacts.map(([kind, description]) => (
                  <div key={kind} className="def-row">
                    <dt>{kind}</dt>
                    <dd>{description}</dd>
                  </div>
                ))}
                <div className="def-row home-modes">
                  <dt>Plan · Work · Operate</dt>
                  <dd>Ask · Auto-Review · Full Access</dd>
                </div>
              </dl>
            </div>
          </section>

          {/* START */}
          <section className="home-section product-start" aria-labelledby="home-start">
            <div className="section-head">
              <div className="section-head-text">
                <h2 className="section-title" id="home-start">{d.startHeading}</h2>
                <p className="section-scope">{d.startLede}</p>
              </div>
            </div>
            <GettingStartedSteps locale={locale} />
            <div className="product-start-links">
              <Link href={`/${locale}/docs/guide`} className="section-link">
                {d.startGuideLink}
                <Icon name="arrow-right" className="icon icon-flip" />
              </Link>
              <Link href={`/${locale}/docs/vocabulary`} className="section-link">
                {d.startVocabularyLink}
                <Icon name="arrow-right" className="icon icon-flip" />
              </Link>
            </div>
          </section>

          {/* WHERE IT RUNS TODAY — each surface with a mark and a word. */}
          <section className="home-section" aria-labelledby="home-availability">
            <div className="section-head">
              <div className="section-head-text">
                <h2 className="section-title" id="home-availability">{d.availabilityHeading}</h2>
                <p className="section-scope">{d.availabilityLede}</p>
              </div>
            </div>
            <dl className="def-rows">
              {d.availability.map(([surface, status, detail], index) => (
                <div key={surface} className="def-row">
                  <dt>{surface}</dt>
                  <dd>
                    <Status tone={AVAILABILITY_TONES[index] ?? "idle"}>{status}</Status>
                    {detail}
                  </dd>
                </div>
              ))}
            </dl>
            <p className="section-scope mt-4">{d.availabilityNote}</p>
            <div className="actions mt-3">
              <a href={APP_SIGNUP_URL} className="section-link" data-usage="signup">
                {d.accountLink}
                <Icon name="arrow-right" className="icon icon-flip" />
              </a>
            </div>
          </section>

          {/* INSTALL — the command again, where the page ends. */}
          <section className="home-section" aria-labelledby="home-install">
            <div className="home-install">
              <div className="section-head-text">
                <h2 className="section-title" id="home-install">{d.installBandHeading}</h2>
                <p className="home-install-channels">
                  GitHub Releases ({d.binaries}) · npm · Cargo · Docker · Windows · Android / Termux · {d.chinaMirrors}
                </p>
                <Link href={`/${locale}/install`} className="section-link">
                  {d.installGuideLink}
                  <Icon name="arrow-right" className="icon icon-flip" />
                </Link>
              </div>
              <InstallCodeBlock
                cmd={GETTING_STARTED_STEPS[0].commands[0]}
                copyLabel={d.copy}
                copiedLabel={d.copied}
              />
            </div>
          </section>

          {/* COMMUNITY */}
          <section className="home-section" aria-labelledby="home-community">
            <div className="split">
              <div className="section-head-text">
                <h2 className="section-title" id="home-community">{d.communityHeading}</h2>
                <p className="section-scope">{d.communityBody}</p>
              </div>
              <nav aria-label={d.communityLinksAria}>
                <ul className="dir-list dir-list-card" role="list">
                  <li>
                    <a href={REPO_URL} className="dir-row">
                      <span className="dir-mark" aria-hidden="true"><Icon name="github" /></span>
                      <span className="dir-text"><span className="dir-title">GitHub</span></span>
                      <span className="dir-action" aria-hidden="true"><Icon name="external" /></span>
                    </a>
                  </li>
                  <li>
                    <a href={REPO_ISSUES_URL} className="dir-row">
                      <span className="dir-mark" aria-hidden="true"><Icon name="alert" /></span>
                      <span className="dir-text"><span className="dir-title">Issues</span></span>
                      <span className="dir-action" aria-hidden="true"><Icon name="external" /></span>
                    </a>
                  </li>
                  <li>
                    <a href={DISCORD_URL} className="dir-row">
                      <span className="dir-mark" aria-hidden="true"><Icon name="message" /></span>
                      <span className="dir-text"><span className="dir-title">Discord</span></span>
                      <span className="dir-action" aria-hidden="true"><Icon name="external" /></span>
                    </a>
                  </li>
                  <li>
                    <Link href={`/${locale}/contribute`} className="dir-row">
                      <span className="dir-mark" aria-hidden="true"><Icon name="git-pull-request" /></span>
                      <span className="dir-text"><span className="dir-title">{d.contribute}</span></span>
                      <span className="dir-action" aria-hidden="true"><Icon name="chevron-right" className="icon icon-flip" /></span>
                    </Link>
                  </li>
                  <li>
                    {publishedRelease ? (
                      <a href={publishedRelease.url} className="dir-row">
                        <span className="dir-mark" aria-hidden="true"><Icon name="package" /></span>
                        <span className="dir-text"><span className="dir-title">{publishedRelease.tag}</span></span>
                        <span className="dir-action" aria-hidden="true"><Icon name="external" /></span>
                      </a>
                    ) : (
                      <a href={REPO_RELEASES_URL} className="dir-row">
                        <span className="dir-mark" aria-hidden="true"><Icon name="package" /></span>
                        <span className="dir-text"><span className="dir-title">Releases</span></span>
                        <span className="dir-action" aria-hidden="true"><Icon name="external" /></span>
                      </a>
                    )}
                  </li>
                </ul>
              </nav>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
