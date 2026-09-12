import type { DocsWebDict } from "../types";

/** 中文对照见 `en/docs-web.ts`；本地客户端行为与 docs/WEB.md 及运行时保持一致。 */
export const docsWeb: DocsWebDict = {
  metaTitle: "浏览器客户端 · Codewhale 文档",
  metaDescription: "仅回环的内嵌浏览器客户端：一次性引导、会话 Cookie 与本地信任边界。",
  bodyClassName: "text-ink-soft leading-[1.9] tracking-wide",
  overviewTitle: "浏览器客户端",
  overviewLead:
    "{webCommand} 在 canonical 运行时 API 之上打开 Codewhale 内嵌的浏览器客户端。它是一个纯本地界面：服务器始终绑定 {loopbackHost}，无法改绑到局域网地址，也无法在关闭运行时认证的情况下运行。默认地址是 {defaultUrl}；端口冲突时用 {portExample} 换一个回环端口。Ctrl+C 停止进程，浏览器会话随之结束。",
  overviewBody:
    "当前客户端提供响应式的线程与搜索侧栏、由运行时持有的会话事实、transcript 与工具收据，以及输入区。它可以创建、选择、重命名和归档线程；发起或引导回合；中断工作；处理审批；回答运行时的用户输入请求。浏览器只是同一个本地运行时的另一视图——不会创建第二个云账号，不会把 provider 凭据复制进浏览器存储，也不会削弱已配置的审批与沙箱策略。",
  workflowTitle: "在浏览器中工作",
  workflowLead:
    "从历史侧栏搜索最近的对话或预览已保存的会话。预览保持只读，直到你选择恢复会话。创建对话时先选择提供商，再搜索其模型列表；起始提示只会填入输入区，供你检查后发送。完整的围栏代码块带有复制按钮，其他消息文本按原文显示。侧栏可以切换明暗主题；向上翻阅后，点击 Back to latest 可回到最新消息。",
  draftsBody:
    "未发送的草稿按对话保存在当前标签页的会话存储中，刷新后可以恢复。存储最多保留最近的 50 份草稿，采用尽力保存方式；存储不可用、已满或草稿数据过大时，草稿只保留在内存中，无法保证刷新后恢复。只有主题偏好使用本地存储。客户端不会把运行时令牌或已配置的提供商凭据写入这两种存储。",
  shortcutsBody:
    "使用 Cmd/Ctrl+K 搜索、Cmd/Ctrl+N 创建对话、Enter 发送、Shift+Enter 换行。输入法仍在组合文字时，按回车不会发送未完成的消息。",
  authTitle: "认证边界",
  authLead:
    "启动 URL 携带的是一个随机、短寿命、一次性的引导凭证——绝不是运行时 bearer 令牌。一次回环请求把它换成 HttpOnly、SameSite=Strict、进程本地的会话 Cookie，并立即使该凭证失效。重用、过期、畸形或非回环的引导尝试都会失败关闭。运行时令牌不会出现在渲染的 HTML、浏览器存储、URL 查询或片段、或浏览器启动参数中。携带 Cookie 的状态变更请求还必须出示精确的本地 web 源；跨源浏览器请求会被拒绝。",
  localTitle: "本地就是本地",
  localLead:
    "{webCommand} 只接受 {portFlag}——没有 {hostFlag}，也没有关闭认证的选项。不要把它当公开网站，也不要通过路由器转发、公开反向代理或隧道暴露它的端口。单独的 {mobileCommand} 和 {httpFlag} 模式有不同的部署与认证约定，操作它们（尤其是选择非回环绑定）之前请阅读运行时 API 文档。",
  remoteTitle: "从网页应用远程控制",
  remoteLead:
    "现已可用。要在已登录的 Codewhale 网页应用中继续正在运行的本地会话，请在该会话里输入 /rc，或用 codewhale rc 启动，然后在浏览器中批准一次性代码。租约有效期间，浏览器接管新的提示与审批，终端保持为可读的安全界面；两端都仍可中断。",
  remoteBody:
    "连接后，横幅和一条文字稿说明会显示实时会话链接。/rc open 在浏览器中打开它，/rc link 打印它，/rc status 显示会话归属，/rc stop 把会话交还给终端。连接中断时，本地输入会保持锁定，直到最后一个网页租约过期，因此两个控制端永远不会争抢。从同一终端登记的每个文件夹共享一个稳定的设备 ID，所以网页应用按机器而不是按会话列出计算机。这与上面的本机浏览器客户端不同：/rc 把本地会话与你的账户配对；codewhale web 只是提供一个不需要账户的本地页面。",
  troubleshootingTitle: "常见问题",
  troubleshootingLead:
    "端口 7878 被占用时用 --port 换一个。浏览器没有打开时，请在十分钟内用同一台机器打开终端打印的一次性启动 URL；如果已用过或过期，请重新启动。提供商不可用时，检查 codewhale doctor 和 /provider；web 命令不配置也不迁移凭据。运行时仍在运行但连接暂时中断时，点击 Reconnect 重新加载状态，草稿会保留。浏览器会话过期后，需要重启 codewhale web 签发新会话；Reconnect 无法续期。",
  sourceNote: "来源文档：docs/WEB.md · 更新时请同步修改 docs-map.ts。",
};
