import { Icon, type IconName } from "@/components/icon";
import { PageHeader, Section } from "@/components/page-header";
import { Status } from "@/components/status-badge";
import { getFacts } from "@/lib/facts";
import { buildPageMetadata } from "@/lib/page-meta";

const REPO_BLOB_BASE = "https://github.com/Hmbown/CodeWhale/blob/main";

export async function generateMetadata({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const isZh = locale === "zh";
  return buildPageMetadata({
    path: "/runtime",
    locale,
    title: isZh ? "Runtime & 集成 · Codewhale" : "Runtime & Integrations · Codewhale",
    description: isZh
      ? "Codewhale 的本地 Runtime API、HTTP/SSE、ACP stdio 适配器、MCP 服务器、VS Code 配套扩展与消息桥接。"
      : "Codewhale's local Runtime API, HTTP/SSE, baseline ACP stdio adapter, MCP servers, an early VS Code companion, and messaging bridges.",
  });
}

interface Integration {
  name: string;
  icon: IconName;
  experimental?: boolean;
  desc: string;
  descZh: string;
  href: string;
}

const INTEGRATIONS: Integration[] = [
  {
    name: "HTTP / SSE Runtime API",
    icon: "terminal",
    desc: "Full local HTTP + Server-Sent Events runtime API on 127.0.0.1:7878. Create threads, stream turns, manage background jobs, and control approval decisions — all from any HTTP client or the bundled mobile page.",
    descZh: "完整的本地 HTTP + Server-Sent Events Runtime API，监听 127.0.0.1:7878。创建线程、流式对话、管理后台任务、控制审批决策——任意 HTTP 客户端或内置手机页面皆可调用。",
    href: `${REPO_BLOB_BASE}/docs/RUNTIME_API.md`,
  },
  {
    name: "ACP (Agent Client Protocol)",
    icon: "plug",
    desc: "Baseline JSON-RPC adapter over stdio for compatible editor clients such as Zed. It supports initialize, new session, prompt, and cancel with text responses; shell and file tools, checkpoint replay, and session loading remain on the full Runtime API.",
    descZh: "面向 Zed 等兼容编辑器客户端的基础 JSON-RPC stdio 适配器。它支持初始化、新建会话、提示和取消，并返回文本响应；shell 与文件工具、检查点回放和会话加载仍由完整 Runtime API 提供。",
    href: `${REPO_BLOB_BASE}/docs/RUNTIME_API.md`,
  },
  {
    name: "MCP (Model Context Protocol)",
    icon: "plug",
    desc: "Connect Codewhale to external tools and services through configured MCP servers over stdio or HTTP/SSE, or expose Codewhale's own tools to another MCP client.",
    descZh: "通过已配置的 MCP 服务器（stdio 或 HTTP/SSE）将 Codewhale 连接到外部工具和服务，或把 Codewhale 自身工具暴露给其他 MCP 客户端。",
    href: `${REPO_BLOB_BASE}/docs/MCP.md`,
  },
  {
    name: "VS Code Extension",
    icon: "monitor",
    desc: "Early companion for the local runtime. It can open Codewhale in a terminal, start and check the Runtime API, and show read-only thread summaries and restore points. It does not yet provide full chat, inline edits, or editor actions.",
    descZh: "本地 Runtime 的早期配套扩展。它可以在终端中打开 Codewhale、启动并检查 Runtime API，以及显示只读线程摘要和还原点；目前尚不提供完整聊天、内联编辑或编辑器操作。",
    href: "https://github.com/Hmbown/CodeWhale/tree/main/extensions/vscode",
  },
  {
    name: "Telegram Bridge",
    icon: "message",
    desc: "First-party Telegram bot bridge. Start a headless Codewhale session, then chat with it from any Telegram client — approvals, tool results, and completions surface inline.",
    descZh: "官方 Telegram 机器人桥接。启动无头 Codewhale 会话，在任何 Telegram 客户端中与之对话——审批、工具结果和完成状态内联展示。",
    href: "https://github.com/Hmbown/CodeWhale/tree/main/integrations/telegram-bridge",
  },
  {
    name: "Feishu / Lark Bridge",
    icon: "message",
    desc: "First-party Feishu / Lark bot bridge. Chat-native agent loop inside your Feishu workspace with approval cards, session linking, and audit trail.",
    descZh: "官方飞书 / Lark 机器人桥接。在飞书工作区内实现聊天原生 Agent 循环，支持审批卡片、会话关联和审计日志。",
    href: "https://github.com/Hmbown/CodeWhale/tree/main/integrations/feishu-bridge",
  },
  {
    name: "Weixin Bridge",
    icon: "message",
    experimental: true,
    desc: "Experimental Weixin / WeChat bridge. Receive agent completions and approvals inside WeChat; early-stage and not recommended for production deployments.",
    descZh: "实验性微信桥接。在微信中接收 Agent 完成通知和审批；早期阶段，不建议用于生产环境。",
    href: "https://github.com/Hmbown/CodeWhale/tree/main/integrations/weixin-bridge",
  },
];

export default async function RuntimePage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const isZh = locale === "zh";
  const facts = await getFacts();

  const trust = [
    {
      title: isZh ? "本机运行" : "Runs on your machine",
      body: isZh
        ? "Runtime API 默认仅监听 127.0.0.1。本地运行时不需要 Codewhale 账户或托管中继。"
        : "The Runtime API binds 127.0.0.1 by default. The local runtime does not require a Codewhale account or hosted relay.",
    },
    {
      title: isZh ? "认证必需" : "Auth required",
      body: isZh
        ? "Runtime API 路由（/v1/*）需要 Bearer Token。启动时传入 --auth-token，或设置 CODEWHALE_RUNTIME_TOKEN 环境变量。仅在回环地址上可用 --insecure-no-auth 关闭认证。"
        : "Runtime API routes (/v1/*) require a Bearer token. Pass --auth-token at startup or set CODEWHALE_RUNTIME_TOKEN. Only a loopback bind can turn this off, with --insecure-no-auth.",
    },
    {
      title: isZh ? "权限用户控制" : "Permissions user-controlled",
      body: isZh
        ? "远程客户端通过经过认证的 Runtime API 提交请求与审批决定。本地模式、权限姿态和沙箱策略仍然生效。"
        : "Remote clients submit requests and approval decisions through the authenticated Runtime API. Local mode, permission posture, and sandbox policy still apply.",
    },
    {
      title: isZh ? "开放协议" : "Open protocols",
      body: isZh
        ? "HTTP/SSE Runtime API、MCP 和基础 ACP stdio 适配器分别服务于不同集成场景；请根据客户端需要选择对应接口。"
        : "The HTTP/SSE Runtime API, MCP surface, and baseline ACP stdio adapter serve different integration needs; choose the interface your compatible client supports.",
    },
  ];

  return (
    <>
      <PageHeader
        seal="接"
        kicker={isZh ? "Runtime & 集成" : "Runtime & Integrations"}
        title={isZh ? "运行时与集成" : "Runtime and integrations"}
        titleAside={isZh ? "Runtime & Integrations" : "运行时与集成"}
        titleAsideLang={isZh ? "en" : "zh"}
        lede={
          isZh
            ? "Codewhale 不仅是一个终端 Agent——它还是一个可通过多种协议和集成方式嵌入到你现有工作流中的本地控制平面。"
            : "Embed Codewhale in the tools you already use. Beyond the terminal, it runs a local control plane that editors, scripts and chat apps can talk to."
        }
        pose="run"
      />

      <div className="page-body">
        <Section id="runtime-integrations" seal="集" title={isZh ? "集成方式" : "Integration surfaces"}>
          <ul className="dir-list dir-list-card" role="list">
            {INTEGRATIONS.map((item) => (
              <li key={item.name}>
                <a href={item.href} target="_blank" rel="noopener noreferrer" className="dir-row">
                  <span className="dir-mark" aria-hidden="true"><Icon name={item.icon} /></span>
                  <span className="dir-text">
                    <span className="dir-title">{item.name}</span>
                    <span className="dir-purpose">{isZh ? item.descZh : item.desc}</span>
                  </span>
                  {item.experimental ? <Status tone="attention">{isZh ? "实验性" : "Experimental"}</Status> : null}
                  <span className="dir-action" aria-hidden="true"><Icon name="external" /></span>
                </a>
              </li>
            ))}
          </ul>
        </Section>

        <Section id="runtime-trust" layout="split" seal="信" title={isZh ? "信任边界" : "Trust boundary"}>
          <dl className="ruled-list ruled-list-stacked">
            {trust.map((item) => (
              <div key={item.title}>
                <dt>{item.title}</dt>
                <dd>{item.body}</dd>
              </div>
            ))}
          </dl>
        </Section>

        <Section id="runtime-facts" layout="split" seal="数" title={isZh ? "运行时事实" : "Runtime facts"}>
          <dl className="ruled-list">
            <div>
              <dt>{isZh ? "版本" : "Version"}</dt>
              <dd className="tabular">{facts.version ?? "—"}</dd>
            </div>
            <div>
              <dt>{isZh ? "工具数量" : "Tool count"}</dt>
              <dd className="tabular">{facts.toolCount ?? "—"}</dd>
            </div>
            <div>
              <dt>{isZh ? "沙箱后端" : "Sandbox backends"}</dt>
              <dd>{facts.sandboxBackends.length ? facts.sandboxBackends.join(" · ") : "—"}</dd>
            </div>
          </dl>
          {/* Crate names and the source revision are for maintainers; they
              sit behind Details. */}
          <details className="group-card disclosure-row mt-4">
            <summary>
              {isZh ? "详情" : "Details"}
              <Icon name="chevron-down" className="disclosure-chevron" />
            </summary>
            <dl className="disclosure-body runtime-details">
              <div>
                <dt>Crates</dt>
                <dd>{facts.crates.length ? `${facts.crates.length} · ${facts.crates.join(", ")}` : "—"}</dd>
              </div>
              <div>
                <dt>{isZh ? "源码版本" : "Source revision"}</dt>
                <dd><code className="inline">{facts.sourceRevision ?? "—"}</code></dd>
              </div>
            </dl>
          </details>
        </Section>

        <section className="page-section">
          <p className="status-line">
            <span>{isZh ? "详细实现文档：" : "Detailed implementation docs:"}</span>
            <a href={`${REPO_BLOB_BASE}/docs/RUNTIME_API.md`} target="_blank" rel="noopener noreferrer" className="link">
              {isZh ? "Runtime API 与 ACP stdio 适配器" : "Runtime API and ACP stdio adapter"}
            </a>
            <a href={`${REPO_BLOB_BASE}/docs/MCP.md`} target="_blank" rel="noopener noreferrer" className="link">
              {isZh ? "MCP 集成" : "MCP integration"}
            </a>
          </p>
        </section>
      </div>
    </>
  );
}
