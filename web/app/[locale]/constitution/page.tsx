import Link from "next/link";
import { Icon } from "@/components/icon";
import { PageHeader, Section } from "@/components/page-header";
import { Status } from "@/components/status-badge";
import { ThinkingTrace } from "@/components/thinking-trace";
import { buildPageMetadata } from "@/lib/page-meta";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const isZh = locale === "zh";
  return buildPageMetadata({
    path: "/constitution",
    locale,
    title: isZh ? "三层法 · Codewhale" : "Three layers of law · Codewhale",
    description: isZh
      ? "Codewhale 的嵌套宪章：内置基础法、你的常备法（/constitution）、仓库自己的法（.codewhale/constitution.json）。位阶由执行框架强制生效，换掉模型也不失效。"
      : "Codewhale's nested constitution: bundled base law, your standing law (/constitution), and your repo's law (.codewhale/constitution.json). Rank is enforced in the harness and survives a model swap.",
  });
}

/** The three layers, rendered as a card row on this page and (compact) on the homepage. */
const LAYERS = [
  {
    n: "01",
    name: { en: "Bundled Constitution", zh: "内置宪章" },
    path: "compiled into every binary",
    pathZh: "编译进每一个二进制",
    en: "The base law. Its priority article fixes the authority order for any conflict, so a stale handoff can never outrank a fresh test result by accident.",
    zh: "基础法。其中的位阶条款为一切冲突固定裁决顺序——过期的交接不会稀里糊涂地压过刚跑出的测试结果。",
  },
  {
    n: "02",
    name: { en: "/constitution — your standing law", zh: "/constitution——你的常备法" },
    path: "$CODEWHALE_HOME/constitution.json",
    pathZh: "$CODEWHALE_HOME/constitution.json",
    en: "Structured data, not a raw prompt editor: guided setup renders it into a model-facing prose block. Drafted with your model's help, ratified by you, and carried across every project.",
    zh: "结构化数据，不是裸的提示词编辑器：引导式设置把它渲染成面向模型的 prose 区块。可由模型协助起草，经你批准生效，跨项目随身携带。",
  },
  {
    n: "03",
    name: { en: "Your repo's law", zh: "仓库自己的法" },
    path: ".codewhale/constitution.json",
    pathZh: ".codewhale/constitution.json",
    en: "Protected invariants, branch policy, verification requirements, escalation conditions — loaded as a repo-local authority block above project instructions, memory, and handoffs.",
    zh: "受保护的不变量、分支策略、验证要求、升级条件——作为仓库本地的权威区块加载，位阶高于项目说明、记忆与交接。",
  },
];

export default async function ConstitutionPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const isZh = locale === "zh";
  const p = (path: string) => (isZh ? `/zh${path}` : `/en${path}`);

  return (
    <>
      <PageHeader
        seal="法"
        kicker={isZh ? "立论" : "The thesis"}
        title={isZh ? "三层法" : "Three layers of law"}
        titleAside={isZh ? "Three layers of law" : "三层法"}
        titleAsideLang={isZh ? "en" : "zh"}
        lede={
          isZh
            ? "项目一变老，指令就开始堆积、彼此冲突：最初的规格、后来推翻它的重构、陈旧的记忆、上一个智能体的交接、你此刻的要求、刚跑出的与交接说法不符的测试结果。扁平的系统提示词让模型靠猜来化解；Codewhale 用一部嵌套的宪章给出明确的位阶。顺序由执行框架强制生效——有测试断言它不会漂移——换掉模型，结构依然完好。"
            : "As a project ages, instructions pile up and conflict: the original spec, a refactor that contradicts it, stale memory, a previous agent's handoff, your current request, fresh test output that doesn't match what the handoff claimed. A flat system prompt makes the model resolve that by guess. Codewhale uses a nested constitution with a defined rank. The harness enforces the order, tests assert it can't drift, and it stays intact when you swap models."
        }
        pose="read"
      >
        <div className="callout">
          <p className="callout-title">
            <Status tone="accent">{isZh ? "自 v0.9.0 起" : "Since v0.9.0"}</Status>
          </p>
          <p>
            {isZh
              ? "宪章优先的初始设置——首次启动依次引导语言、模型、安全姿态和你的宪章；之后随时 /setup。模型可以起草，由你批准。"
              : "Constitution-first setup — first launch walks language, model, posture, and your constitution; /setup any time. The model can draft it. You ratify it."}
          </p>
        </div>
      </PageHeader>

      <div className="page-body">
        <Section
          id="constitution-rank"
          seal="序"
          title={isZh ? "位阶，从最稳到最活" : "The rank, most-static first"}
          scope={
            isZh
              ? "三层之下依次是项目说明（AGENTS.md）、记忆与交接。你此刻的要求和实时工具证据仍然主宰当前回合——模型可以被给到很多层，但它被要求不去报告工具没有返回的事实。"
              : "Below the three layers rank project instructions (AGENTS.md), then memory and handoffs. Your current request and live tool evidence still control the active turn — the model may be given many layers, but it is instructed never to report a fact the tools did not return."
          }
        >
          <ol className="steps">
            {LAYERS.map((layer, index) => (
              <li key={layer.n}>
                <span className="gs-step-index" aria-hidden="true">{index + 1}</span>
                <h3>{isZh ? layer.name.zh : layer.name.en}</h3>
                <p className="dir-meta">{isZh ? layer.pathZh : layer.path}</p>
                <p>{isZh ? layer.zh : layer.en}</p>
              </li>
            ))}
          </ol>
          <div className="callout mt-5">
            <p className="callout-title">
              <Icon name="shield" className="icon" />
              {isZh ? "诚实的边界" : "The honest boundary"}
            </p>
            <p>
              {isZh
                ? "审批、沙箱、网络与信任控制由代码强制执行——宪章文本永远越不过它们。"
                : "Approval, sandbox, network, and trust controls are enforced in code — constitution text never overrides them."}
            </p>
          </div>
        </Section>

        <Section
          id="constitution-trace"
          seal="证"
          title={isZh ? "在推理里可以被看到" : "Observable in the reasoning"}
          scope={
            isZh
              ? "位阶会体现在模型的推理里：它在裁决时直接援引条款。下面是示意，概括了这类推理的样子，并非某一次会话的逐字记录。"
              : "The rank shows up in the model's reasoning: it cites the articles as it decides. These panes are illustrations of that kind of reasoning, paraphrased for this page, not a transcript of one session."
          }
        >
          <ThinkingTrace locale={locale} />
        </Section>

        <section className="page-section">
          <div className="actions">
            <Link href={p("/install")} className="btn btn-primary btn-lg">
              {isZh ? "安装" : "Install"}
            </Link>
            <Link
              href="https://github.com/Hmbown/CodeWhale/blob/main/docs/CONFIGURATION.md#constitution-project-instructions-and-repo-authority"
              className="btn btn-ghost btn-lg"
            >
              {isZh ? "配置文档" : "Configuration"}
              <Icon name="external" className="icon" />
            </Link>
          </div>
        </section>
      </div>
    </>
  );
}
