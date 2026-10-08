import { Icon, type IconName } from "@/components/icon";
import { PageHeader, Section } from "@/components/page-header";
import { Status } from "@/components/status-badge";
import { getFacts } from "@/lib/facts";
import { getRuntime, pickText } from "@/lib/i18n/dictionaries";
import { buildPageMetadata } from "@/lib/page-meta";

const REPO_BLOB_BASE = "https://github.com/codewhale-hq/CodeWhale/blob/main";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getRuntime(locale);
  return buildPageMetadata({
    path: "/runtime",
    locale,
    title: t.metaTitle,
    description: t.metaDescription,
  });
}

interface Integration {
  name: string;
  icon: IconName;
  experimental?: boolean;
  desc: { en: string; zh: string };
  href: string;
}

const INTEGRATIONS: Integration[] = [
  {
    name: "HTTP / SSE Runtime API",
    icon: "terminal",
    desc: {
      en: "Full local HTTP + Server-Sent Events runtime API on 127.0.0.1:7878. Create threads, stream turns, manage background jobs, and control approval decisions — all from any HTTP client or the bundled mobile page.",
      zh: "完整的本地 HTTP + Server-Sent Events Runtime API，监听 127.0.0.1:7878。创建线程、流式对话、管理后台任务、控制审批决策——任意 HTTP 客户端或内置手机页面皆可调用。",
    },
    href: `${REPO_BLOB_BASE}/docs/RUNTIME_API.md`,
  },
  {
    name: "ACP (Agent Client Protocol)",
    icon: "plug",
    desc: {
      en: "Baseline JSON-RPC adapter over stdio for compatible editor clients such as Zed. It supports initialize, new session, prompt, and cancel with text responses; shell and file tools, checkpoint replay, and session loading remain on the full Runtime API.",
      zh: "面向 Zed 等兼容编辑器客户端的基础 JSON-RPC stdio 适配器。它支持初始化、新建会话、提示和取消，并返回文本响应；shell 与文件工具、检查点回放和会话加载仍由完整 Runtime API 提供。",
    },
    href: `${REPO_BLOB_BASE}/docs/RUNTIME_API.md`,
  },
  {
    name: "MCP (Model Context Protocol)",
    icon: "plug",
    desc: {
      en: "Connect Codewhale to external tools and services through configured MCP servers over stdio or HTTP/SSE, or expose Codewhale's own tools to another MCP client.",
      zh: "通过已配置的 MCP 服务器（stdio 或 HTTP/SSE）将 Codewhale 连接到外部工具和服务，或把 Codewhale 自身工具暴露给其他 MCP 客户端。",
    },
    href: `${REPO_BLOB_BASE}/docs/MCP.md`,
  },
  {
    name: "VS Code Extension",
    icon: "monitor",
    desc: {
      en: "Early companion for the local runtime. It can open Codewhale in a terminal, start and check the Runtime API, and show read-only thread summaries and restore points. It does not yet provide full chat, inline edits, or editor actions.",
      zh: "本地 Runtime 的早期配套扩展。它可以在终端中打开 Codewhale、启动并检查 Runtime API，以及显示只读线程摘要和还原点；目前尚不提供完整聊天、内联编辑或编辑器操作。",
    },
    href: "https://github.com/codewhale-hq/CodeWhale/tree/main/extensions/vscode",
  },
  {
    name: "CodeWhale GUI (VS Code)",
    icon: "monitor",
    desc: {
      en: "The community-maintained graphical frontend, in a separate repository: chat, threads, live file changes, and task tracking in a VS Code sidebar over this Runtime API. Install it from the VS Code Marketplace; the source is on GitHub.",
      zh: "社区维护的图形前端，位于独立仓库：在 VS Code 侧边栏中基于此 Runtime API 进行对话、管理线程、查看实时文件变更与任务进度。可从 VS Code Marketplace 安装；源码见 GitHub。",
    },
    href: "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode",
  },
  {
    name: "Telegram Bridge",
    icon: "message",
    desc: {
      en: "First-party Telegram bot bridge. Start a headless Codewhale session, then chat with it from any Telegram client — approvals, tool results, and completions surface inline.",
      zh: "官方 Telegram 机器人桥接。启动无头 Codewhale 会话，在任何 Telegram 客户端中与之对话——审批、工具结果和完成状态内联展示。",
    },
    href: "https://github.com/codewhale-hq/CodeWhale/tree/main/integrations/telegram-bridge",
  },
  {
    name: "Feishu / Lark Bridge",
    icon: "message",
    desc: {
      en: "First-party Feishu / Lark bot bridge. Chat-native agent loop inside your Feishu workspace with approval cards, session linking, and audit trail.",
      zh: "官方飞书 / Lark 机器人桥接。在飞书工作区内实现聊天原生 Agent 循环，支持审批卡片、会话关联和审计日志。",
    },
    href: "https://github.com/codewhale-hq/CodeWhale/tree/main/integrations/feishu-bridge",
  },
  {
    name: "Weixin Bridge",
    icon: "message",
    experimental: true,
    desc: {
      en: "Experimental Weixin / WeChat bridge. Receive agent completions and approvals inside WeChat; early-stage and not recommended for production deployments.",
      zh: "实验性微信桥接。在微信中接收 Agent 完成通知和审批；早期阶段，不建议用于生产环境。",
    },
    href: "https://github.com/codewhale-hq/CodeWhale/tree/main/integrations/weixin-bridge",
  },
];

