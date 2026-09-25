import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { buildInstallGuide, installAnchorErrors, renderInstallGuideModule } from "../scripts/install-guide-lib.mjs";
import { INSTALL_GUIDE } from "./install-guide.generated";

describe("canonical installation guide", () => {
  it("ships the entire current guide, including the untranslated-document notice", () => {
    const source = readFileSync(new URL("../../docs/INSTALL.md", import.meta.url), "utf8");
    expect(readFileSync(new URL("./install-guide.generated.ts", import.meta.url), "utf8"))
      .toBe(renderInstallGuideModule(source));
    expect(INSTALL_GUIDE.chunks.map((chunk) => chunk.text).join("\n"))
      .toContain("not yet updated for this revision");
  });

  it("keeps top-level command bytes intact and nested code inside its list", () => {
    const guide = buildInstallGuide('# Setup\n\n```sh\nprintf "$HOME & <path>"\n```\n\n1. In a list:\n\n   ```sh\n   echo nested\n   ```\n');
    expect(guide.chunks[1]).toEqual({ kind: "code", text: 'printf "$HOME & <path>"' });
    expect(guide.chunks[2].text).toMatch(/<ol>[\s\S]*<li>[\s\S]*<pre><code[\s\S]*echo nested[\s\S]*<\/li>[\s\S]*<\/ol>/);
  });

  it("resolves relative docs and preserves duplicate-heading and legacy anchors", () => {
    const guide = buildInstallGuide('# Start\n\n## Notes\n\n## Notes\n\n<a id="legacy"></a>\n\n[Keys](KEYBINDINGS.md) [Notes](INSTALL.md#notes-1) [Old](#legacy)\n');
    expect(guide.anchors).toEqual(["start", "notes", "notes-1", "legacy"]);
    expect(guide.chunks[0].text).toContain('href="https://github.com/Hmbown/CodeWhale/blob/main/docs/KEYBINDINGS.md"');
    expect(guide.chunks[0].text).toContain('href="#notes-1"');
    expect(installAnchorErrors("INSTALL.md#notes-1 INSTALL.md#missing", guide.anchors)).toEqual(["missing"]);
  });

  it("fails generation on a broken internal link", () => {
    expect(() => buildInstallGuide("# Setup\n\n[Missing](#gone)"))
      .toThrow("Missing INSTALL.md anchors: gone");
  });

  it("gives scrollable tables distinct, escaped names from their headers", () => {
    const table = '| "Need" | Reason |\n| --- | --- |\n| curl | Download |\n';
    const guide = buildInstallGuide(`# Setup\n\n${table}\n${table}`);
    const html = guide.chunks.map((chunk) => chunk.text).join("\n");
    const names = [...html.matchAll(/role="region" aria-label="([^"]+)"/g)].map((match) => match[1]);
    expect(names).toEqual([
      "Installation table 1: &quot;Need&quot; / Reason",
      "Installation table 2: &quot;Need&quot; / Reason",
    ]);
  });

  it.each(["javascript:alert(1)", "data:text/html,hello", "file:///tmp/private"])("rejects unsafe URL %s", (href) => {
    expect(() => buildInstallGuide(`# Setup\n\n[link](${href})`)).toThrow("Unsupported install-guide link protocol");
  });

  it("rejects arbitrary raw HTML instead of publishing it", () => {
    expect(() => buildInstallGuide('# Setup\n\n<img src="x" onerror="alert(1)">'))
      .toThrow("Unsupported raw HTML");
  });
});
