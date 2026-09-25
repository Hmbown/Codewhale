import { Icon } from "./icon";
/**
 * "See how it decides" — a terminal-styled pane that surfaces REAL reasoning
 * traces from a Codewhale session, paired with the decision each produced.
 *
 * The point is "show, don't tell": every agent claims to be aligned/trustworthy;
 * Codewhale can prove it, because the Constitution is observable in the model's
 * reasoning (it cites "Article II", "Article V", etc. as it decides). No other
 * agent can show this because none have a hierarchy the model reasons against.
 *
 * The traces below are faithful excerpts from an actual session — not invented
 * marketing copy. Keep them honest if you edit them (see AGENTS.md Article II).
 */

export type Scene = {
  /** Short tab label for players that show one scene at a time. */
  tab: { en: string; zh: string };
  context: { en: string; zh: string };
  trace: string;
  cites: string[];
  decision: { en: string; zh: string };
};

export const SCENES: Scene[] = [
  {
    tab: { en: "Stale handoff", zh: "过期交接" },
    context: {
      en: "When the prior session's summary didn't match the repo…",
      zh: "当上一会话的总结与仓库真实状态不符时……",
    },
    trace:
      "Per Article II (truth) and V (verification), I report what's actually in the tree — not what the previous agent claimed.",
    cites: ["Article II · Truth", "Article V · Verification"],
    decision: {
      en: "Reported the uncommitted work the summary left out, instead of repeating the claim.",
      zh: "如实报告了总结遗漏的未提交改动，而不是照搬旧说法。",
    },
  },
  {
    tab: { en: "Conflicting orders", zh: "指令冲突" },
    context: {
      en: "When a “pause” signal conflicted with the user's request to continue…",
      zh: "当「暂停」信号与用户要求继续相冲突时……",
    },
    trace:
      "The user's current request outranks the runtime statute — so I keep executing the task, not stopping on a technicality.",
    cites: ["Article III · User agency", "Article VII · Hierarchy"],
    decision: {
      en: "Finished the work the user actually asked for.",
      zh: "完成了用户真正要求的工作。",
    },
  },
  {
    tab: { en: "Community credit", zh: "社区署名" },
    context: {
      en: "When trimming the README, tempted to drop the contributor wall…",
      zh: "精简 README 时，本可删掉贡献者名单……",
    },
    trace:
      "Deleting community credit would break the project's own ethos. Relocate it instead — zero names lost.",
    cites: ["Community ethos"],
    decision: {
      en: "Moved 200 lines of credits to docs/CONTRIBUTORS.md and linked from the README.",
      zh: "把 200 行贡献记录迁到 docs/CONTRIBUTORS.md，并在 README 中给出链接。",
    },
  },
];

export function ThinkingTrace({ locale = "en" }: { locale?: string }) {
  const isZh = locale === "zh";
  return (
    <div className="grid-3">
      {SCENES.map((s, i) => (
        <figure key={i} className="trace">
          <figcaption className="trace-head">
            <span className="status status-accent">
              <span className="status-dot" aria-hidden="true" />
              {isZh ? "推理痕迹" : "Reasoning trace"}
            </span>
            <span className="trace-context">{isZh ? s.context.zh : s.context.en}</span>
          </figcaption>
          <pre className="trace-body">{s.trace}</pre>
          <p className="trace-cites">
            {s.cites.map((c) => (
              <span key={c} className="pill">{c}</span>
            ))}
          </p>
          <p className="trace-decision">
            <Icon name="arrow-right" className="icon icon-flip" />
            <span>{isZh ? s.decision.zh : s.decision.en}</span>
          </p>
        </figure>
      ))}
    </div>
  );
}
