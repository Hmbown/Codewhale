"use client";

import { useId, useState } from "react";
import Link from "next/link";
import { Icon } from "@/components/icon";
import { TerminalCapture } from "@/components/terminal-capture";
import { getNativeTerminalCopy, NATIVE_TERMINAL_VIEWS } from "@/lib/content/native-terminal";
import { TERMINAL_CAPTURE_FRAMES, type TerminalCaptureFrameId } from "@/lib/terminal-capture.generated";
import "./native-terminal-gallery.css";

function hasCapture(id: string): id is TerminalCaptureFrameId {
  return Object.prototype.hasOwnProperty.call(TERMINAL_CAPTURE_FRAMES, id);
}

// New captured views appear when their real frame is added to the generated
// module. A named view without a capture never becomes a website button.
const views = Object.keys(NATIVE_TERMINAL_VIEWS).flatMap((id) =>
  hasCapture(id) ? [{ id }] : [],
);

/** Website controls select real PTY captures; the terminal itself is unchanged. */
export function NativeTerminalGallery({
  locale,
  defaultFrame = "home",
  label,
  regionLabel,
}: {
  locale: string;
  defaultFrame?: TerminalCaptureFrameId;
  label: string;
  regionLabel: string;
}) {
  const [frame, setFrame] = useState<TerminalCaptureFrameId>(defaultFrame);
  const captureId = useId();
  const selected = views.find((view) => view.id === frame) ?? views[0];
  if (!selected) return null;
  const copy = getNativeTerminalCopy(locale);
  const { label: viewLabel, description } = copy.views[selected.id];

  return (
    <div className="native-terminal-gallery">
      <div className="native-terminal-controls" role="group" aria-label={copy.viewsLabel}>
        {views.map((view) => (
          <button
            key={view.id}
            type="button"
            className="native-terminal-button"
            aria-pressed={selected.id === view.id}
            aria-controls={captureId}
            onClick={() => setFrame(view.id)}
          >
            {copy.views[view.id].label}
          </button>
        ))}
      </div>
      <p className="native-terminal-description" aria-live="polite" aria-atomic="true">{description}</p>
      <div className="native-terminal-view" id={captureId} dir="ltr">
        <TerminalCapture
          frame={selected.id}
          label={selected.id === "home" ? label : `${viewLabel}. ${description}`}
          regionLabel={`${regionLabel} · ${viewLabel}`}
        />
      </div>
      <div className="native-terminal-footer">
        <span className="native-terminal-scroll-hint">{copy.scrollHint}</span>
        <Link href={`/${locale}/ratatui`} className="native-terminal-components-link">
          {copy.componentsLink}
          <Icon name="arrow-right" className="icon icon-flip" />
        </Link>
      </div>
    </div>
  );
}
