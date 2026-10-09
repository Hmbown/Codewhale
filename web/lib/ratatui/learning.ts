import type { LocalizedText } from "@/lib/content/vocabulary";
import { pickText } from "@/lib/i18n/dictionaries";
import type { CatalogueEntry } from "./catalogue";

export interface LearningTask {
  id: string;
  label: LocalizedText;
  description: LocalizedText;
  families: string[];
  recommended: string[];
}

/** Intent routes sit beside the generated catalogue so exports remain reproducible. */
export const TASKS: LearningTask[] = [
  {
    id: "build-app", label: { en: "Build a terminal app", zh: "构建终端应用" },
    description: { en: "Start with the native shell, composer and workbar, then arrange your own workspace.", zh: "从原生界面框架、输入框与工作栏开始，再组合自己的工作区。" },
    families: ["studio", "native-views", "native-chrome", "workbar", "scenes"],
    recommended: ["native-composer", "workbar-tasks", "showcase-work"],
  },
  {
    id: "input", label: { en: "Accept input", zh: "接收输入" },
    description: { en: "Add editable fields, search, choices, forms and keyboard navigation.", zh: "添加可编辑字段、搜索、选择、表单与键盘导航。" },
    families: ["input", "chrome"], recommended: ["text-input-typed", "picker-query", "form"],
  },
  {
    id: "conversations", label: { en: "Show conversations", zh: "展示对话" },
    description: { en: "Render messages, code, attached context, queued input and agents.", zh: "渲染消息、代码、附加上下文、排队输入与智能体。" },
    families: ["transcript", "components"], recommended: ["message", "transcript-code", "pending-queued"],
  },
  {
    id: "review", label: { en: "Review work", zh: "审查工作" },
    description: { en: "Explain changes, request a decision and present results supplied by your app.", zh: "说明变更、请求决定，并展示应用提供的结果。" },
    families: ["display"], recommended: ["diff", "approval-command", "receipt-table"],
  },
  {
    id: "style", label: { en: "Style your terminal", zh: "设计终端风格" },
    description: { en: "Choose a Codewhale theme, ocean depth and readable terminal details.", zh: "选择 Codewhale 主题、海洋深度与易读的终端细节。" },
    families: ["tui-palettes", "water", "foundation"], recommended: ["tui-theme-underwater", "ocean-column", "depth"],
  },
  {
    id: "motion", label: { en: "Add motion and life", zh: "添加动画与海洋生物" },
    description: { en: "Show progress with spinners, or add the whale, fish and jellyfish.", zh: "用加载动画展示进度，或添加鲸鱼、鱼群与水母。" },
    families: ["motion", "habitat", "whales", "whale-actions"], recommended: ["spinner", "whale-busy", "fish-school"],
  },
];

export interface FamilyGuidance {
  purpose: LocalizedText;
  responsibility: LocalizedText;
  example: string;
}

