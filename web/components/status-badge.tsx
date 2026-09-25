/**
 * State is a mark and a word, never colour alone.
 *
 * <Status> is the general form: a dot in the tone's colour and the word in
 * text. `live` lights its dot (bioluminescence) and is only for something
 * the site actually observes; nothing static qualifies.
 *
 * <StatusBadge> keeps the older availability vocabulary (experimental,
 * preview, pending, unavailable) for the pages that still use it.
 */

import type { ReactNode } from "react";
import type { LocalizedText } from "@/lib/content/vocabulary";
import { pickText } from "@/lib/i18n/dictionaries";

export type StatusTone = "ready" | "attention" | "idle" | "live" | "danger" | "accent";

export function Status({ tone, children }: { tone: StatusTone; children: ReactNode }) {
  return (
    <span className={`status status-${tone}`}>
      <span className="status-dot" aria-hidden="true" />
      {children}
    </span>
  );
}

export type StatusKind = "experimental" | "preview" | "pending" | "unavailable";

const DEFAULT_LABELS: Record<StatusKind, LocalizedText> = {
  experimental: { en: "Experimental", zh: "实验性" },
  preview: { en: "Preview", zh: "预览" },
  pending: { en: "Pending", zh: "待就绪" },
  unavailable: { en: "Unavailable", zh: "暂不可用" },
};

export function StatusBadge({
  kind,
  locale = "en",
  label,
}: {
  kind: StatusKind;
  locale?: string;
  /** Overrides the default per-kind label (e.g. a media entry's pendingLabel). */
  label?: LocalizedText;
}) {
  const text = pickText(label ?? DEFAULT_LABELS[kind], locale);

  return (
    <span className={`status-badge status-badge-${kind}`}>
      <span className="status-badge-dot" aria-hidden="true" />
      {text}
    </span>
  );
}
