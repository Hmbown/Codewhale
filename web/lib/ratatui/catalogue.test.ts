import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { motionId, previewAssetPath, readCatalogue, searchEntries } from "./catalogue";
import { discoveryText } from "./learning";

const catalogue = readCatalogue();
const publicRoot = join(process.cwd(), "public", "ratatui");

describe("Ratatui explorer", () => {
  it("finds the base composer by its public API alongside its responsive variants", () => {
    const found = searchEntries(catalogue.entries, "NativeComposer");
    expect(found.map((entry) => entry.name)).toContain("native-composer");
    expect(found.map((entry) => entry.name)).toContain("native-composer-narrow");
    expect(searchEntries(catalogue.entries, "native composer narrow").map((entry) => entry.name))
      .toContain("native-composer-narrow");
  });

  it("combines search and collection selection and leaves an empty search recoverable", () => {
    expect(searchEntries(catalogue.entries, "fish", "habitat").map((entry) => entry.name)).toContain("fish-school");
    expect(searchEntries(catalogue.entries, "fish", "input")).toHaveLength(0);
    expect(searchEntries(catalogue.entries, "" )).toHaveLength(catalogue.entries.length);
  });

  it("finds studio scenes by their displayed composed APIs instead of private fixture inputs", () => {
    const studio = catalogue.entries.filter((entry) => entry.family === "studio");
    const shells = searchEntries(studio, "TerminalShell").map((entry) => entry.name);
    expect(shells).toHaveLength(3);
    expect(shells).toEqual(expect.arrayContaining(["showcase-work", "showcase-decision", "showcase-narrow"]));
    expect(searchEntries(studio, "NativeComposer").map((entry) => entry.name))
      .toEqual(expect.arrayContaining(["showcase-work", "showcase-narrow"]));
    expect(searchEntries(studio, "ApprovalCard").map((entry) => entry.name)).toEqual(["showcase-decision"]);
    expect(searchEntries(studio, "Inputs")).toHaveLength(0);
    for (const name of ["showcase-work", "showcase-decision", "showcase-narrow"]) {
      const text = discoveryText(studio.find((entry) => entry.name === name)!);
      expect(text).toContain("terminalshell");
      expect(text).not.toContain("inputs");
    }
  });

  it("keeps the hub hero entry available with its wide and narrow renders", () => {
    const hero = catalogue.entries.find((entry) => entry.name === "showcase-work");
    expect(hero).toBeDefined();
    const data = JSON.parse(readFileSync(join(publicRoot, hero!.previewPath), "utf8"));
    expect(data.previews["dark-truecolor"]["native"]).toContain("viewBox=");
    expect(data.previews["dark-truecolor"]["40"]).toContain('viewBox="0 0 400 ');
  });

  it("rejects an asset path outside the generated component directory", () => {
    const entry = catalogue.entries[0];
    expect(previewAssetPath(entry)).toBe(`/ratatui/entries/${entry.name}.json`);
    expect(() => previewAssetPath({ ...entry, previewPath: "entries/../../secret.json" })).toThrow();
  });

  it("provides every advertised profile and actual column width for every component", () => {
    const names = new Set(catalogue.entries.map((entry) => entry.name));
    expect(names.size).toBe(catalogue.entries.length);
    expect(catalogue.profiles).toHaveLength(9);
    expect(catalogue.sizes.map((size) => size.id)).toEqual(["native", "40", "80", "120"]);
    let buffers = 0;
    for (const entry of catalogue.entries) {
      expect(entry.api.length, entry.name).toBeGreaterThan(0);
      expect(entry.source.url).toContain(`/blob/${catalogue.source.revision}/src/gallery/`);
      const data = JSON.parse(readFileSync(join(publicRoot, entry.previewPath), "utf8"));
      expect(data.name).toBe(entry.name);
      expect(data.fixture.standalone).toBe(false);
      expect(data.fixture.code.length).toBeGreaterThan(0);
      for (const profile of catalogue.profiles) for (const size of catalogue.sizes) {
        const svg = data.previews[profile.id][size.id];
        const width = size.id === "native" ? entry.width : Number(size.id);
        expect(svg).toContain(`viewBox="0 0 ${width * 10} ${entry.height * 20}"`);
        expect(svg).not.toMatch(/<script|foreignObject|https?:\/\/[^\s"']+\.(?:png|js|woff)/i);
        buffers += 1;
      }
    }
    expect(buffers).toBe(catalogue.entries.length * 9 * 4);
    expect(catalogue.families.reduce((sum, family) => sum + family.count, 0)).toBe(catalogue.entries.length);
  });

  it("loads native motion with its real cadence and preserves the static accessibility profiles", () => {
    for (const entry of catalogue.entries) {
      const id = motionId(entry, "dark-truecolor");
      if (!id) continue;
      const data = JSON.parse(readFileSync(join(publicRoot, "motion", `${id}.json`), "utf8"));
      expect(data.frames.length).toBeGreaterThan(1);
      expect(data.frameMs).toBeGreaterThanOrEqual(20);
      expect(data.source).toMatch(/^https:\/\/github\.com\/Hmbown\/codewhale-ratatui\/blob\/[a-f0-9]{40}\//);
    }
    expect(motionId(catalogue.entries.find((entry) => entry.name === "showcase-work")!, "light-truecolor")).toBe("studio-light");
    expect(catalogue.profiles.map((profile) => profile.id)).toEqual(expect.arrayContaining(["ascii", "no-color", "ansi-16"]));
  });
});