/** Teach composition without making the gallery's example state an application authority. */
export const FAMILY_GUIDANCE: Record<string, FamilyGuidance> = {
  studio: {
    purpose: { en: "See how the native composer, conversation and workbar fit together.", zh: "了解原生输入框、对话与工作栏如何组合。" },
    responsibility: { en: "Adapt the layout to your data; your app handles input and session state.", zh: "根据自己的数据调整布局；输入与会话状态由应用处理。" },
    example: "examples/showcase.rs",
  },
  "native-views": {
    purpose: { en: "Build session lists, settings and pickers from Codewhale's terminal parts.", zh: "用 Codewhale 终端组件构建会话列表、设置与选择器。" },
    responsibility: { en: "Supply rows and selection, then handle the resulting actions in your app.", zh: "提供行与选择状态，并在应用中处理相应操作。" },
    example: "examples/gallery.rs",
  },
  "native-chrome": {
    purpose: { en: "Compose the native input area, permission words, workflow progress and metrics.", zh: "组合原生输入区域、权限文字、工作流进度与指标。" },
    responsibility: { en: "Own the editable draft, cursor and reported state; painting does not submit a message.", zh: "管理可编辑草稿、光标与状态；渲染不会发送消息。" },
    example: "examples/starter.rs",
  },
  workbar: {
    purpose: { en: "Show Tasks, Fleet, Jobs, Files, Notes, Context, Git or Cost in a native workbar.", zh: "在原生工作栏中展示任务、Fleet、作业、文件、笔记、上下文、Git 或费用。" },
    responsibility: { en: "Supply stable row IDs and panel facts, and connect selection to your own actions.", zh: "提供稳定的行 ID 与面板数据，并将选择连接到自己的操作。" },
    example: "examples/starter.rs",
  },
  scenes: {
    purpose: { en: "Explore optional workspace, review, fleet and habitat compositions.", zh: "探索可选的工作区、审查、Fleet 与海洋生境组合。" },
    responsibility: { en: "Reuse the parts you need; these scenes contain example data rather than live sessions.", zh: "复用所需组件；这些场景包含示例数据，而非实时会话。" },
    example: "examples/gallery.rs",
  },
  input: {
    purpose: { en: "Edit text, validate fields and choose from searchable lists.", zh: "编辑文本、验证字段，并从可搜索列表中选择。" },
    responsibility: { en: "Keep input state and validation in your app, and handle submitted or cancelled outcomes.", zh: "在应用中管理输入状态与验证，并处理提交或取消结果。" },
    example: "examples/starter.rs",
  },
  chrome: {
    purpose: { en: "Add headings, tabs, toggles, settings details and keyboard hints.", zh: "添加标题、标签页、开关、设置详情与键盘提示。" },
    responsibility: { en: "Update selected values and dispatch keys; the controls display your current state.", zh: "更新选定值并分发按键；控件展示应用当前的状态。" },
    example: "examples/gallery.rs",
  },
  transcript: {
    purpose: { en: "Display structured prose, code, links, attachments and queued instructions.", zh: "展示结构化正文、代码、链接、附件与排队指令。" },
    responsibility: { en: "Supply parsed content and queue state; your app owns scrolling, links and clipboard actions.", zh: "提供解析后的内容与队列状态；应用负责滚动、链接与剪贴板操作。" },
    example: "examples/gallery.rs",
  },
  components: {
    purpose: { en: "Combine messages, tool and agent cards, attention queues and workspace framing.", zh: "组合消息、工具与智能体卡片、待处理队列及工作区框架。" },
    responsibility: { en: "Supply conversation and agent facts, and keep execution and priority decisions in your app.", zh: "提供对话与智能体数据；执行与优先级决策由应用负责。" },
    example: "examples/gallery.rs",
  },
  display: {
    purpose: { en: "Present diffs, approvals, artifacts, review decisions and run results.", zh: "展示差异、审批、产物、审查决定与运行结果。" },
    responsibility: { en: "Supply the changes and results, and handle approval choices in your app.", zh: "提供变更与结果，并在应用中处理审批选择。" },
    example: "examples/gallery.rs",
  },
  "tui-palettes": {
    purpose: { en: "Compare the sixteen native Codewhale themes and their backgrounds and semantic inks.", zh: "比较十六种原生 Codewhale 主题及其背景与语义颜色。" },
    responsibility: { en: "Choose a palette and terminal capabilities; use semantic roles for readable states.", zh: "选择调色板与终端能力，并用语义角色保证状态易读。" },
    example: "examples/showcase.rs",
  },
  water: {
    purpose: { en: "Apply native ocean depth or an optional ombré treatment to rendered components.", zh: "为已渲染组件添加原生海洋深度或可选渐变效果。" },
    responsibility: { en: "Paint components first, then apply the background pass with your viewport and motion policy.", zh: "先渲染组件，再根据视口与动画策略应用背景效果。" },
    example: "examples/starter.rs",
  },
  foundation: {
    purpose: { en: "Use shared surfaces, depth, dividers, icons, status marks and key hints.", zh: "使用共享表面、层次、分隔线、图标、状态标记与按键提示。" },
    responsibility: { en: "Choose meaningful labels and layout bounds, and retain words alongside status color.", zh: "选择清晰标签与布局边界，并在状态颜色旁保留文字。" },
    example: "examples/gallery.rs",
  },
  motion: {
    purpose: { en: "Show running work, verification, notifications and calm state transitions.", zh: "展示进行中的工作、验证、通知与平静的状态过渡。" },
    responsibility: { en: "Supply elapsed time and completion status; choose Full, Reduced or Still motion and schedule redraws.", zh: "提供经过时间与完成状态；选择 Full、Reduced 或 Still 动画并安排重绘。" },
    example: "examples/motion.rs",
  },
  habitat: {
    purpose: { en: "Render fish schools, jellyfish and bubbles together or individually.", zh: "组合或单独渲染鱼群、水母与气泡。" },
    responsibility: { en: "Supply the clock, density and motion preference; the creatures render into your chosen area.", zh: "提供时钟、密度与动画偏好；海洋生物渲染到选定区域。" },
    example: "examples/habitat.rs",
  },
  whales: {
    purpose: { en: "Give session state a whale: resting, busy, needing attention, done or with a pod.", zh: "用鲸鱼表现会话状态：休息、忙碌、需要关注、完成或带有鲸群。" },
    responsibility: { en: "Map your app's state to WhaleState; use the compact or words-only fallback when space is tight.", zh: "将应用状态映射到 WhaleState；空间不足时使用紧凑或纯文字样式。" },
    example: "examples/showcase.rs",
  },
  "whale-actions": {
    purpose: { en: "Explore the whale's full action vocabulary and related animated performances.", zh: "探索鲸鱼的完整动作语言与相关动画表现。" },
    responsibility: { en: "For animation, keep a whale_motion Stage and supply inputs and time from your host.", zh: "播放动画时保留 whale_motion Stage，并由应用提供输入与时间。" },
    example: "examples/showcase.rs",
  },
};

export function normalizeSearch(value: string): string {
  return value.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase();
}

