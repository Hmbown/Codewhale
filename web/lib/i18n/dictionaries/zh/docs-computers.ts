import type { DocsComputersDict } from "../types";

/** 「把任务发送到云端」页的简体中文词典；与 `en/docs-computers.ts` 逐段对应。 */
export const docsComputers: DocsComputersDict = {
  metaTitle: "把任务发送到云端 · Codewhale 文档",
  metaDescription:
    "把编码任务交给 Codewhale 云端 Agent，由它在一个分支上完成工作并提交拉取请求——先提议，只有你确认后才会开始。",
  bodyClassName: "text-ink-soft leading-[1.9] tracking-wide",
  title: "把任务发送到云端",
  lede:
    "云端 Agent 会把任务从你的机器上接走：它在一个全新的云端沙箱中工作，推送一个分支并提交拉取请求，你则可以继续在本地工作。在你确认之前，不会启动任何东西、不会产生费用，也不会推送代码。",
  sections: [
    {
      id: "status",
      title: "了解目前的完成度",
      blocks: [
        {
          note: "云端 Agent 目前是预览版。完整流程已有离线测试覆盖，但真实链路——真实的沙箱，以及在各个代码托管平台上真实提交的拉取请求——尚未经过端到端验证。暂不支持私有仓库。",
        },
      ],
    },
    {
      id: "before",
      title: "开始之前",
      blocks: [
        {
          list: [
            "用 `codewhale login` 登录你的 Codewhale 账户。未登录时，任务只能提议，无法确认。",
            "把账户 API 密钥设为 `CODEWHALE_API_KEY`，让沙箱中的 Agent 以你的账户身份运行。缺少它时，确认会在产生任何费用之前被拒绝。",
            "如果使用 GitHub，请先登录 `gh` 命令行工具；Codewhale 会用这个登录来提交拉取请求。",
          ],
        },
        { code: "codewhale dispatch --status", lang: "终端" },
        {
          p: "`--status` 会显示从你的 git 远程仓库中识别出的代码托管平台，以及所需的凭据是否齐全。它从不打印任何密钥。",
        },
      ],
    },
    {
      id: "send",
      title: "先提议，再确认",
      blocks: [
        {
          code: `codewhale dispatch "fix the flaky login test and open a PR" --remote github
codewhale dispatch --confirm cloud_<id>`,
          lang: "终端",
        },
        {
          p: "第一条命令只写入一份提议并打印其 id；第二条命令才真正启动。Codewhale 可以自己提议云端任务，但从不自行确认。在会话中，使用 `/dispatch <任务>` 和 `/dispatch confirm <id>`。",
        },
        {
          p: "确认之后，Agent 会把仓库克隆到一个新沙箱中完成工作，推送一个新分支（绝不强制推送），并提交拉取请求。任务完成、失败或被取消时，沙箱都会被删除。",
        },
      ],
    },
    {
      id: "track",
      title: "跟踪或取消任务",
      blocks: [
        {
          code: `codewhale dispatch --list
codewhale dispatch --show cloud_<id>
codewhale dispatch --cancel cloud_<id>`,
          lang: "终端",
        },
        {
          p: "云端任务也会出现在 `/jobs` 中。每个任务会显示进度、分支、拉取请求链接（有了之后），以及运行了多少分钟——这是运行时长，不是账单。取消会立即拆除沙箱。",
        },
      ],
    },
    {
      id: "forge",
      title: "选择拉取请求提交到哪里",
      blocks: [
        {
          p: "Codewhale 支持 GitHub、CNB 和 Gitee，且从不假定 `origin` 就是 GitHub。名为 `github`、`cnb` 或 `gitee` 的远程仓库即对应该平台；其他远程仓库按其主机名识别。如果仓库关联了多个平台，请传入 `--remote`。",
        },
        {
          p: "如果拉取请求无法提交——例如缺少代码托管平台的令牌——任务会在推送之后被标记为失败，并说明没有提交拉取请求。它绝不会报告一个并不存在的链接。",
        },
      ],
    },
  ],
  next: [
    {
      href: "/docs/auth",
      label: "连接模型提供商",
      note: "登录 Codewhale 账户并管理密钥。",
    },
    {
      href: "/docs/review",
      label: "查看改动",
      note: "合并之前，先审查 Agent 提交的拉取请求。",
    },
    {
      href: "/docs/fleet",
      label: "运行 Workflow",
      note: "改为在你自己的机器上运行更长的多步骤工作。",
    },
  ],
  sourceNote: "来源文档：docs/DAYTONA_CLOUD_DISPATCH.md、docs/CODEWHALE_AGENT.md · 修改时同步更新 docs-map.ts。",
};
