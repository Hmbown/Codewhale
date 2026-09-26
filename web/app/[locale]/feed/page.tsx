import Link from "next/link";
import { Icon, type IconName } from "@/components/icon";
import { PageHeader } from "@/components/page-header";
import { FeedCard } from "@/components/feed-card";
import { FeedRetry } from "@/components/feed-retry";
import { EmptyState, ErrorState, UnavailableState } from "@/components/surface-state";
import { loadFeed, type FeedLoadStatus } from "@/lib/github";
import { getEnv } from "@/lib/kv";
import { getStates } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";
import type { FeedItem } from "@/lib/types";

export const revalidate = 600;

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const isZh = locale === "zh";
  return buildPageMetadata({
    path: "/feed",
    locale,
    title: isZh ? "动态 · Codewhale" : "Activity · Codewhale",
    description: isZh
      ? "来自 Hmbown/CodeWhale GitHub 仓库的议题、合并请求和发布的实时动态。"
      : "Live feed of issues, pull requests, and releases mirrored from the Hmbown/CodeWhale GitHub repo.",
  });
}

export default async function FeedPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const isZh = locale === "zh";

  const env = await getEnv();
  let feed: FeedItem[] = [];
  // Four honest answers for an empty column: GitHub answered and had nothing
  // (`ok` → empty), GitHub was not asked or refused (`skipped` / `unavailable`
  // → not loaded, retry), or the fetch itself threw (`failed` → error, retry).
  // Each column answers for itself: one refused endpoint must not tell the
  // other column that its source did not answer.
  let issuesStatus: FeedLoadStatus | "failed" = "ok";
  let pullsStatus: FeedLoadStatus | "failed" = "ok";
  try {
    const load = await loadFeed(env.GITHUB_TOKEN, 50);
    feed = load.items;
    issuesStatus = load.issuesStatus;
    pullsStatus = load.pullsStatus;
  } catch (e) {
    issuesStatus = "failed";
    pullsStatus = "failed";
    console.error("feed fetch failed", e);
  }

  const issues = feed.filter((f) => f.kind === "issue");
  const pulls = feed.filter((f) => f.kind === "pull");
  const states = getStates(locale);
  // FeedRetry first busts this route's ISR entry, because a bare server
  // re-render would serve the same cached `skipped`/`unavailable` record for
  // up to ten minutes.
  const retry = <FeedRetry label={states.retry} />;
  const columnState = (status: FeedLoadStatus | "failed") =>
    status === "failed" ? (
      <ErrorState locale={locale} compact action={retry} />
    ) : status === "ok" ? (
      <EmptyState locale={locale} compact />
    ) : (
      <UnavailableState locale={locale} compact action={retry} />
    );

  const copy = isZh
    ? {
        title: "动态",
        titleAside: "Activity",
        lede: (
          <>
            来自{" "}
            <Link href="https://github.com/Hmbown/CodeWhale" className="link">Hmbown/CodeWhale</Link>
            {" "}的议题与合并请求镜像，每十分钟刷新一次。点击任意条目跳转至 GitHub。
          </>
        ),
        pulls: "合并请求",
        issues: "议题",
        shown: (n: number) => `${n} 条`,
        actions: ["提交议题", "提交合并请求", "发起讨论"],
      }
    : {
        title: "Activity",
        titleAside: "动态",
        lede: (
          <>
            Follow issues and pull requests from{" "}
            <Link href="https://github.com/Hmbown/CodeWhale" className="link">Hmbown/CodeWhale</Link>,
            mirrored here and refreshed every ten minutes. Select any item to open it on GitHub.
          </>
        ),
        pulls: "Pull requests",
        issues: "Issues",
        shown: (n: number) => `${n} shown`,
        actions: ["Open an issue", "Open a pull request", "Start a discussion"],
      };
  const actionLinks: { href: string; icon: IconName }[] = [
    { href: "https://github.com/Hmbown/CodeWhale/issues/new/choose", icon: "alert" },
    { href: "https://github.com/Hmbown/CodeWhale/compare", icon: "git-pull-request" },
    { href: "https://github.com/Hmbown/CodeWhale/discussions/new", icon: "message" },
  ];
  const columns = [
    { id: "feed-pulls", title: copy.pulls, items: pulls, status: pullsStatus },
    { id: "feed-issues", title: copy.issues, items: issues, status: issuesStatus },
  ];

  return (
    <>
      <PageHeader
        seal="动"
        title={copy.title}
        titleAside={copy.titleAside}
        titleAsideLang={isZh ? "en" : "zh"}
        lede={copy.lede}
        pose="browse"
      />
      <div className="page-body">
        <div className="grid-2 feed-columns">
          {columns.map((column) => (
            <section key={column.id} className="feed-column" aria-labelledby={column.id}>
              <div className="feed-column-head">
                <h2 id={column.id}>{column.title}</h2>
                <span className="page-meta tabular">{copy.shown(column.items.length)}</span>
              </div>
              {column.items.length > 0 ? (
                <ul className="feed-items" role="list">
                  {column.items.map((item) => (
                    <li key={item.url}><FeedCard item={item} /></li>
                  ))}
                </ul>
              ) : (
                <div className="feed-empty">{columnState(column.status)}</div>
              )}
            </section>
          ))}
        </div>

        <section className="page-section">
          <ul className="grid-3" role="list">
            {actionLinks.map((link, index) => (
              <li key={link.href}>
                <Link href={link.href} className="dir-row feed-action">
                  <span className="dir-mark" aria-hidden="true"><Icon name={link.icon} /></span>
                  <span className="dir-text"><span className="dir-title">{copy.actions[index]}</span></span>
                  <span className="dir-action" aria-hidden="true"><Icon name="external" /></span>
                </Link>
              </li>
            ))}
          </ul>
        </section>
      </div>
    </>
  );
}
