/**
 * <Seal> — one Han glyph in a small square stamp, inked in the logo's ombre.
 *
 * A seal names the subject of a page or section at a glance (问 questions,
 * 接 integrations, 数 facts, 信 trust, 集 surfaces, 法 law, 序 rank, 证 evidence,
 * 动 activity, 深 depth). It is a mark, not a word: the heading beside it
 * says the same thing, so the seal is hidden from assistive technology.
 */
export function Seal({ char = "深", size = "md" }: { char?: string; size?: "sm" | "md" | "lg" }) {
  return (
    <span className={`seal seal-${size}`} aria-hidden="true" lang="zh">
      {char}
    </span>
  );
}
