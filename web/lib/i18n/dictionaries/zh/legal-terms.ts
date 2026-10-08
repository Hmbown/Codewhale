import type { LegalTermsDict } from "../types";

/**
 * Chinese dictionary for `app/[locale]/legal/terms/page.tsx`. Only the page
 * chrome is translated; the terms themselves are binding in English, which is
 * what `updated` tells the reader.
 *
 * Spacing note: the space between `于` and `{date}` came from the literal
 * space between two JSX expressions, and there is none before `。` because
 * JSX dropped the newline-only whitespace that separated them. Do not "fix"
 * either as a typo.
 */
export const legalTerms: LegalTermsDict = {
  metaTitle: "服务条款 · Codewhale",
  metaDescription: "Shannon Labs 产品 Codewhale 的服务条款。",
  kicker: "法律",
  title: "服务条款",
  updated: "生效并最近更新于 {date}。以下为具有约束力的英文文本。",
  privacyLink: "隐私政策",
  homeLink: "返回首页",
};
