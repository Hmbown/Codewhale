import Link from "next/link";
import { Icon } from "@/components/icon";
import { PageHeader, Section } from "@/components/page-header";
import { Status } from "@/components/status-badge";
import { ThinkingTrace } from "@/components/thinking-trace";
import { getConstitution, pickText } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getConstitution(locale);
  return buildPageMetadata({
    path: "/constitution",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

/** The three layers, rendered as a card row on this page and (compact) on the homepage. */
const LAYERS = [
  {
    n: "01",
    name: { en: "Bundled Constitution", zh: "内置宪章" },
    path: { en: "compiled into every binary", zh: "编译进每一个二进制" },
    body: {
      en: "The base law. Its priority article fixes the authority order for any conflict, so a stale handoff can never outrank a fresh test result by accident.",
      zh: "基础法。其中的位阶条款为一切冲突固定裁决顺序——过期的交接不会稀里糊涂地压过刚跑出的测试结果。",
    },
  },
  {
    n: "02",
    name: { en: "/constitution — your standing law", zh: "/constitution——你的常备法" },
    path: { en: "$CODEWHALE_HOME/constitution.json", zh: "$CODEWHALE_HOME/constitution.json" },
    body: {
      en: "Structured data, not a raw prompt editor: guided setup renders it into a model-facing prose block. Drafted with your model's help, ratified by you, and carried across every project.",
      zh: "结构化数据，不是裸的提示词编辑器：引导式设置把它渲染成面向模型的 prose 区块。可由模型协助起草，经你批准生效，跨项目随身携带。",
    },
  },
  {
    n: "03",
    name: { en: "Your repo's law", zh: "仓库自己的法" },
    path: { en: ".codewhale/constitution.json", zh: ".codewhale/constitution.json" },
    body: {
      en: "Protected invariants, branch policy, verification requirements, escalation conditions — loaded as a repo-local authority block above project instructions, memory, and handoffs.",
      zh: "受保护的不变量、分支策略、验证要求、升级条件——作为仓库本地的权威区块加载，位阶高于项目说明、记忆与交接。",
    },
  },
];

export default async function ConstitutionPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getConstitution(locale);

  return (
    <>
      <PageHeader
        seal="法"
        kicker={t.kicker}
        title={t.title}
        titleAside={t.titleAside}
        titleAsideLang={t.titleAsideLang}
        lede={t.lede}
        pose="read"
      >
        <div className="callout">
          <p className="callout-title">
            <Status tone="accent">{t.since}</Status>
          </p>
          <p>{t.sinceBody}</p>
        </div>
      </PageHeader>

      <div className="page-body">
        <Section
          id="constitution-rank"
          seal="序"
          title={t.rankTitle}
          scope={t.rankScope}
        >
          <ol className="steps">
            {LAYERS.map((layer, index) => (
              <li key={layer.n}>
                <span className="gs-step-index" aria-hidden="true">{index + 1}</span>
                <h3>{pickText(layer.name, locale)}</h3>
                <p className="dir-meta">{pickText(layer.path, locale)}</p>
                <p>{pickText(layer.body, locale)}</p>
              </li>
            ))}
          </ol>
          <div className="callout mt-5">
            <p className="callout-title">
              <Icon name="shield" className="icon" />
              {t.boundaryTitle}
            </p>
            <p>{t.boundaryBody}</p>
          </div>
        </Section>

        <Section
          id="constitution-trace"
          seal="证"
          title={t.traceTitle}
          scope={t.traceScope}
        >
          <ThinkingTrace locale={locale} />
        </Section>

        <section className="page-section">
          <div className="actions">
            <Link href={`/${locale}/install`} className="btn btn-primary btn-lg">
              {t.install}
            </Link>
            <Link
              href="https://github.com/codewhale-hq/CodeWhale/blob/main/docs/CONFIGURATION.md#constitution-project-instructions-and-repo-authority"
              className="btn btn-ghost btn-lg"
            >
              {t.configuration}
              <Icon name="external" className="icon" />
            </Link>
          </div>
        </section>
      </div>
    </>
  );
}
