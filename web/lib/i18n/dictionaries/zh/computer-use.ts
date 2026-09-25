import type { ComputerUseDict } from "../types";

/**
 * Simplified-Chinese dictionary for `app/[locale]/computer-use/page.tsx` and
 * the Computer Use section of the install page. Product names, the menu
 * items Pause, Stop and Check for updates, and the macOS setting names stay
 * as the app shows them.
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Mac 版 Computer Use · Codewhale",
  metaDescription: "下载并设置 Mac 版 Codewhale Computer Use。提供应用后台操作、权限设置，以及由用户控制的暂停和停止功能。",
  title: "Computer Use",
  lead: "让 Codewhale 在应用中完成操作，你也可以继续工作。Mac 助手将权限设置、应用后台操作，以及暂停或停止输入的控制集中在菜单栏中。",
  publisher: "由 Codewhale 开发",
  download: "下载 Mac 版",
  downloadZip: "ZIP 压缩包（供应用内更新使用）",
  requirements: "macOS 13.5 或更高版本 · Apple 芯片与 Intel",
  included: "只需下载一个应用，无需另行安装 Node 或编译器。",
  pendingTitle: "Mac 下载包准备中",
  pendingBody: "完成 Apple 公证和发布检查后，正式安装包将在此提供。",
  unavailableTitle: "暂时无法查询下载状态",
  unavailableBody: "请刷新页面重试，或通过下方链接查看已发布版本。",
  releases: "已发布版本",
  receipt: "下载校验信息",
  setup: "设置你的 Mac",
  steps: [
    { title: "安装应用", body: "打开下载的磁盘映像，将 Codewhale Computer Use 拖入“应用程序”。从“应用程序”中打开它，然后点击菜单栏中的鲸鱼图标，选择 Computer Use。" },
    { title: "检查权限", body: "通过设置按钮打开系统设置中的“辅助功能”和“屏幕录制”。由你决定授予哪些权限。" },
    { title: "运行后台检查", body: "助手会打开一个临时练习窗口，输入文本并截取该窗口，同时检查运行期间指针或前台应用是否发生变化。" },
    { title: "连接 Codewhale", body: "在 Codewhale 插件市场中审查、信任并启用 Computer Use。请使用 0.3.1 或更高版本，确保本地操作受助手的暂停和停止控制。" },
  ],
  controlsTitle: "继续工作，掌握控制权。",
  controlsBody: "受支持的操作可在选定应用的后台完成。需要前台控制的应用或手势必须得到你的授权。菜单会显示目标应用与输入模式；Pause 暂停助手输入，Stop 结束助手的现有会话。",
  updateTitle: "由你决定何时更新",
  updateBody: "在应用中选择 Check for updates。安装更新前，助手会校验下载文件、Codewhale 签名和 Apple 公证，并保留旧版应用以便恢复。",
  help: "设置与故障排查",
  notes: "版本说明",
  demo: "查看后台检查演示",
  source: "源码与其他平台",
  platforms: "此下载适用于 Mac。Windows 和 Linux 目前通过源码插件及宿主端设置使用。",
};
