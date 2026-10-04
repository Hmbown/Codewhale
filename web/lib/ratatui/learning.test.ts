import { existsSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { readCatalogue } from "./catalogue";
import { FAMILY_GUIDANCE, TASKS, discoveryText, displayApi, filterByTask, getGuidance, normalizeSearch } from "./learning";

const catalogue = readCatalogue();
const names = new Map(catalogue.entries.map((entry) => [entry.name, entry]));
const matching = (query: string) => {
  const terms = normalizeSearch(query).trim().split(/\s+/).filter(Boolean);
  return catalogue.entries.filter((entry) => terms.every((term) => discoveryText(entry).includes(term)))
    .map((entry) => entry.name);
};

describe("Ratatui learning routes", () => {
  it("gives every generated collection exactly one intent and an existing example", () => {
    const families = TASKS.flatMap((task) => task.families);
    expect(new Set(families).size).toBe(families.length);
    expect([...families].sort()).toEqual(catalogue.families.map((family) => family.id).sort());
    expect(Object.keys(FAMILY_GUIDANCE).sort()).toEqual([...families].sort());
    for (const guidance of Object.values(FAMILY_GUIDANCE)) {
      expect(guidance.purpose.en.length).toBeGreaterThan(0);
      expect(guidance.purpose.zh.length).toBeGreaterThan(0);
      expect(guidance.responsibility.en.length).toBeGreaterThan(0);
      expect(guidance.responsibility.zh.length).toBeGreaterThan(0);
      expect(existsSync(join(process.cwd(), "..", "vendor", "codewhale-ratatui", guidance.example)), guidance.example).toBe(true);
    }
  });

  it("recommends available entries belonging to each task and keeps gallery order", () => {
    for (const task of TASKS) {
      const filtered = filterByTask(catalogue.entries, task.id);
      expect(filtered).toEqual(catalogue.entries.filter((entry) => task.families.includes(entry.family)));
      for (const name of task.recommended) {
        expect(names.has(name), name).toBe(true);
        expect(task.families).toContain(names.get(name)!.family);
      }
    }
    expect(filterByTask(catalogue.entries, "all")).toEqual(catalogue.entries);
    expect(filterByTask(catalogue.entries, "unknown")).toEqual(catalogue.entries);
  });

  it("finds themes, ombré and loading by common words without matching unrelated widgets", () => {
    expect(matching("themes")).toContain("tui-theme-underwater");
    expect(matching("ombré")).toEqual(matching("ombre"));
    expect(matching("ombré")).toContain("ocean-column");
    expect(matching("loading")).toContain("spinner");
    expect(matching("loading")).not.toContain("toasts");
    expect(matching("progress")).toContain("workflow-progress-live");
    expect(matching("progress")).not.toContain("receipt-table");
    expect(matching("progress")).not.toContain("artifact");
  });

  it("distinguishes fish, jellyfish, fields and search while retaining public API lookup", () => {
    expect(matching("jelly")).toContain("jellyfish");
    expect(matching("jelly")).not.toContain("fish-school");
    expect(matching("fish")).toContain("fish-school");
    expect(matching("forms")).toContain("form");
    expect(matching("forms")).not.toContain("text-input-typed");
    expect(matching("search")).toContain("picker-query");
    expect(matching("search")).not.toContain("whale-rest");
    expect(matching("NativeComposer")).toContain("native-composer");
    expect(matching("水母")).toContain("jellyfish");
  });

  it("offers public recipes only for matching component APIs and uses English locale fallback", () => {
    const expected = {
      "native-composer": "composer", "workbar-tasks": "workbar", "ocean-column": "ocean",
      "whale-busy": "whale", "whale-actions": "animated-whale",
    };
    for (const [name, recipe] of Object.entries(expected)) {
      expect(getGuidance(names.get(name)!, "en").recipeId).toBe(recipe);
    }
    for (const name of ["native-composer-rich-selection", "showcase-work", "atmosphere-ocean", "fish-school"]) {
      expect(getGuidance(names.get(name)!, "en").recipeId).toBeUndefined();
    }
    const whale = names.get("whale-busy")!;
    expect(getGuidance(whale, "fr")).toEqual(getGuidance(whale, "en"));
    expect(getGuidance(whale, "zh").hostNote).toContain("WhaleState");
    expect(getGuidance(whale, "zh").description).not.toBe(getGuidance(whale, "en").description);
  });

  it("shows each studio scene's composed components instead of the shared frame's imports", () => {
    expect(displayApi(names.get("showcase-work")!)).toContain("TerminalShell");
    expect(displayApi(names.get("showcase-work")!)).toContain("NativeComposer");
    expect(displayApi(names.get("showcase-decision")!)).toContain("ApprovalCard");
    expect(displayApi(names.get("showcase-color")!)).toContain("Ombre");
    expect(displayApi(names.get("showcase-life")!)).toContain("Whale");
    expect(displayApi(names.get("composer")!)).toEqual(names.get("composer")!.api);
  });
});
