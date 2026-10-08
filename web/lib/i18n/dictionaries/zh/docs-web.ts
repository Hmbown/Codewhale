import type { DocsWebDict } from "../types";

/** 「打开浏览器客户端」页的简体中文词典；与 `en/docs-web.ts` 逐段对应。 */
export const docsWeb: DocsWebDict = {
  metaTitle: "打开浏览器客户端 · Codewhale 文档",
  metaDescription:
    "在你自己机器上的浏览器标签页里使用 Codewhale，或者用 /rc 在 Codewhale 网页应用中接着使用正在终端里运行的会话。",
  bodyClassName: "text-ink-soft leading-[1.9] tracking-wide",
  title: "打开浏览器客户端",
  lede:
    "比起终端，更喜欢浏览器窗口？Codewhale 可以在你的机器上提供自己的客户端。它只是同一个本地会话的另一种视图——审批相同、沙箱相同，也不需要账户。",
  sections: [
    {
      id: "start",
      title: "启动",
      blocks: [
        { p: "在你希望 Codewhale 工作的文件夹中运行：" },
        { code: "codewhale web\ncodewhale web --port 8788   # if 7878 is taken", lang: "终端" },
        {
          p: "Codewhale 会在 `http://127.0.0.1:7878` 启动本地服务，打印一个一次性链接，并在默认浏览器中打开它。如果浏览器没有打开，请在十分钟内使用打印出来的链接。在终端按 Ctrl+C 即可停止，浏览器会话也随之结束。",
        },
      ],
    },
    {
      id: "use",
      title: "在浏览器中工作",
      blocks: [
        {
          p: "浏览器客户端可以列出和搜索你的会话线程，显示对话记录及每个工具的结果，并提供输入框。你可以开始、引导或中断一个回合，回应审批，给线程改名或归档。你的提供商密钥始终留在 Codewhale 中，不会被复制到浏览器存储里。",
        },
      ],
    },
    {
      id: "local",
      title: "只在本机使用",
      blocks: [
        {
          list: [
            "服务只监听 `127.0.0.1`。没有任何选项能把它开放到你的网络，也无法在不认证的情况下运行。",
            "链接中携带的是一次性代码，而不是你的访问令牌。打开链接时，这个代码会换成一个绑定到当前进程的 cookie，随即失效。",
            "不要通过路由器、公共代理或隧道转发这个端口。如果要在手机或另一台机器上使用，请参阅 [Runtime API](/docs/runtime-api)，并先读一读它的认证规则。",
          ],
        },
      ],
    },
    {
      id: "remote",
      title: "在网页应用中接着使用会话",
      blocks: [
        {
          p: "这是另一回事：它把已经在终端里运行的会话交给已登录的 Codewhale 网页应用，让你可以换一台设备继续。这需要一个 [Codewhale 账户](/docs/auth#account)。",
        },
        {
          code: `/rc          # in the running session; approve the one-time code in your browser
/rc status   # who controls the session now
/rc link     # print the session link
/rc stop     # hand control back to the terminal`,
          lang: "Codewhale",
        },
        {
          p: "网页应用接管会话期间，新的提示和审批都来自浏览器，终端仍可查看内容。任何一方都可以中断。你也可以用 `codewhale rc` 直接以这种方式启动会话。",
        },
      ],
    },
    {
      id: "fix",
      title: "遇到问题时",
      blocks: [
        {
          rows: [
            ["端口被占用", "用 `--port` 指定一个空闲端口。"],
            ["浏览器没有打开", "在十分钟内，把打印出来的链接复制到同一台机器上的浏览器中打开。"],
            ["链接已过期或已被使用", "这是预期行为。重新运行 `codewhale web` 获取新链接。"],
            ["模型没有回复", "web 命令不负责配置提供商。请检查 `codewhale doctor` 和 `/provider`。"],
          ],
        },
      ],
    },
  ],
  next: [
    {
      href: "/docs/runtime-api",
      label: "用 Runtime API 自动化",
      note: "浏览器客户端所基于的本地 API。",
    },
    {
      href: "/docs/auth",
      label: "连接模型提供商",
      note: "为浏览器客户端接上一个模型。",
    },
    {
      href: "/docs/modes",
      label: "设置模式与审批",
      note: "浏览器中同样适用这些审批设置。",
    },
  ],
  sourceNote: "来源文档：docs/WEB.md · 修改时同步更新 docs-map.ts。",
};
