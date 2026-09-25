import type { DocsRuntimeApiDict } from "../types";

/** 「用 Runtime API 自动化」页的简体中文词典；与 `en/docs-runtime-api.ts` 逐段对应。 */
export const docsRuntimeApi: DocsRuntimeApiDict = {
  metaTitle: "用 Runtime API 自动化 · Codewhale 文档",
  metaDescription:
    "用你自己的脚本和应用驱动 Codewhale：在 CI 中运行一次性任务，或启动本地 HTTP API 来发送回合、接收事件流并回应审批。",
  bodyClassName: "text-ink-soft leading-[1.9] tracking-wide",
  title: "用 Runtime API 自动化",
  lede:
    "脚本和应用可以驱动你在终端里使用的同一个引擎。单个任务用 `codewhale exec`；需要会话线程、实时事件和审批的应用，则运行本地的 Runtime API。一切都在你的机器上运行，没有托管中转。",
  sections: [
    {
      id: "exec",
      title: "在脚本中运行单个任务",
      blocks: [
        {
          code: `codewhale exec "Reply with exactly: pong"
codewhale exec --auto "fix the failing test and run it again"
codewhale exec --auto --output-format stream-json "update the changelog"`,
          lang: "终端",
        },
        {
          p: "普通的 `exec` 只回答一次，不使用工具。`--auto` 允许它使用工具并自动批准，所以只在你信任的仓库或容器中使用；它绝不会放宽[沙箱](/docs/sandbox)。`--output-format stream-json` 每行输出一个 JSON 事件，并保存会话，以便之后用 `--continue` 接着运行。`--max-turns` 和 `--allowed-tools` 可以为一次运行设定限制。",
        },
      ],
    },
    {
      id: "start",
      title: "启动 Runtime API",
      blocks: [
        {
          code: `export CODEWHALE_RUNTIME_TOKEN="$(openssl rand -hex 32)"
codewhale app-server --http            # http://127.0.0.1:7878`,
          lang: "终端",
        },
        {
          p: "请在启动前自己设置令牌；如果不设置，Codewhale 会为该进程生成一个令牌，但不会打印出来。每个 `/v1/*` 请求都必须以 `Authorization: Bearer <token>` 的形式携带它。`--port` 可以更改端口。",
        },
      ],
    },
    {
      id: "turn",
      title: "发送一个回合并观察它",
      blocks: [
        {
          code: `API=http://127.0.0.1:7878
AUTH="Authorization: Bearer $CODEWHALE_RUNTIME_TOKEN"

THREAD=$(curl -s -X POST "$API/v1/threads" -H "$AUTH" \\
  -H "Content-Type: application/json" -d '{}' | jq -r .id)

curl -s -X POST "$API/v1/threads/$THREAD/turns" -H "$AUTH" \\
  -H "Content-Type: application/json" -d '{"prompt": "Summarize README.md"}'

curl -N "$API/v1/threads/$THREAD/events?since_seq=0" -H "$AUTH"`,
          lang: "终端",
        },
        {
          p: "线程就是一段对话；回合是一次请求，以及 Codewhale 为它所做的一切。事件流会从你给出的序号开始回放，然后保持连接等待新事件，因此重新连接的客户端不会漏掉任何内容。",
        },
        {
          rows: [
            ["停止回合", "`POST /v1/threads/{id}/turns/{turn_id}/interrupt`"],
            ["回应审批", "`POST /v1/approvals/{approval_id}`"],
            ["引导正在运行的回合", "`POST /v1/threads/{id}/turns/{turn_id}/steer`"],
            ["列出已保存的会话", "`GET /v1/sessions`"],
          ],
        },
        {
          p: "[docs/RUNTIME_API.md](https://github.com/Hmbown/CodeWhale/blob/main/docs/RUNTIME_API.md) 列出了所有路由、请求体和事件。",
        },
      ],
    },
    {
      id: "other",
      title: "选择其他连接方式",
      blocks: [
        {
          rows: [
            ["codewhale app-server --stdio", "通过标准输入输出传输 JSON-RPC，不监听网络。适合 SDK 或本地探测。"],
            ["codewhale serve --acp", "面向 Zed 等编辑器的 Agent Client Protocol。"],
            ["codewhale serve --mcp", "把 Codewhale 的工具提供给其他 MCP 客户端。见[用 MCP 连接工具](/docs/mcp)。"],
            ["codewhale web", "内置的[浏览器客户端](/docs/web)，基于同一个 API。"],
            ["codewhale doctor --json", "以 JSON 输出健康状况和能力信息，不含任何密钥。"],
          ],
          codeTerms: true,
        },
      ],
    },
    {
      id: "security",
      title: "保持私密",
      blocks: [
        {
          list: [
            "服务默认只监听 `127.0.0.1`。令牌只是本地防护，不能替代 TLS 或 VPN；不要把端口暴露到网络上。",
            "`--insecure-no-auth` 只在回环地址上被接受。",
            "API 从不返回你的提供商密钥。健康和能力报告只包含元数据——没有密钥、文件内容或消息。",
          ],
        },
      ],
    },
  ],
  next: [
    {
      href: "/docs/web",
      label: "打开浏览器客户端",
      note: "基于同一个 API 的现成客户端。",
    },
    {
      href: "/docs/hooks",
      label: "在事件发生时运行命令",
      note: "不写客户端，也能响应会话事件。",
    },
    {
      href: "/docs/fleet",
      label: "运行 Workflow",
      note: "可在任何终端查看的持久多步骤运行。",
    },
  ],
  sourceNote: "来源文档：docs/RUNTIME_API.md · 修改时同步更新 docs-map.ts。",
};
