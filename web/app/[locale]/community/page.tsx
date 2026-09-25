import Link from "next/link";
import { Icon, type IconName } from "@/components/icon";
import { PageHeader, Section } from "@/components/page-header";
import { getFacts } from "@/lib/facts";
import { buildPageMetadata } from "@/lib/page-meta";
import { RELEASE_CONTRIBUTORS, RELEASE_HELPERS } from "@/lib/release-credits";

export const revalidate = 300;

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const isZh = locale === "zh";
  return buildPageMetadata({
    path: "/community",
    locale,
    title: isZh ? "社区 · Codewhale" : "Community · Codewhale",
    description: isZh
      ? "了解 Codewhale 的国际开源社区，提交 issue、发送 pull request、改进翻译并查看版本贡献者。"
      : "File issues, send pull requests, improve translations, and see who contributed to each Codewhale release.",
  });
}

export default async function CommunityPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const isZh = locale === "zh";
  const p = (path: string) => `/${locale}${path}`;
  const facts = await getFacts();
  const sourceIsPublished = facts.latestPublishedRelease?.version === facts.version;

  const contributionPaths = isZh
    ? [
        {
          title: "报告问题",
          description: "报告 bug、兼容性问题或不清楚的行为，并附上系统信息、复现步骤和可以安全分享的日志。",
          cta: "提交 issue",
          href: "https://github.com/Hmbown/CodeWhale/issues/new/choose",
        },
        {
          title: "改进代码或测试",
          description: "挑一个范围清楚的问题，写最小的补丁，加一个覆盖改动的回归测试。",
          cta: "查看开放 issues",
          href: "https://github.com/Hmbown/CodeWhale/issues",
        },
        {
          title: "改进文档或翻译",
          description: "改正说错的地方，补一个示例，或者帮忙完成一个语言包。",
          cta: "查看本地化指南",
          href: "https://github.com/Hmbown/CodeWhale/blob/main/docs/LOCALIZATION.md",
        },
        {
          title: "复现并审查现有工作",
          description: "在你的平台和提供商上验证 issue 或 pull request，然后分享你运行的命令、结果和剩余问题。",
          cta: "查看 pull requests",
          href: "https://github.com/Hmbown/CodeWhale/pulls",
        },
      ]
    : [
        {
          title: "Report a problem",
          description: "File a bug, compatibility problem, or unclear behavior with system details, reproduction steps, and any logs you can share safely.",
          cta: "File an issue",
          href: "https://github.com/Hmbown/CodeWhale/issues/new/choose",
        },
        {
          title: "Improve code or tests",
          description: "Pick one problem with clear edges, write the smallest patch that fixes it, and add a regression test that covers it.",
          cta: "Browse open issues",
          href: "https://github.com/Hmbown/CodeWhale/issues",
        },
        {
          title: "Improve documentation or translations",
          description: "Fix a wrong sentence, add an example, or help finish a language pack.",
          cta: "Open the localization guide",
          href: "https://github.com/Hmbown/CodeWhale/blob/main/docs/LOCALIZATION.md",
        },
        {
          title: "Reproduce and review existing work",
          description: "Try an issue or pull request on your platform and provider. Post the commands you ran, what happened, and what is still wrong.",
          cta: "Browse pull requests",
          href: "https://github.com/Hmbown/CodeWhale/pulls",
        },
      ];

  const activityLinks = isZh
    ? [
        { title: "仓库动态", description: "最近的 issues 与 pull requests。", href: p("/feed") },
        { title: "社区摘要", description: "经过维护者审核的每周项目记录。", href: p("/digest") },
        { title: "公开路线图", description: "已发布、正在进行、考虑中和明确不做的工作。", href: p("/roadmap") },
      ]
    : [
        { title: "Repository activity", description: "Recent issues and pull requests.", href: p("/feed") },
        { title: "Community digest", description: "The weekly project record, reviewed by a maintainer.", href: p("/digest") },
        { title: "Public roadmap", description: "Shipped, underway, considered, and ruled-out work.", href: p("/roadmap") },
      ];

  const activityIcons: IconName[] = ["repeat", "book", "tide"];

  return (
    <>
      <PageHeader
        kicker={isZh ? "国际开源社区" : "International open-source community"}
        title={isZh ? "与世界各地的贡献者一起构建 Codewhale。" : "Build Codewhale with contributors around the world."}
        lede={
          isZh
            ? "运行时、文档、测试和翻译，来自不同国家、语言和平台的贡献者。第一次参与不必是大功能。一份清楚的 bug 报告、一处文档修正、或一个带测试的小补丁，都算。"
            : "The runtime, docs, tests, and translations come from contributors across countries, languages, and platforms. A first contribution does not have to be a feature. A clear bug report, a documentation fix, or a small tested patch counts."
        }
        pose="talk"
        actions={
          <>
            <Link href="https://github.com/Hmbown/CodeWhale/issues/new/choose" className="btn btn-primary btn-lg">
              {isZh ? "提交 issue" : "File an issue"}
            </Link>
            <Link href="https://github.com/Hmbown/CodeWhale/pulls" className="btn btn-secondary btn-lg">
              {isZh ? "查看 pull requests" : "Browse pull requests"}
            </Link>
            <Link href={p("/contribute")} className="btn btn-ghost btn-lg">
              {isZh ? "阅读贡献指南" : "Read the contribution guide"}
              <Icon name="arrow-right" className="icon icon-flip" />
            </Link>
          </>
        }
      />

      <div className="page-body">
        <Section
          id="community-paths"
          title={isZh ? "从一件小事开始。" : "Start with one small thing."}
          scope={
            isZh
              ? "报 bug、写代码、写测试、改文档、翻译、审查，都有用。选一样适合你时间的。"
              : "Bug reports, code, tests, docs, translations, and review all help. Pick the one that fits the time you have."
          }
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
          title={isZh ? "从提案到发布，都是公开的。" : "From proposal to release, in the open."}
          scope={
            isZh
              ? "动态页显示最近的仓库活动。社区摘要保留每周存档。路线图区分已发布的和还在讨论的。"
              : "The activity feed shows recent repository work. The community digest keeps the weekly archive of repository activity. The roadmap separates what shipped from what is still being discussed."
          }
        >
          <ul className="dir-list dir-list-card" role="list">
            {activityLinks.map((item, index) => (
              <li key={item.href}>
                <Link href={item.href} className="dir-row">
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
          title={isZh ? "贡献者署名是版本记录的一部分。" : "Contributor credit is part of the release record."}
          scope={
            isZh
              ? `${sourceIsPublished ? "这一版本" : "这一版"}包含社区提交的代码、测试、复现和验证。即使维护者改过补丁再合入，原作者的署名也保留在提交、更新日志和贡献者名单中。`
              : `This ${sourceIsPublished ? "release" : "version"} includes code, tests, reproductions, and verification from the community. If a maintainer reworks a patch before it lands, the original author stays credited in the commit, the changelog, and the contributor record.`
          }
        >
          <p className="page-kicker mb-4">
            {sourceIsPublished
              ? isZh
                ? `v${facts.version} 版本致谢`
                : `v${facts.version} release credit`
              : isZh
                ? `v${facts.version} 致谢（未发布）`
                : `v${facts.version} credit (unreleased)`}
          </p>
          <div className="grid-2">
            <div className="tile">
              <h3>{isZh ? "已合并或吸收的贡献" : "Merged or adapted contributions"}</h3>
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
                <h3>{isZh ? "报告、复现与验证" : "Reports, reproductions, and verification"}</h3>
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
            <Link href="https://github.com/Hmbown/CodeWhale/blob/main/docs/CONTRIBUTORS.md" className="link">
              {isZh ? "完整贡献者名单" : "Full contributor record"}
            </Link>
            <Link href="https://github.com/Hmbown/CodeWhale/blob/main/CHANGELOG.md" className="link">CHANGELOG</Link>
          </p>
        </Section>
      </div>
    </>
  );
}
