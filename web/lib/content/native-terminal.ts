import type { LocalizedText } from "./vocabulary";
import { pickText } from "@/lib/i18n/dictionaries";

/** Labels describe the captured application views; terminal contents stay verbatim. */
export const NATIVE_TERMINAL_VIEWS = {
  home: {
    label: { en: "Home", zh: "首页" },
    description: {
      en: "A new session, with the composer and status line.",
      zh: "新会话界面，包含输入框与状态栏。",
    },
  },
  composer: {
    label: { en: "Composer", zh: "输入框" },
    description: {
      en: "A message in the composer, before it is sent.",
      zh: "输入框中的消息，尚未发送。",
    },
  },
  workbar: {
    label: { en: "Workbar", zh: "工作栏" },
    description: {
      en: "The session dock, with tasks and other work views.",
      zh: "会话工作栏，包含任务与其他工作视图。",
    },
  },
  "workbar-fleet": {
    label: { en: "Fleet", zh: "智能体团队" },
    description: {
      en: "The Fleet workbar in a new session.",
      zh: "新会话中的智能体团队工作栏。",
    },
  },
  "provider-picker": {
    label: { en: "Providers", zh: "提供商" },
    description: {
      en: "Choose a provider for your model connection.",
      zh: "为模型连接选择提供商。",
    },
  },
  help: {
    label: { en: "Help", zh: "帮助" },
    description: {
      en: "Commands and keyboard shortcuts in the help view.",
      zh: "帮助视图中的命令与快捷键。",
    },
  },
} satisfies Record<string, { label: LocalizedText; description: LocalizedText }>;

export const NATIVE_TERMINAL_COPY = {
  title: { en: "Inside the terminal", zh: "走进终端" },
  description: {
    en: "Explore Codewhale’s composer, workbar, provider picker and help.",
    zh: "探索 Codewhale 的输入框、工作栏、提供商选择与帮助视图。",
  },
  viewsLabel: { en: "Terminal views", zh: "终端视图" },
  scrollHint: { en: "Scroll sideways to see the full terminal.", zh: "左右滚动，查看完整终端。" },
  componentsLink: { en: "Build with these components", zh: "使用这些组件构建应用" },
} satisfies Record<string, LocalizedText>;

export function getNativeTerminalCopy(locale: string) {
  return {
    title: pickText(NATIVE_TERMINAL_COPY.title, locale),
    description: pickText(NATIVE_TERMINAL_COPY.description, locale),
    viewsLabel: pickText(NATIVE_TERMINAL_COPY.viewsLabel, locale),
    scrollHint: pickText(NATIVE_TERMINAL_COPY.scrollHint, locale),
    componentsLink: pickText(NATIVE_TERMINAL_COPY.componentsLink, locale),
    views: Object.fromEntries(Object.entries(NATIVE_TERMINAL_VIEWS).map(([id, view]) => [id, {
      label: pickText(view.label, locale),
      description: pickText(view.description, locale),
    }])),
  };
}
