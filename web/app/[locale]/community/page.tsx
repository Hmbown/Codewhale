import Link from "next/link";
import { Icon, type IconName } from "@/components/icon";
import { PageHeader, Section } from "@/components/page-header";
import { getFacts } from "@/lib/facts";
import { fill, getCommunity, pickTextLocale } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";
import { RELEASE_CONTRIBUTORS, RELEASE_HELPERS } from "@/lib/release-credits";

export const revalidate = 300;

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getCommunity(locale);
  return buildPageMetadata({
    path: "/community",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

const pathsEn = [
  {
    title: "Report a problem",
    description: "File a bug, compatibility problem, or unclear behavior with system details, reproduction steps, and any logs you can share safely.",
    cta: "File an issue",
    href: "https://github.com/codewhale-hq/CodeWhale/issues/new/choose",
  },
  {
    title: "Improve code or tests",
    description: "Pick one problem with clear edges, write the smallest patch that fixes it, and add a regression test that covers it.",
    cta: "Browse open issues",
    href: "https://github.com/codewhale-hq/CodeWhale/issues",
  },
  {
    title: "Improve documentation or translations",
    description: "Fix a wrong sentence, add an example, or help finish a language pack.",
    cta: "Open the localization guide",
    href: "https://github.com/codewhale-hq/CodeWhale/blob/main/docs/LOCALIZATION.md",
  },
  {
    title: "Reproduce and review existing work",
    description: "Try an issue or pull request on your platform and provider. Post the commands you ran, what happened, and what is still wrong.",
    cta: "Browse pull requests",
    href: "https://github.com/codewhale-hq/CodeWhale/pulls",
  },
];

const pathsZh = [
  {
    title: "报告问题",
    description: "报告 bug、兼容性问题或不清楚的行为，并附上系统信息、复现步骤和可以安全分享的日志。",
    cta: "提交 issue",
    href: "https://github.com/codewhale-hq/CodeWhale/issues/new/choose",
  },
  {
    title: "改进代码或测试",
    description: "挑一个范围清楚的问题，写最小的补丁，加一个覆盖改动的回归测试。",
    cta: "查看开放 issues",
    href: "https://github.com/codewhale-hq/CodeWhale/issues",
  },
  {
    title: "改进文档或翻译",
    description: "改正说错的地方，补一个示例，或者帮忙完成一个语言包。",
    cta: "查看本地化指南",
    href: "https://github.com/codewhale-hq/CodeWhale/blob/main/docs/LOCALIZATION.md",
  },
  {
    title: "复现并审查现有工作",
    description: "在你的平台和提供商上验证 issue 或 pull request，然后分享你运行的命令、结果和剩余问题。",
    cta: "查看 pull requests",
    href: "https://github.com/codewhale-hq/CodeWhale/pulls",
  },
];

const activityEn = [
  { title: "Repository activity", description: "Recent issues and pull requests.", path: "/feed" },
  { title: "Community digest", description: "The weekly project record, reviewed by a maintainer.", path: "/digest" },
  { title: "Public roadmap", description: "Shipped, underway, considered, and ruled-out work.", path: "/roadmap" },
];

const activityZh = [
  { title: "仓库动态", description: "最近的 issues 与 pull requests。", path: "/feed" },
  { title: "社区摘要", description: "经过维护者审核的每周项目记录。", path: "/digest" },
  { title: "公开路线图", description: "已发布、正在进行、考虑中和明确不做的工作。", path: "/roadmap" },
];

export default async function CommunityPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getCommunity(locale);
  const p = (path: string) => `/${locale}${path}`;
  const facts = await getFacts();
  const sourceIsPublished = facts.latestPublishedRelease?.version === facts.version;
  const textLocale = pickTextLocale(locale);
  const contributionPaths = { en: pathsEn, zh: pathsZh }[textLocale];
  const activityLinks = { en: activityEn, zh: activityZh }[textLocale];


  const activityIcons: IconName[] = ["repeat", "book", "tide"];

  return (
    <>
      <PageHeader
        kicker={t.kicker}
        title={t.title}
        lede={t.lede}
        pose="talk"
        actions={
          <>
            <Link href="https://github.com/codewhale-hq/CodeWhale/issues/new/choose" className="btn btn-primary btn-lg">
              {t.fileIssue}
            </Link>
            <Link href="https://github.com/codewhale-hq/CodeWhale/pulls" className="btn btn-secondary btn-lg">
              {t.browsePulls}
            </Link>
            <Link href={p("/contribute")} className="btn btn-ghost btn-lg">
              {t.readGuide}
              <Icon name="arrow-right" className="icon icon-flip" />
            </Link>
          </>
        }
      />

      <div className="page-body">
        <Section
          id="community-paths"
          title={t.pathsTitle}
          scope={t.pathsScope}
        >
          <div className="grid-2">
            {contributionPaths.map((path) => (
              <Link key={path.title} href={path.href} className="tile">
                <h3>{path.title}</h3>
                <p>{path.description}</p>
                <span className="section-link">
                  {path.cta}
                  <Icon name={path.href.includes("LOCALIZATION") ? "external" : "arrow-right"} className="icon icon-flip" />
                </span>
              </Link>
            ))}
          </div>
        </Section>

        <Section
          id="community-record"
          title={t.recordTitle}
          scope={t.recordScope}
        >
          <ul className="dir-list dir-list-card" role="list">
            {activityLinks.map((item, index) => (
              <li key={item.path}>
                <Link href={p(item.path)} className="dir-row">
                  <span className="dir-mark" aria-hidden="true"><Icon name={activityIcons[index] ?? "book"} /></span>
                  <span className="dir-text">
                    <span className="dir-title">{item.title}</span>
                    <span className="dir-purpose">{item.description}</span>
                  </span>
                  <span className="dir-action" aria-hidden="true"><Icon name="chevron-right" className="icon icon-flip" /></span>
                </Link>
              </li>
            ))}
          </ul>
        </Section>

        <Section
          id="community-credit"
          title={t.creditTitle}
          scope={sourceIsPublished ? t.creditScope : t.creditScopeUnreleased}
        >
          <p className="page-kicker mb-4">
            {fill(sourceIsPublished ? t.creditLabel : t.creditLabelUnreleased, { version: facts.version ?? "—" })}
          </p>
          <div className="grid-2">
            <div className="tile">
              <h3>{t.mergedTitle}</h3>
              <ul className="credit-list" role="list">
                {RELEASE_CONTRIBUTORS.map((handle) => (
                  <li key={handle}>
                    <Link href={`https://github.com/${handle.slice(1)}`}>{handle}</Link>
                  </li>
                ))}
              </ul>
            </div>
            {RELEASE_HELPERS.length > 0 ? (
              <div className="tile">
                <h3>{t.helpersTitle}</h3>
                <ul className="credit-list" role="list">
                  {RELEASE_HELPERS.map((handle) => (
                    <li key={handle}>
                      <Link href={`https://github.com/${handle.slice(1)}`}>{handle}</Link>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
          </div>
          <p className="status-line mt-4">
            <Link href="https://github.com/codewhale-hq/CodeWhale/blob/main/docs/CONTRIBUTORS.md" className="link">
              {t.fullRecord}
            </Link>
            <Link href="https://github.com/codewhale-hq/CodeWhale/blob/main/CHANGELOG.md" className="link">CHANGELOG</Link>
          </p>
        </Section>
      </div>
    </>
  );
}
