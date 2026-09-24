import type { DocsTroubleshootingDict } from "../types";

/**
 * Simplified Chinese dictionary for
 * `app/[locale]/docs/troubleshooting/page.tsx`.
 */
export const docsTroubleshooting: DocsTroubleshootingDict = {
  metaTitle: "排障 · Codewhale 文档",
  metaDescription:
    "常见问题的快速分诊：挂起的回合、离线队列、崩溃恢复、schema 错误、MCP 故障与 Docker 说明。",
  bodyClassName: "text-ink-soft leading-[1.9] tracking-wide",
  overviewTitle: "排障",
  overviewLead:
    "先快速分诊：确认二进制与配置（codewhale --version、~/.codewhale/config.toml），需要更详细日志时用 RUST_LOG=codewhale_tui=debug 启动（HTTP 重试/重连用 RUST_LOG=codewhale_tui::client=debug；日志写入 ~/.codewhale/logs/），并看一眼 ~/.codewhale/sessions 与 ~/.codewhale/tasks 的当前状态。",
  incidents: [
    [
      "回合挂起或流停止",
      "前台 shell 命令还在跑时按 Ctrl+B 把它移到后台（回合继续，命令变成 /jobs 下的后台任务）；想取消回合本身用 Esc 或 Ctrl+C。检查 codewhale_tui::client 的重试日志和端点连通性，重启后确认此前在途的回合被标记为中断，而不是停在运行态。",
    ],
    [
      "网络中断 / 离线行为",
      "离线时新提示词会排队，队列按会话持久化在 ~/.codewhale/sessions/checkpoints/<session-id>.offline_queue.json（旧的全局 offline_queue.json 会在升级时被接管一次）。用 /queue list 查看，恢复连接后重新发送（/queue edit <n> 加回车，或走正常输入流程），队列清空后文件随之清除。",
    ],
    [
      "崩溃恢复",
      "每个会话的检查点保存在 ~/.codewhale/sessions/checkpoints/<session-id>.json（旧的 latest.json 仍会读取，但不再写入）；除非传入 --resume/--continue，启动会开新会话。用 codewhale --resume <id> 或 TUI 里的 Ctrl+R 显式恢复；若检查点 schema 比二进制新，升级二进制或移除过期检查点。",
    ],
    [
      "持久状态 schema 错误",
      "形如 schema vX is newer than supported vY 的错误涉及 sessions、运行时 thread/turn/item 记录和 tasks。先确认二进制版本，编辑前备份状态目录，然后用更新的兼容二进制运行，或归档不兼容记录并重建状态。",
    ],
    [
      "MCP / 工具执行失败",
      "校验 ~/.codewhale/mcp.json 的 schema 和服务器命令路径，手动确认服务器进程能启动，并在 TUI 历史/日志中检查沙箱拒绝。用 /mcp validate 诊断，可暂时禁用出问题的服务器隔离原因，验证后再启用。",
    ],
  ],
  dockerTitle: "Docker 说明",
  dockerLead:
    "每个发布都会向 GitHub Container Registry 推送多架构 Linux 镜像。默认镜像是保守的运行时镜像：以非 root 的 codewhale 用户（UID/GID 1000:1000）运行，不授予免密 sudo，用户状态放在挂载到 /home/codewhale/.codewhale 的卷里。可复现的安装请固定发布标签而不是 latest。",
  dockerToolboxNote:
    "需要在容器内使用 apt-get、编译工具链或包管理器时，不要改默认镜像约定——基于 docs/examples/Dockerfile.toolbox 构建显式的 toolbox 镜像，并为每个项目使用独立的命名状态卷，避免会话、配置和离线队列跨工作区串扰。不要把 API 密钥或 SSH 私钥烘进自定义镜像。",
  sourceNote:
    "来源文档：docs/OPERATIONS_RUNBOOK.md, docs/DOCKER.md · 更新时请同步修改 docs-map.ts。",
};
