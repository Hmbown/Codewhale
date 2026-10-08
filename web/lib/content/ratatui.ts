import { pickText } from "@/lib/i18n/dictionaries";
import type { LocalizedText } from "./vocabulary";

/** Public explorer copy. Component names, APIs and terminal fixtures stay verbatim. */
export interface RatatuiCopy {
  metaTitle: string;
  metaDescription: string;
  title: string;
  intro: string;
  searchLabel: string;
  searchPlaceholder: string;
  allComponents: string;
  families: string;
  componentCount: string;
  preview: string;
  code: string;
  api: string;
  source: string;
  profile: string;
  size: string;
  play: string;
  pause: string;
  reset: string;
  copy: string;
  copied: string;
  copyFailed: string;
  download: string;
  openSource: string;
  install: string;
  back: string;
  previous: string;
  next: string;
  terminalPreviewLabel: string;
  emptyTitle: string;
  emptyDescription: string;
  clearSearch: string;
  loading: string;
  previewError: string;
  retry: string;
  usageTitle: string;
  usageDescription: string;
  installationTitle: string;
  installationDescription: string;
  tryLocally: string;
  repository: string;
  viewGuide: string;
  quality: string;
  releaseHistory: string;
  checks: string;
  fixtureNote: string;
  motionNote: string;
  rustVersion: string;
  license: string;
  nativeSize: string;
  resultsLabel: string;
  share: string;
  fit: string;
  actualSize: string;
  columns: string;
  renderedPreviewNote: string;
  fixtureTitle: string;
  motion: string;
  motionDescription: string;
  frame: string;
  frameCount: string;
  speed: string;
  shareCopied: string;
  projectStatus: string;
  sourceRevision: string;
  buildChecks: string;
  galleryChecks: string;
  statusNote: string;
  keyboardHint: string;
  visibleCount: string;
  gettingStarted: string;
  hostNote: string;
  downloadSvg: string;
  previewLoading: string;
  componentNavLabel: string;
  websiteNote: string;
  reducedMotionNote: string;
  tasksTitle: string;
  tasksDescription: string;
  use: string;
  guide: string;
  runExample: string;
  relatedMotion: string;
  recipeNote: string;
  recipeControls: string;
  firstAppTitle: string;
  firstAppDescription: string;
  existingApp: string;
  themeNote: string;
  starterSource: string;
  stateTitle: string;
  stateDescription: string;
  paintTitle: string;
  paintDescription: string;
  actionsTitle: string;
  actionsDescription: string;
  heroLabel: string;
  heroAlt: string;
  heroCaption: string;
  heroOpenExplorer: string;
  heroOpenComponent: string;
  familyPosition: string;
}

