import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { TERMINAL_SCREENSHOT } from "./media-manifest";
import { TERMINAL_CAPTURE_FRAMES, TERMINAL_CAPTURE_META } from "./terminal-capture.generated";

// The homepage terminal is live text drawn from real PTY cells. These checks
// keep the generated runs tied to the capture files, cell for cell.
const webRoot = new URL("../", import.meta.url);

describe("terminal capture", () => {
  it("keeps the generated module current with the capture files", () => {
    const out = execFileSync(process.execPath, ["scripts/render-terminal-capture.mjs", "--check"], {
      cwd: webRoot,
      encoding: "utf8",
    });
    expect(out).toContain("is current");
  });

  it("keeps every frame's text identical to its captured cells", () => {
    for (const frame of Object.values(TERMINAL_CAPTURE_FRAMES)) {
      const capture = JSON.parse(readFileSync(new URL(`../../${frame.file}`, import.meta.url), "utf8")) as {
        rows: number;
        cols: number;
        cells: { text: string }[][];
      };
      expect(frame.lines).toHaveLength(capture.rows);
      frame.lines.forEach((runs, row) => {
        const text = runs.map(([t]) => t).join("");
        expect(text, `${frame.file} row ${row}`).toBe(capture.cells[row].map((c) => c.text || " ").join(""));
        expect([...text]).toHaveLength(capture.cols);
      });
    }
  });

  it("names its source and the build it came from", () => {
    expect(TERMINAL_CAPTURE_META.source).toContain("website_current_terminal_capture");
    expect(TERMINAL_CAPTURE_META.baseCommit).toBe(TERMINAL_SCREENSHOT.sourceCommit);
    expect(`web/lib/terminal-captures/website-home-100x24.json`).toBe(TERMINAL_SCREENSHOT.capture);
  });

  it("renders as one labelled image inside a keyboard-scrollable region", () => {
    const component = readFileSync(new URL("../components/terminal-capture.tsx", import.meta.url), "utf8");
    expect(component).toContain('role="region"');
    expect(component).toContain("tabIndex={0}");
    expect(component).toContain('role="img"');
    expect(component).toContain("aria-label={label}");
  });
});
