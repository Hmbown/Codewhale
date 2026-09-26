import type { CSSProperties } from "react";
import {
  TERMINAL_CAPTURE_FRAMES,
  type TerminalCaptureFrameId,
  type TerminalCaptureStyle,
} from "@/lib/terminal-capture.generated";

const BRAILLE = /[⠀-⣿]/;

function runStyle(style: TerminalCaptureStyle): CSSProperties {
  return {
    color: style.fg,
    backgroundColor: style.bg,
    fontWeight: style.bold ? 700 : undefined,
    fontStyle: style.italic ? "italic" : undefined,
    opacity: style.dim ? 0.6 : undefined,
    textDecoration: style.underline ? "underline" : style.strike ? "line-through" : undefined,
  };
}

/**
 * A real terminal frame as live HTML text: the PTY cells the terminal
 * emulator parsed (lib/terminal-captures, compacted by
 * scripts/render-terminal-capture.mjs), drawn as styled runs in the system
 * monospace font. Nothing is retouched or staged.
 *
 * The frame is 100 columns wide, so on a narrow screen it scrolls sideways
 * inside its own focusable region instead of shrinking past legibility.
 * Assistive technology gets one summary (`label`); the glyph-by-glyph grid
 * is presentation. Braille cells (the whale mark) are pinned to one column
 * each, because system monospace fonts draw them from a fallback font.
 */
export function TerminalCapture({
  frame = "home",
  label,
  regionLabel,
}: {
  frame?: TerminalCaptureFrameId;
  label: string;
  regionLabel: string;
}) {
  const data = TERMINAL_CAPTURE_FRAMES[frame];
  const styles = data.styles as readonly TerminalCaptureStyle[];
  // The padding around the grid continues the frame's own top and bottom rows.
  const top = styles[data.lines[0][0][1]].bg;
  const bottom = styles[data.lines[data.lines.length - 1][0][1]].bg;
  return (
    <div className="term-capture-scroll" role="region" aria-label={regionLabel} tabIndex={0}>
      <pre
        className="term-capture"
        role="img"
        aria-label={label}
        data-capture-file={data.file}
        style={{ background: `linear-gradient(${top}, ${bottom})` }}
      >
        {data.lines.map((runs, row) => (
          <span key={row} className="term-capture-row">
            {runs.map(([text, index], run) => (
              <span key={run} style={runStyle(styles[index])}>
                {BRAILLE.test(text)
                  ? [...text].map((ch, i) =>
                      BRAILLE.test(ch) ? (
                        <span key={i} className="term-capture-cell">
                          {ch}
                        </span>
                      ) : (
                        ch
                      ),
                    )
                  : text}
              </span>
            ))}
          </span>
        ))}
      </pre>
    </div>
  );
}
