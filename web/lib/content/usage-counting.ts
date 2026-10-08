/**
 * usage-counting.ts — the website's own explanation of anonymous usage
 * counting, shown on the privacy page beside the opt-out control.
 *
 * Counting is on by default. The copy names exactly what is counted, where
 * it goes (Codewhale's own endpoint, which may pass aggregate counts to
 * PostHog as a processor), what never leaves the browser, and how to turn it
 * off. It never claims that the person agreed to anything.
 */

import type { LocalizedText } from "./vocabulary";

export const USAGE_COUNTING_COPY = {
  footerLink: { en: "Usage data", zh: "使用数据" },
  heading: { en: "Usage counting on this site", zh: "本站的使用统计" },
  summary: {
    en: "This site counts page views, documentation views, install-command copies, sign-in and sign-up link clicks, and error pages shown as plain totals. Counting is on by default. Only those totals leave your browser — no page addresses, referrers, account, or content — sent to Codewhale's own endpoint, which may pass them to PostHog. A random install id, unrelated to you, rotates every 90 days.",
    zh: "本站会把页面浏览、文档浏览、安装命令复制、登录和注册链接点击以及错误页面显示次数作为纯总数统计，默认开启。离开浏览器的只有这些总数——没有页面地址、来源、账户或内容——发送到 Codewhale 自己的端点，该端点可能把总数交给 PostHog 处理。与你无关的随机安装 id 每 90 天轮换一次。",
  },
  choice: {
    en: "Turning it off is kept in this browser and clears any queued counts and the install id. Nothing here records that you accepted anything.",
    zh: "关闭后的选择保存在此浏览器中，并会清除已排队的计数和安装 id。这里不会记录你同意了任何内容。",
  },
  status: {
    default: { en: "Anonymous usage counting is on (default).", zh: "匿名使用统计已开启（默认）。" },
    on: { en: "Anonymous usage counting is on.", zh: "匿名使用统计已开启。" },
    off: { en: "Anonymous usage counting is off for this browser.", zh: "此浏览器已关闭匿名使用统计。" },
    unavailable: { en: "This browser has no site storage available, so no counts are kept and there is nothing to turn off.", zh: "此浏览器没有可用的站点存储，因此不会保留任何计数，也没有可关闭的项。" },
  },
  turnOff: { en: "Turn off", zh: "关闭" },
  turnOn: { en: "Turn on", zh: "开启" },
  elsewhere: {
    en: "In the Codewhale app, use Settings → Usage data. In the terminal runtime, run codewhale config set telemetry false or set CODEWHALE_TELEMETRY=0.",
    zh: "在 Codewhale 应用中，使用设置 → 使用数据。在终端运行时中，执行 codewhale config set telemetry false 或设置 CODEWHALE_TELEMETRY=0。",
  },
} as const satisfies Record<string, LocalizedText | Record<string, LocalizedText>>;