export const RATATUI_COPY: Record<keyof RatatuiCopy, LocalizedText> = {
  metaTitle: { en: "Codewhale Ratatui · Terminal components", zh: "Codewhale Ratatui · 终端组件" },
  metaDescription: {
    en: "Explore Codewhale’s reusable Ratatui components. Compare terminal themes and widths, inspect real rendered previews, and use the Rust examples in your app.",
    zh: "探索 Codewhale 的可复用 Ratatui 组件。比较终端主题和宽度，查看真实渲染预览，并将 Rust 示例用于你的应用。",
  },
  title: { en: "Codewhale Ratatui", zh: "Codewhale Ratatui" },
  intro: {
    en: "Bring Codewhale’s terminal components into your app. Explore the composer, workbar, ocean, themes and creatures, then take the Rust code with you.",
    zh: "将 Codewhale 的终端组件用于你的应用。探索输入框、工作栏、海洋、主题与海洋生物，然后带走 Rust 代码。",
  },
  searchLabel: { en: "Find a component", zh: "查找组件" },
  searchPlaceholder: { en: "Search components, APIs or themes…", zh: "搜索组件、API 或主题…" },
  allComponents: { en: "All components", zh: "全部组件" },
  families: { en: "Collections", zh: "组件集" },
  componentCount: { en: "{count} previews", zh: "{count} 个预览" },
  preview: { en: "Preview", zh: "预览" },
  code: { en: "Gallery source", zh: "展示源码" },
  api: { en: "Components used", zh: "使用的组件" },
  source: { en: "Source", zh: "源码" },
  profile: { en: "Terminal profile", zh: "终端配置" },
  size: { en: "Width", zh: "宽度" },
  play: { en: "Play", zh: "播放" },
  pause: { en: "Pause", zh: "暂停" },
  reset: { en: "Restart", zh: "重新开始" },
  copy: { en: "Copy code", zh: "复制代码" },
  copied: { en: "Copied", zh: "已复制" },
  copyFailed: { en: "Select the code to copy it.", zh: "请选择代码后复制。" },
  download: { en: "Download SVG", zh: "下载 SVG" },
  openSource: { en: "Open source", zh: "查看源码" },
  install: { en: "Use in your app", zh: "用于你的应用" },
  back: { en: "Explore all components", zh: "探索全部组件" },
  previous: { en: "Previous component", zh: "上一个组件" },
  next: { en: "Next component", zh: "下一个组件" },
  terminalPreviewLabel: { en: "Terminal preview of {name}", zh: "{name} 的终端预览" },
  emptyTitle: { en: "No matching components", zh: "没有匹配的组件" },
  emptyDescription: { en: "Try a component name, an API such as NativeComposer, or another collection.", zh: "试试组件名称、NativeComposer 等 API，或其他组件集。" },
  clearSearch: { en: "Clear search", zh: "清除搜索" },
  loading: { en: "Loading the preview…", zh: "正在加载预览…" },
  previewError: { en: "The preview could not load. Try again or open its source.", zh: "无法加载预览。请重试或查看源码。" },
  retry: { en: "Try again", zh: "重试" },
  usageTitle: { en: "Compose your own terminal", zh: "构建你的终端界面" },
  usageDescription: {
    en: "Use a single widget or compose a workspace. Your app owns the state, clock and actions; the library renders the interface.",
    zh: "使用单个组件，或组合成完整工作区。你的应用管理状态、时钟与操作；组件库负责渲染界面。",
  },
  installationTitle: { en: "Start building", zh: "开始构建" },
  installationDescription: { en: "Add the library to your Cargo.toml. Use your existing backend, or enable Crossterm for a new app.", zh: "将组件库添加到 Cargo.toml。可使用现有终端后端，或为新应用启用 Crossterm。" },
  tryLocally: { en: "Try the interactive gallery", zh: "试用交互式展示" },
  repository: { en: "GitHub repository", zh: "GitHub 仓库" },
  viewGuide: { en: "Terminal view guide", zh: "终端视图指南" },
  quality: { en: "Quality and compatibility", zh: "质量与兼容性" },
  releaseHistory: { en: "Release history", zh: "发布历史" },
  checks: { en: "Latest checks", zh: "最新检查" },
  fixtureNote: { en: "This source reproduces the gallery preview, including its example data and internal helpers. For public API code you can reuse, choose Use in your app.", zh: "此源码用于重现展示预览，包含示例数据和内部辅助函数。如需复用公开 API 代码，请选择“用于你的应用”。" },
  motionNote: { en: "Animation uses rendered frames. Your app supplies the clock.", zh: "动画使用实际渲染的帧。时钟由你的应用提供。" },
  rustVersion: { en: "Rust 1.89+", zh: "Rust 1.89+" },
  license: { en: "MIT license", zh: "MIT 许可证" },
  nativeSize: { en: "Native width", zh: "原始宽度" },
  resultsLabel: { en: "Components matching your search", zh: "匹配搜索的组件" },
  share: { en: "Open component page", zh: "打开组件页面" },
  fit: { en: "Fit preview", zh: "适应预览宽度" },
  actualSize: { en: "Actual size", zh: "实际大小" },
  columns: { en: "{count} columns", zh: "{count} 列" },
  renderedPreviewNote: {
    en: "Rendered by the Rust components with example data. Choose a terminal profile and width to compare their layouts.",
    zh: "使用示例数据，由 Rust 组件渲染。选择终端配置和宽度，比较不同布局。",
  },
  fixtureTitle: { en: "Rendering source", zh: "渲染源码" },
  motion: { en: "Motion", zh: "动画" },
  motionDescription: { en: "Watch the rendered frames, or move through them at your own pace.", zh: "播放渲染帧，或按自己的节奏逐帧查看。" },
  frame: { en: "Frame", zh: "帧" },
  frameCount: { en: "{count} frames", zh: "{count} 帧" },
  speed: { en: "Speed", zh: "速度" },
  shareCopied: { en: "Link copied", zh: "链接已复制" },
  projectStatus: { en: "Follow the library", zh: "关注组件库" },
  sourceRevision: { en: "Library source", zh: "组件库源码" },
  buildChecks: { en: "Library checks", zh: "组件库检查" },
  galleryChecks: { en: "Gallery checks", zh: "展示检查" },
  statusNote: { en: "Open the latest checks and releases on GitHub.", zh: "在 GitHub 查看最新检查与发布。" },
  keyboardHint: { en: "Use the search field and collection links to find a component.", zh: "使用搜索框和组件集链接查找组件。" },
  visibleCount: { en: "{visible} of {total} previews", zh: "{total} 个预览中的 {visible} 个" },
  gettingStarted: { en: "Get started", zh: "开始使用" },
  hostNote: { en: "Your app owns state, timing and actions. Components render into the Ratatui frame you supply.", zh: "你的应用管理状态、时间与操作。组件渲染到你提供的 Ratatui 帧。" },
  downloadSvg: { en: "Download SVG", zh: "下载 SVG" },
  previewLoading: { en: "Loading the preview…", zh: "正在加载预览…" },
  componentNavLabel: { en: "Ratatui component collections", zh: "Ratatui 组件集" },
  websiteNote: { en: "Part of Codewhale. Built for your Ratatui app.", zh: "来自 Codewhale，用于你的 Ratatui 应用。" },
  reducedMotionNote: { en: "Playback is paused for reduced motion. You can step through the frames or choose Play.", zh: "已按减少动态效果的偏好暂停播放。你可以逐帧查看或选择播放。" },
  tasksTitle: { en: "What are you building?", zh: "你想构建什么？" },
  tasksDescription: { en: "Choose a starting point, or browse every component below.", zh: "选择一个起点，或浏览下方的全部组件。" },
  use: { en: "Use in your app", zh: "用于你的应用" },
  guide: { en: "Read the getting-started guide", zh: "阅读入门指南" },
  runExample: { en: "Run from the cloned library", zh: "在克隆的组件库中运行" },
  relatedMotion: { en: "Related animation demo", zh: "相关动画示例" },
  recipeNote: { en: "A public API starting point from the runnable recipes example. Pass your app’s theme and state; adapt the data and layout to your app.", zh: "这是可运行示例中的公开 API 起点。传入应用的主题与状态，并按需调整数据和布局。" },
  recipeControls: { en: "F1 composer · F2 workbar · F3 water · F4 whale · F5 animated whale. Press m in F5 to compare motion policies; Esc closes.", zh: "F1 输入框 · F2 工作栏 · F3 海洋背景 · F4 鲸鱼 · F5 动态鲸鱼。在 F5 中按 m 比较动画策略；Esc 退出。" },
  firstAppTitle: { en: "Start with a working app", zh: "从可运行的应用开始" },
  firstAppDescription: { en: "Type a message, press Enter to echo it, and Esc to exit. The starter connects editable input, the native composer and workbar in one small app.", zh: "输入消息，按 Enter 显示消息，按 Esc 退出。入门示例在一个小应用中连接可编辑输入、原生输入框与工作栏。" },
  existingApp: { en: "Add to an existing Ratatui app", zh: "添加到现有 Ratatui 应用" },
  themeNote: { en: "Detect the theme once when your app starts, then pass it to your draw function. Theme::detect().tui() selects the native terminal palette.", zh: "启动时检测一次主题，再传入绘制函数。Theme::detect().tui() 会选择原生终端配色。" },
  starterSource: { en: "See the complete starter", zh: "查看完整入门示例" },
  stateTitle: { en: "Keep the state", zh: "保存状态" },
  stateDescription: { en: "Your app stores drafts, selection, work and a persistent whale Stage.", zh: "应用保存草稿、选中项、工作数据及持续使用的鲸鱼 Stage。" },
  paintTitle: { en: "Render the view", zh: "渲染视图" },
  paintDescription: { en: "Components paint the facts you supply into your Ratatui frame.", zh: "组件将你提供的数据绘制到 Ratatui 帧中。" },
  actionsTitle: { en: "Handle the action", zh: "处理操作" },
  actionsDescription: { en: "Route input outcomes to your app. Supply elapsed time when motion needs a redraw.", zh: "将输入结果交给应用处理；动画需要重绘时，提供经过的时间。" },
  heroLabel: { en: "Codewhale terminal, rendered by the library", zh: "由组件库渲染的 Codewhale 终端" },
  heroAlt: { en: "A Codewhale terminal session: conversation, composer and Tasks workbar over the ocean background", zh: "Codewhale 终端会话：对话、输入框与任务工作栏，背景为海洋" },
  heroCaption: { en: "Native conversation, composer and Tasks workbar · Underwater truecolor · {width}×{height}", zh: "原生对话、输入框与任务工作栏 · Underwater 真彩色 · {width}×{height}" },
  heroOpenExplorer: { en: "Open in the explorer", zh: "在组件探索器中打开" },
  heroOpenComponent: { en: "About this composition", zh: "关于此组合" },
  familyPosition: { en: "{index} of {count} in this collection", zh: "本组件集中第 {index} 个，共 {count} 个" },
};

/** Use the site's deterministic locale selection rather than page-local copy forks. */
export function getRatatuiCopy(locale: string): RatatuiCopy {
  return Object.fromEntries(
    Object.entries(RATATUI_COPY).map(([key, text]) => [key, pickText(text, locale)]),
  ) as unknown as RatatuiCopy;
}
