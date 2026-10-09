import Link from "next/link";
import { Icon } from "@/components/icon";
import { PageHeader, Section } from "@/components/page-header";
import { getContribute, pickTextLocale } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";
import { getMerchCopy } from "@/lib/content/contributor-merch";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getContribute(locale);
  return buildPageMetadata({
    path: "/contribute",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

const stepsEn = [
  {
    n: "01",
    title: "Choose one clear problem",
    body: "Browse open issues, especially good first issue and help wanted. If the behavior is not tracked yet, open an issue with a reproduction before starting a large change.",
    cta: { label: "Browse open issues", href: "https://github.com/codewhale-hq/CodeWhale/issues" },
  },
  {
    n: "02",
    title: "Fork and create a branch",
    body: "Clone your fork and use a short branch name such as fix/provider-timeout or docs/fleet-example. Keep unrelated changes in separate pull requests.",
    cta: { label: "Open the repository", href: "https://github.com/codewhale-hq/CodeWhale" },
  },
  {
    n: "03",
    title: "Test the behavior you changed",
    body: "Run the smallest relevant test first, then formatting and the broader checks required for the part of the repository you touched.",
    cta: { label: "Read the contributor guide", href: "https://github.com/codewhale-hq/CodeWhale/blob/main/CONTRIBUTING.md" },
  },
  {
    n: "04",
    title: "Explain the result in the PR",
    body: "Describe the problem, the reason for the change, the checks you ran, and any remaining risk. Link the issue when one exists.",
    cta: { label: "View open pull requests", href: "https://github.com/codewhale-hq/CodeWhale/pulls" },
  },
];

const stepsZh = [
  {
    n: "01",
    title: "选择一个明确的问题",
    body: "先浏览 open issues，尤其是 good first issue 和 help wanted。如果问题尚未记录，请在开始大改动前提交带复现步骤的 issue。",
    cta: { label: "浏览 open issues", href: "https://github.com/codewhale-hq/CodeWhale/issues" },
  },
  {
    n: "02",
    title: "Fork 并创建分支",
    body: "克隆你的 fork，并使用简短的分支名，例如 fix/provider-timeout 或 docs/fleet-example。无关修改请拆成不同的 pull request。",
    cta: { label: "打开仓库", href: "https://github.com/codewhale-hq/CodeWhale" },
  },
  {
    n: "03",
    title: "测试你修改的行为",
    body: "先运行最小相关测试，再执行格式检查和你所修改部分需要的更完整检查。",
    cta: { label: "阅读贡献指南", href: "https://github.com/codewhale-hq/CodeWhale/blob/main/CONTRIBUTING.md" },
  },
  {
    n: "04",
    title: "在 PR 中说明结果",
    body: "说明问题、修改原因、已运行的检查和剩余风险；如果已有 issue，请在 PR 中关联。",
    cta: { label: "查看 open pull requests", href: "https://github.com/codewhale-hq/CodeWhale/pulls" },
  },
];

const pathsEn = [
  {
    title: "Report a bug or compatibility problem",
    body: "Include your system, Codewhale version, reproduction steps, expected behavior, and any logs you can share safely.",
    label: "File an issue",
    href: "https://github.com/codewhale-hq/CodeWhale/issues/new/choose",
  },
  {
    title: "Improve code or tests",
    body: "Choose a well-bounded problem, make the smallest useful patch, and add a regression test that proves the changed behavior.",
    label: "Browse open issues",
    href: "https://github.com/codewhale-hq/CodeWhale/issues",
  },
  {
    title: "Improve documentation or translations",
    body: "Correct inaccurate guidance, add a practical example, or help a complete language pack stay natural and aligned with the English keys.",
    label: "Read the localization guide",
    href: "https://github.com/codewhale-hq/CodeWhale/blob/main/docs/LOCALIZATION.md",
  },
  {
    title: "Reproduce and review existing work",
    body: "Verify an issue or pull request with your platform and provider, then share the exact checks and results.",
    label: "Browse pull requests",
    href: "https://github.com/codewhale-hq/CodeWhale/pulls",
  },
];

const pathsZh = [
  {
    title: "报告 bug 或兼容性问题",
    body: "提供系统信息、Codewhale 版本、复现步骤、期望行为和可公开的日志。",
    label: "提交 issue",
    href: "https://github.com/codewhale-hq/CodeWhale/issues/new/choose",
  },
  {
    title: "改进代码或测试",
    body: "选择一个边界清楚的问题，提交最小补丁，并用回归测试证明修改后的行为。",
    label: "查看待处理 issues",
    href: "https://github.com/codewhale-hq/CodeWhale/issues",
  },
  {
    title: "改进文档或翻译",
    body: "修正不准确的说明、补充实际示例，或帮助完整语言包保持自然且与英文键一致。",
    label: "阅读本地化指南",
    href: "https://github.com/codewhale-hq/CodeWhale/blob/main/docs/LOCALIZATION.md",
  },
  {
    title: "复现和审查现有工作",
    body: "验证 issue 或 pull request 在你的平台和提供商上的行为，并分享准确的测试结果。",
    label: "浏览 pull requests",
    href: "https://github.com/codewhale-hq/CodeWhale/pulls",
  },
];

const reviewNotesEn = [
  "Keep one problem per pull request so the change is easy to test, review, and credit.",
  "Add tests for new behavior. For documentation fixes, verify commands, configuration names, and the current version against the repository.",
  "Open an issue before changing authentication, credentials, sandbox policy, release plumbing, branding, or global prompts.",
  "If a maintainer needs to adapt or harvest a patch, the original contributor remains credited in the commit, changelog, and contributor list.",
];

const reviewNotesZh = [
  "一个 PR 只解决一个问题，便于测试、审查和保留贡献者署名。",
  "新增行为应有测试；修正文档时，请核对实际命令、配置名和当前版本。",
  "修改认证、凭据、沙箱、发布流程、品牌或全局提示词前，请先提交 issue 并确认范围。",
  "如果维护者需要整理或摘取补丁，原作者仍会在提交、CHANGELOG 和贡献者名单中获得署名。",
];

export default async function ContributePage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getContribute(locale);
  const textLocale = pickTextLocale(locale);
  const merch = getMerchCopy(locale);
  const steps = { en: stepsEn, zh: stepsZh }[textLocale];
  const paths = { en: pathsEn, zh: pathsZh }[textLocale];
  const reviewNotes = { en: reviewNotesEn, zh: reviewNotesZh }[textLocale];

  return (
    <>
      <PageHeader
        kicker={t.kicker}
        title={t.title}
        lede={t.lede}
        pose="write"
        actions={
          <>
            <Link href="https://github.com/codewhale-hq/CodeWhale/issues/new/choose" className="btn btn-primary btn-lg">
              {t.fileIssue}
            </Link>
            <Link href="https://github.com/codewhale-hq/CodeWhale/pulls" className="btn btn-secondary btn-lg">
              {t.browsePulls}
            </Link>
            <Link href="https://github.com/codewhale-hq/CodeWhale/blob/main/CONTRIBUTING.md" className="btn btn-ghost btn-lg">
              {t.fullGuide}
              <Icon name="external" className="icon" />
            </Link>
          </>
        }
      />

      <div className="page-body">
        <Section id="contribute-paths" title={t.pathsTitle}>
          <div className="grid-2">
            {paths.map((path) => (
              <Link key={path.title} href={path.href} className="tile">
                <h3>{path.title}</h3>
                <p>{path.body}</p>
                <span className="section-link">
                  {path.label}
                  <Icon name="arrow-right" className="icon icon-flip" />
                </span>
              </Link>
            ))}
          </div>
        </Section>

        <Section id="contribute-workflow" title={t.workflowTitle}>
          <ol className="steps">
            {steps.map((step, index) => (
              <li key={step.n}>
                <span className="gs-step-index" aria-hidden="true">{index + 1}</span>
                <h3>{step.title}</h3>
                <p>{step.body}</p>
                <Link href={step.cta.href} className="section-link">
                  {step.cta.label}
                  <Icon name="arrow-right" className="icon icon-flip" />
                </Link>
              </li>
            ))}
          </ol>
        </Section>

        <Section
          id="contribute-review"
          title={t.reviewTitle}
          scope={t.reviewScope}
        >
          <ul className="group-card" role="list">
            {reviewNotes.map((note) => (
              <li key={note} className="group-row check-row">
                <Icon name="check" className="icon" />
                <span>{note}</span>
              </li>
            ))}
          </ul>
        </Section>

        <Section
          id="contribute-dev"
          title={t.devTitle}
          scope={t.devScope}
        >
          <pre tabIndex={0} className="code-block">
{`git clone https://github.com/YOUR_USERNAME/CodeWhale.git
cd CodeWhale
git checkout -b fix/your-change

cargo build
cargo test -p <owning-crate> --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked`}
          </pre>
        </Section>
        <Section id="contributor-merch" title={merch.title} scope={merch.lede}>
          <Link href={`/${locale}/merch`} className="btn btn-secondary">{merch.cta}</Link>
        </Section>
      </div>
    </>
  );
}