/** Name-scoped aliases avoid the repeated family descriptions matching unrelated widgets. */
const ALIASES: [RegExp, string][] = [
  [/^(tui-theme-|view-theme$)/, "themes palette colors colours 主题 调色板"],
  [/^(atmosphere-|ocean-)/, "ombre ombres gradient background underwater 渐变 背景 海洋"],
  [/^(spinner$|verification-pending$|motion-working$)/, "loading busy waiting 加载 等待"],
  [/^verification-/, "verification verified checks 验证 检查"],
  [/^(workflow-progress-|count-bars$)/, "progress completion 进度 完成"],
  [/^workflow-tree/, "tasks hierarchy tree 任务 层级"],
  [/^(text-input-|native-composer|composer$)/, "input edit editable typing 输入 编辑"],
  [/^form$/, "forms fields validation 表单 字段 验证"],
  [/^(picker-|mode-picker$|status-picker$|view-commands$|view-models)/, "search filter select choices 搜索 筛选 选择"],
  [/^native-composer-rich-search$/, "search find 搜索 查找"],
  [/^list/, "selection rows scroll 选择 列表 滚动"],
  [/^fish-school$/, "fish fishes swimming 鱼 鱼群"],
  [/^jellyfish$/, "jelly jellyfish 水母"],
  [/^bubble-field$/, "bubbles 气泡"],
  [/^habitat-(scene|ascii|reduced)$/, "fish jellyfish bubbles aquarium 鱼 水母 气泡"],
  [/^whale-/, "whales mascot character 鲸鱼 角色"],
  [/^whale-needs$/, "attention waiting needs input 关注 等待 输入"],
  [/^workbar-/, "dock panel 工作栏 面板"],
  [/^(transcript-|message$)/, "conversation messages prose 对话 消息"],
  [/^pending-/, "queue queued attachments steering 队列 附件"],
  [/^diff/, "changes patch review 差异 变更 审查"],
  [/^approval-/, "permission decision approve 审批 权限"],
  [/^receipt-/, "results cost usage 结果 费用 用量"],
  [/^toasts/, "notifications feedback 通知 反馈"],
];

export function discoveryText(entry: CatalogueEntry): string {
  const aliases = ALIASES.filter(([pattern]) => pattern.test(entry.name)).map(([, text]) => text);
  return normalizeSearch([entry.name, entry.title, ...displayApi(entry), ...aliases].join(" "));
}

export function getGuidance(entry: CatalogueEntry, locale: string): {
  title: string;
  description: string;
  hostNote: string;
  example: string;
  recipeId?: string;
  guideAnchor?: string;
} {
  const guidance = FAMILY_GUIDANCE[entry.family];
  let recipeId: string | undefined;
  if (entry.family === "native-chrome" && entry.api.includes("NativeComposer")) recipeId = "composer";
  if (entry.family === "workbar" && entry.api.includes("Workbar")) recipeId = "workbar";
  if (entry.family === "water" && entry.api.includes("OceanColumn")) recipeId = "ocean";
  if (entry.family === "whales" && entry.api.includes("Whale")) recipeId = "whale";
  if (entry.name === "whale-actions") recipeId = "animated-whale";
  return {
    title: entry.title,
    description: guidance ? pickText(guidance.purpose, locale) : entry.description,
    hostNote: guidance ? pickText(guidance.responsibility, locale) : pickText({
      en: "Your app supplies state and handles input; components render into your terminal.",
      zh: "应用提供状态并处理输入；组件渲染到你的终端。",
    }, locale),
    example: guidance?.example ?? "examples/gallery.rs",
    guideAnchor: ["motion", "habitat", "whales", "whale-actions"].includes(entry.family)
      ? "bring-the-whale-and-water-to-life"
      : ["tui-palettes", "water", "foundation"].includes(entry.family)
        ? "use-the-native-colors-and-background"
        : ["native-chrome", "input", "chrome", "workbar"].includes(entry.family)
          ? "connect-input-to-your-state" : "choose-the-parts-you-need",
    ...(recipeId ? { recipeId } : {}),
  };
}

export function filterByTask(entries: readonly CatalogueEntry[], taskId: string): CatalogueEntry[] {
  const task = TASKS.find((candidate) => candidate.id === taskId);
  return task ? entries.filter((entry) => task.families.includes(entry.family)) : [...entries];
}

/**
 * The exporter reads API symbols from gallery fixture code, where the studio
 * sections share one private frame: every showcase entry reports only the
 * whale-motion types that happen to appear bare. Curate what each composed
 * scene actually presents (verified against src/gallery/showcase.rs).
 */
const ENTRY_API: Record<string, string[]> = {
  "showcase-work": ["TerminalShell", "Message", "NativeComposer", "PostureBar", "MetricsLine", "Workbar", "OceanColumn"],
  "showcase-decision": ["TerminalShell", "ApprovalCard", "Diff", "Form", "Toggle"],
  "showcase-color": ["TuiPalette", "Ombre", "Segmented", "HorizonRule"],
  "showcase-life": ["Whale", "WhaleState", "Habitat", "Picker"],
  "showcase-narrow": ["TerminalShell", "Message", "NativeComposer", "PostureBar", "MetricsLine", "Workbar", "OceanColumn"],
};

export function displayApi(entry: CatalogueEntry): string[] {
  return ENTRY_API[entry.name] ?? entry.api;
}
