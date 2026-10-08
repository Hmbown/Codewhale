import type { LegalPrivacyDict } from "../types";

/**
 * Chinese dictionary for `app/[locale]/legal/privacy/page.tsx`. Only the page
 * chrome is translated; the terms themselves are binding in English, which is
 * what `updated` tells the reader.
 *
 * Spacing note: the space between `于` and `{date}` came from the literal
 * space between two JSX expressions, and there is none before `。` because
 * JSX dropped the newline-only whitespace that separated them. Do not "fix"
 * either as a typo.
 */
export const legalPrivacy: LegalPrivacyDict = {
  metaTitle: "隐私政策 · Codewhale",
  metaDescription: "Shannon Labs 如何在你使用 Codewhale 时处理信息。",
  kicker: "法律",
  title: "隐私政策",
  updated: "生效并最近更新于 {date}。以下为具有约束力的英文文本。",
  termsLink: "服务条款",
  homeLink: "返回首页",
};
