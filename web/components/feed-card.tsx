import Link from "next/link";
import type { FeedItem } from "@/lib/types";
import { relativeTime } from "@/lib/github";
import { Status, type StatusTone } from "./status-badge";

const KIND_LABEL: Record<FeedItem["kind"], string> = {
  issue: "Issue",
  pull: "Pull request",
  release: "Release",
  discussion: "Discussion",
};

// State is a mark and a word; the word is GitHub's own state.
const STATE_TONE: Record<FeedItem["state"], StatusTone> = {
  open: "ready",
  closed: "idle",
  merged: "accent",
  draft: "idle",
  published: "ready",
};

/** One GitHub item: its state, title, labels and who opened it. */
export function FeedCard({ item }: { item: FeedItem; dense?: boolean }) {
  return (
    <article className="feed-card">
      <div className="feed-card-meta">
        <Status tone={STATE_TONE[item.state]}>{item.state}</Status>
        <span>{KIND_LABEL[item.kind]}</span>
        <span className="tabular">#{item.number}</span>
        <span className="feed-card-time tabular">{relativeTime(item.updatedAt)}</span>
      </div>
      <h3 className="feed-card-title">
        <Link href={item.url}>{item.title}</Link>
      </h3>
      <div className="feed-card-foot">
        {item.labels.slice(0, 3).map((l) => (
          <span key={l.name} className="pill">{l.name}</span>
        ))}
        <span className="feed-card-author">
          @{item.author}
          {item.comments > 0 && <span className="tabular"> · {item.comments} {item.comments === 1 ? "reply" : "replies"}</span>}
        </span>
      </div>
    </article>
  );
}
