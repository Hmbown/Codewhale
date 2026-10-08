/**
 * account-entry.ts — copy for the public sign-in and create-account pages.
 *
 * codewhale.net is not the signed-in app; these pages send the person to
 * app.codewhale.net while saying plainly that the terminal needs no account
 * and that an account is never a paid plan by itself.
 */

import type { LocalizedText } from "./vocabulary";

export const ACCOUNT_ENTRY_COPY = {
  signIn: {
    metaTitle: { en: "Sign in · Codewhale", zh: "登录 · Codewhale" },
    kicker: { en: "Sign in", zh: "登录" },
    title: { en: "Sign in to Codewhale.", zh: "登录 Codewhale 账户。" },
    action: { en: "Sign in", zh: "登录" },
    switchPrompt: { en: "Need an account?", zh: "还没有账户？" },
    switchLabel: { en: "Create account", zh: "创建账户" },
  },
  signUp: {
    metaTitle: { en: "Create account · Codewhale", zh: "创建账户 · Codewhale" },
    kicker: { en: "Create account", zh: "创建账户" },
    title: { en: "Create a Codewhale account.", zh: "创建 Codewhale 账户。" },
    action: { en: "Create account", zh: "创建账户" },
    switchPrompt: { en: "Already have an account?", zh: "已有账户？" },
    switchLabel: { en: "Sign in", zh: "去登录" },
  },
  lede: {
    en: "Sign in to the Codewhale app to keep your conversations and connected providers together. The web app is a development preview. To continue a terminal session in the browser, run /rc in that session. You can use the terminal without an account.",
    zh: "登录 Codewhale 应用，集中管理对话与已连接的提供商。网页应用目前为开发预览版。要在浏览器中继续终端会话，请在该会话里运行 /rc。使用终端无需账户。",
  },
  metaDescription: {
    en: "Access your Codewhale account and the app’s development preview. Connect to a terminal session from your browser with /rc.",
    zh: "访问 Codewhale 账户与应用开发预览版。用 /rc 从浏览器连接终端会话。",
  },
  installLocally: { en: "Install locally", zh: "本机安装" },
} as const satisfies Record<string, unknown> & { lede: LocalizedText; installLocally: LocalizedText };
