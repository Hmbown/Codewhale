import Link from "next/link";
import { Icon } from "@/components/icon";
import { PageHeader } from "@/components/page-header";
import { Status } from "@/components/status-badge";
import { EmptyState } from "@/components/surface-state";
import { CHANGELOG, type ChangelogRelease } from "@/lib/changelog.generated";
import { changelogAnchor } from "@/scripts/changelog-lib.mjs";
import { getFacts } from "@/lib/facts";
import { fill, getChangelog, getChrome } from "@/lib/i18n/dictionaries";
import { REPO_RELEASES_URL, REPO_URL } from "@/lib/i18n/links";
import { buildPageMetadata } from "@/lib/page-meta";

// The two headline facts come from the facts layer, which may be refreshed
// from the KV snapshot after deploy; the notes are build-time from CHANGELOG.md.
export const revalidate = 300;

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getChangelog(locale);
  return buildPageMetadata({
    path: "/changelog",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

/** Link labels carry a trailing ↗ in the dictionaries; the icon draws it. */
function ext(label: string) {
  return (
    <>
      {label.replace(/\s*↗\s*$/u, "")}
      <Icon name="external" className="icon" />
    </>
  );
}

function formatDate(iso: string, dateLocale: string): string {
  return new Date(iso).toLocaleDateString(dateLocale, {
    year: "numeric",
    month: "short",
    day: "numeric",
    // Dates come from ISO timestamps and YYYY-MM-DD headings; render them
    // as the UTC day they name, not the server's local day.
    timeZone: "UTC",
  });
}

export default async function ChangelogPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getChangelog(locale);
  const chrome = getChrome(locale);
  const facts = await getFacts();
  const published = facts.latestPublishedRelease;
  const candidate = facts.version;
  const candidateIsPublished = published !== null && candidate !== null && published.version === candidate;

  return (
    <>
      <PageHeader kicker={t.kicker} title={t.title} lede={t.lead} pose="write">
        <dl className="def-rows changelog-facts">
          <div className="def-row" data-published={published ? "true" : "false"}>
            <dt>{t.publishedLabel}</dt>
            <dd>
              {published ? (
                <>
                  <Status tone="ready">
                    {fill(t.publishedValue, {
                      tag: published.tag,
                      date: formatDate(published.publishedAt, chrome.dateLocale),
                    })}
                  </Status>
                  <a href={published.url} target="_blank" rel="noreferrer" className="link">
                    {ext(t.releasePageLink)}
                  </a>
                </>
              ) : (
                <a href={REPO_RELEASES_URL} target="_blank" rel="noreferrer" className="link">
                  {ext(t.releasesLink)}
                </a>
              )}
            </dd>
          </div>
          <div className="def-row" data-candidate={candidateIsPublished ? "published" : "unreleased"}>
            <dt>{t.candidateLabel}</dt>
            <dd>
              {candidate && (
                <Status tone={candidateIsPublished ? "ready" : "idle"}>
                  {candidateIsPublished
                    ? fill(t.candidateMatches, { version: `v${candidate}` })
                    : fill(t.candidateValue, { version: `v${candidate}` })}
                </Status>
              )}
              <a href={`${REPO_URL}/blob/main/CHANGELOG.md`} target="_blank" rel="noreferrer" className="link">
                {ext(t.fullNotes)}
              </a>
            </dd>
          </div>
        </dl>
      </PageHeader>

      <div className="page-body">
        <div className="page-body-narrow changelog-list">
          {CHANGELOG.length === 0 ? (
            <EmptyState
              locale={locale}
              title={t.emptyTitle}
              body={t.emptyBody}
              action={
                <a className="btn btn-secondary" href={REPO_RELEASES_URL}>
                  {ext(t.releasesLink)}
                </a>
              }
            />
          ) : (
            CHANGELOG.map((release) => (
              <Release
                key={release.version}
                release={release}
                locale={locale}
                publishedUrl={
                  published && published.version === release.version ? published.url : null
                }
              />
            ))
          )}
          <p className="status-line">
            <Link href={`/${locale}/docs`} className="link">
              {chrome.footerDocs}
            </Link>
            <a href={REPO_RELEASES_URL} target="_blank" rel="noreferrer" className="link">
              {ext(t.releasesLink)}
            </a>
          </p>
        </div>
      </div>
    </>
  );
}

function Release({
  release,
  locale,
  publishedUrl,
}: {
  release: ChangelogRelease;
  locale: string;
  publishedUrl: string | null;
}) {
  const t = getChangelog(locale);
  const chrome = getChrome(locale);
  const heading = release.unreleased ? t.unreleasedHeading : `v${release.version}`;
  const anchor = release.unreleased ? "unreleased" : `v${release.version}`;
  // Every entry here is clipped or capped for the web; this is the in-page
  // path to the unabridged notes for exactly this version.
  const notesUrl = `${REPO_URL}/blob/main/CHANGELOG.md#${changelogAnchor(release)}`;

  return (
    <article id={anchor} className="changelog-release scroll-mt-32">
      <div className="changelog-release-head">
        {/* Each release is a receipt: the shell marks it. */}
        <h2>
          <Icon name="shell" />
          {heading}
        </h2>
        <div className="changelog-links">
          {release.date && <span className="tabular">{formatDate(release.date, chrome.dateLocale)}</span>}
          {publishedUrl && (
            <a href={publishedUrl} target="_blank" rel="noreferrer">
              {ext(t.releasePageLink)}
            </a>
          )}
          {release.compareUrl && (
            <a href={release.compareUrl} target="_blank" rel="noreferrer">
              {ext(t.compareLink)}
            </a>
          )}
          <a href={notesUrl} target="_blank" rel="noreferrer">
            {ext(fill(t.releaseNotesLink, { version: heading }))}
          </a>
        </div>
      </div>
      {release.unreleased && <p className="changelog-unreleased-note">{t.unreleasedNote}</p>}
      <div className="changelog-sections">
        {release.sections.map((section, i) => (
          <section key={`${section.heading}-${i}`}>
            <h3>
              {section.heading}
              {section.itemCount > section.items.length && (
                <a href={notesUrl} target="_blank" rel="noreferrer">
                  {fill(t.moreEntries, { shown: section.items.length, total: section.itemCount })}
                </a>
              )}
            </h3>
            <ul>
              {section.items.map((item, j) => (
                <li key={j}>{item}</li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    </article>
  );
}
