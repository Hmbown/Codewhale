import type { DocsMcpDict } from "../types";

/** 「用 MCP 连接工具」页的简体中文词典；与 `en/docs-mcp.ts` 逐段对应。 */
export const docsMcp: DocsMcpDict = {
  metaTitle: "用 MCP 连接工具 · Codewhale 文档",
  metaDescription:
    "添加 Model Context Protocol 服务器让 Codewhale 使用更多工具，登录远程服务器，把 Codewhale 本身作为 MCP 服务器运行，并试用代码模式。",
  bodyClassName: "text-ink-soft leading-[1.9] tracking-wide",
  title: "用 MCP 连接工具",
  lede:
    "MCP 服务器能为 Codewhale 带来更多工具——数据库、问题跟踪系统、浏览器等。你可以添加一个由 Codewhale 替你启动的本地服务器，也可以通过 URL 添加远程服务器。这些工具与内置工具走同一套审批流程。",
  sections: [
    {
      id: "add",
      title: "添加服务器",
      blocks: [
        {
          code: `codewhale mcp add git --command "uvx" --arg "mcp-server-git"
codewhale mcp add docs --url "https://example.com/mcp"
codewhale mcp list
codewhale mcp validate`,
          lang: "终端",
        },
        {
          p: "`--command` 通过 stdio 启动本地服务器，每个参数用一个 `--arg`。`--url` 通过 Streamable HTTP 连接远程服务器，必要时回退到旧版 SSE。`mcp validate` 会检查配置文件以及你标记为必需的服务器。",
        },
        {
          p: "在会话中，`/mcp` 会打开 MCP 管理器：显示每个服务器的状态、传输方式、超时设置、错误信息和已发现的工具。同样的操作在那里也能完成，例如 `/mcp add stdio <name> <command>` 和 `/mcp add http <name> <url>`。",
        },
        {
          note: "MCP 服务器以你的权限运行。只添加你信任的服务器，就像只安装你信任的程序一样。",
        },
      ],
    },
    {
      id: "remote-auth",
      title: "登录远程服务器",
      blocks: [
        {
          p: "使用 OAuth 的服务器，先用 URL 添加再登录。使用 bearer token 的服务器，把令牌放在环境变量里，而不是写进配置文件：",
        },
        {
          code: `codewhale mcp login docs
codewhale mcp add tracker --url "https://example.com/mcp" --bearer-token-env-var TRACKER_TOKEN`,
          lang: "终端",
        },
        {
          p: "显式设置的 Authorization 请求头始终优先：先应用配置中的请求头，其次是 bearer token 环境变量，最后才是已保存的 OAuth 登录。`codewhale mcp logout <name>` 会删除本机保存的登录信息；提供方那边的授权可能仍然有效，需要到提供方处撤销。",
        },
      ],
    },
    {
      id: "config",
      title: "编辑配置文件",
      blocks: [
        {
          p: "服务器配置保存在 `~/.codewhale/mcp.json`，`codewhale mcp init` 会生成一个初始文件。其他客户端使用的 `mcpServers` 键同样可用，现成的配置可以直接粘贴进来。",
        },
        {
          code: `{
  "servers": {
    "example": {
      "command": "node",
      "args": ["./path/to/your-mcp-server.js"],
      "env": {},
      "disabled": false
    }
  }
}`,
          lang: "mcp.json",
        },
        {
          p: "修改文件后，在会话中运行 `/mcp reload` 即可，无需重启。服务器只在某个回合需要它的工具时才启动；如果希望它在启动时就连接，把它标记为 `\"required\": true`。",
        },
      ],
    },
    {
      id: "tool-names",
      title: "找到这些工具",
      blocks: [
        {
          p: "每个工具在模型眼中的名字是 `mcp_<server>_<tool>`：名为 `git` 的服务器提供的 `status` 工具，就叫 `mcp_git_status`。`codewhale mcp tools <server>` 会列出某个服务器提供的工具。连接失败或已禁用的服务器，其工具永远不会显示为可用。",
        },
        {
          p: "MCP 工具遵循你的[审批设置](/docs/modes)：在策略允许时，列出和读取服务器的资源与提示词无需确认；有副作用的工具会先征求你的同意。Full Access 也不会绕过仓库规则或托管策略。",
        },
      ],
    },
    {
      id: "serve",
      title: "把 Codewhale 作为 MCP 服务器运行",
      blocks: [
        {
          p: "其他 MCP 客户端——包括另一个 Codewhale 会话——都可以使用 Codewhale 的工具。只需注册一次：",
        },
        {
          code: `codewhale mcp add-self
codewhale mcp tools codewhale`,
          lang: "终端",
        },
        {
          p: "`add-self` 会写入一条通过 stdio 运行 `codewhale serve --mcp` 的配置。每个客户端各自启动一个进程，不会打开任何网络端口。`codewhale serve --http` 则是另一回事——它是供应用使用的 [Runtime API](/docs/runtime-api)。",
        },
      ],
    },
    {
      id: "code-mode",
      title: "用代码模式组合工具调用（实验性）",
      blocks: [
        {
          p: "代码模式让模型写一段简短的 JavaScript 程序，在其中调用多个工具、循环和筛选结果，而不必把每次调用都作为单独的一步。只有程序的最终结果会返回给模型，长时间的查找因此更紧凑。它默认关闭。可以只在一次会话中试用，也可以在配置中开启：",
        },
        {
          code: `codewhale --enable code_mode

# ~/.codewhale/config.toml
[features]
code_mode = true`,
          lang: "终端 / config.toml",
        },
        {
          list: [
            "程序中只能调用无需审批的只读工具。任何写入、运行 shell 命令或需要征求你同意的调用，都会让程序停止，并报告被拒绝的是哪一次调用。",
            "程序中暂时还不能调用 MCP 工具，请把它们作为普通工具调用来使用。",
            "每个程序的上限：50 次工具调用、同时最多 4 次、30 秒、返回 16 KiB。",
            "Plan 模式下不能使用代码模式。",
          ],
        },
      ],
    },
  ],
  next: [
    {
      href: "/docs/hooks",
      label: "在事件发生时运行命令",
      note: "在工具调用执行前检查或改写它，MCP 工具也不例外。",
    },
    {
      href: "/docs/modes",
      label: "设置模式与审批",
      note: "决定哪些 MCP 调用需要你批准。",
    },
    {
      href: "/docs/runtime-api",
      label: "用 Runtime API 自动化",
      note: "通过 HTTP 从你自己的应用或脚本驱动 Codewhale。",
    },
  ],
  sourceNote:
    "来源文档：docs/MCP.md、crates/tui/src/tools/codemode.rs · 修改时同步更新 docs-map.ts。",
};