const TRUST = [
  {
    title: { en: "Runs on your machine", zh: "本机运行" },
    body: {
      en: "The Runtime API binds 127.0.0.1 by default. The local runtime does not require a Codewhale account or hosted relay.",
      zh: "Runtime API 默认仅监听 127.0.0.1。本地运行时不需要 Codewhale 账户或托管中继。",
    },
  },
  {
    title: { en: "Auth required", zh: "认证必需" },
    body: {
      en: "Runtime API routes (/v1/*) require a Bearer token. Pass --auth-token at startup or set CODEWHALE_RUNTIME_TOKEN. Only a loopback bind can turn this off, with --insecure-no-auth.",
      zh: "Runtime API 路由（/v1/*）需要 Bearer Token。启动时传入 --auth-token，或设置 CODEWHALE_RUNTIME_TOKEN 环境变量。仅在回环地址上可用 --insecure-no-auth 关闭认证。",
    },
  },
  {
    title: { en: "Permissions user-controlled", zh: "权限用户控制" },
    body: {
      en: "Remote clients submit requests and approval decisions through the authenticated Runtime API. Local mode, permission posture, and sandbox policy still apply.",
      zh: "远程客户端通过经过认证的 Runtime API 提交请求与审批决定。本地模式、权限姿态和沙箱策略仍然生效。",
    },
  },
  {
    title: { en: "Open protocols", zh: "开放协议" },
    body: {
      en: "The HTTP/SSE Runtime API, MCP surface, and baseline ACP stdio adapter serve different integration needs; choose the interface your compatible client supports.",
      zh: "HTTP/SSE Runtime API、MCP 和基础 ACP stdio 适配器分别服务于不同集成场景；请根据客户端需要选择对应接口。",
    },
  },
];

export default async function RuntimePage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = getRuntime(locale);
  const facts = await getFacts();

  return (
    <>
      <PageHeader
        seal="接"
        kicker={t.kicker}
        title={t.title}
        titleAside={t.titleAside}
        titleAsideLang={t.titleAsideLang}
        lede={t.lede}
        pose="run"
      />

      <div className="page-body">
        <Section id="runtime-integrations" seal="集" title={t.integrationsTitle}>
          <ul className="dir-list dir-list-card" role="list">
            {INTEGRATIONS.map((item) => (
              <li key={item.name}>
                <a href={item.href} target="_blank" rel="noopener noreferrer" className="dir-row">
                  <span className="dir-mark" aria-hidden="true"><Icon name={item.icon} /></span>
                  <span className="dir-text">
                    <span className="dir-title">{item.name}</span>
                    <span className="dir-purpose">{pickText(item.desc, locale)}</span>
                  </span>
                  {item.experimental ? <Status tone="attention">{t.experimental}</Status> : null}
                  <span className="dir-action" aria-hidden="true"><Icon name="external" /></span>
                </a>
              </li>
            ))}
          </ul>
        </Section>

        <Section id="runtime-trust" layout="split" seal="信" title={t.trustTitle}>
          <dl className="ruled-list ruled-list-stacked">
            {TRUST.map((item) => (
              <div key={item.title.en}>
                <dt>{pickText(item.title, locale)}</dt>
                <dd>{pickText(item.body, locale)}</dd>
              </div>
            ))}
          </dl>
        </Section>

        <Section id="runtime-facts" layout="split" seal="数" title={t.factsTitle}>
          <dl className="ruled-list">
            <div>
              <dt>{t.version}</dt>
              <dd className="tabular">{facts.version ?? "—"}</dd>
            </div>
            <div>
              <dt>{t.toolCount}</dt>
              <dd className="tabular">{facts.toolCount ?? "—"}</dd>
            </div>
            <div>
              <dt>{t.sandboxBackends}</dt>
              <dd>{facts.sandboxBackends.length ? facts.sandboxBackends.join(" · ") : "—"}</dd>
            </div>
          </dl>
          {/* Crate names and the source revision are for maintainers; they
              sit behind Details. */}
          <details className="group-card disclosure-row mt-4">
            <summary>
              {t.details}
              <Icon name="chevron-down" className="disclosure-chevron" />
            </summary>
            <dl className="disclosure-body runtime-details">
              <div>
                <dt>Crates</dt>
                <dd>{facts.crates.length ? `${facts.crates.length} · ${facts.crates.join(", ")}` : "—"}</dd>
              </div>
              <div>
                <dt>{t.sourceRevision}</dt>
                <dd><code className="inline">{facts.sourceRevision ?? "—"}</code></dd>
              </div>
            </dl>
          </details>
        </Section>

        <section className="page-section">
          <p className="status-line">
            <span>{t.docsLead}</span>
            <a href={`${REPO_BLOB_BASE}/docs/RUNTIME_API.md`} target="_blank" rel="noopener noreferrer" className="link">
              {t.runtimeApiDoc}
            </a>
            <a href={`${REPO_BLOB_BASE}/docs/MCP.md`} target="_blank" rel="noopener noreferrer" className="link">
              {t.mcpDoc}
            </a>
          </p>
        </section>
      </div>
    </>
  );
}
