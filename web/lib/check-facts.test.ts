/**
 * Tests for check-facts.mjs via its re-importable logic.
 *
 * We test the diffFacts helper (ported from check-facts.mjs) against
 * fixture-like objects to prove the stale-version detection works.
 * End-to-end `node scripts/check-facts.mjs` exit-code tests are not run
 * from vitest since they depend on the actual workspace file tree.
 */
import { describe, it, expect } from "vitest";

// --- inline diffFacts (same logic as check-facts.mjs) ----------------

interface ProviderFact {
  id: string;
  label: string;
  env: string;
}

interface PublishedReleaseFact {
  tag: string;
  version: string;
  publishedAt: string;
  url: string;
}

interface ModelFact {
  id: string;
  provider: string | null;
  contextWindow: number | null;
  maxOutput: number | null;
  reasoning: boolean;
  addedAt: string | null;
}

interface RepoFacts {
  [key: string]: unknown;
  generatedAt: string;
  sourceRevision: string | null;
  sourceCommittedAt: string | null;
  version: string | null;
  crates: string[];
  sandboxBackends: string[];
  providers: ProviderFact[];
  models: ModelFact[];
  defaultModel: string | null;
  nodeEngines: string | null;
  toolCount: number | null;
  license: string | null;
  latestPublishedRelease: PublishedReleaseFact | null;
}

function diffFacts(
  committed: Record<string, unknown>,
  fresh: Record<string, unknown>,
): Array<{ field: string; committed: unknown; fresh: unknown }> {
  const checkFields = [
    "version",
    "crates",
    "sandboxBackends",
    "providers",
    "models",
    "defaultModel",
    "nodeEngines",
    "toolCount",
    "license",
    "latestPublishedRelease",
  ];
  const diffs: Array<{ field: string; committed: unknown; fresh: unknown }> = [];
  for (const field of checkFields) {
    const a = JSON.stringify(committed[field] ?? null);
    const b = JSON.stringify(fresh[field] ?? null);
    if (a !== b) {
      diffs.push({ field, committed: committed[field], fresh: fresh[field] });
    }
  }
  return diffs;
}

// --- helpers ---------------------------------------------------------

function freshFacts(overrides: Partial<RepoFacts> = {}): RepoFacts {
  return {
    generatedAt: new Date().toISOString(),
    sourceRevision: null,
    sourceCommittedAt: null,
    version: "0.8.64",
    crates: ["cli", "config", "tui"],
    sandboxBackends: [
      "seatbelt (macOS, when available)",
      "bubblewrap (Linux, opt-in when installed)",
    ],
    providers: [
      { id: "deepseek", label: "DeepSeek", env: "DEEPSEEK_API_KEY" },
      { id: "anthropic", label: "Anthropic", env: "ANTHROPIC_API_KEY" },
    ],
    models: [
      {
        id: "deepseek-v4-pro",
        provider: "DeepSeek",
        contextWindow: 1000000,
        maxOutput: 128000,
        reasoning: true,
        addedAt: "2026-07-01",
      },
    ],
    defaultModel: "deepseek-v4-pro",
    nodeEngines: ">=18",
    toolCount: 78,
    license: "MIT",
    latestPublishedRelease: {
      tag: "v0.8.63",
      version: "0.8.63",
      publishedAt: "2026-06-01T00:00:00Z",
      url: "https://github.com/Hmbown/CodeWhale/releases/tag/v0.8.63",
    },
    ...overrides,
  };
}

// --- tests -----------------------------------------------------------

describe("diffFacts (check-facts parity)", () => {
  it("returns empty array when facts match", () => {
    const committed = freshFacts();
    const fresh = freshFacts();
    expect(diffFacts(committed, fresh)).toEqual([]);
  });

  it("detects stale version (0.8.62 vs 0.8.64)", () => {
    const committed = freshFacts({ version: "0.8.62" });
    const fresh = freshFacts({ version: "0.8.64" });
    const diffs = diffFacts(committed, fresh);
    expect(diffs).toHaveLength(1);
    expect(diffs[0]).toEqual({
      field: "version",
      committed: "0.8.62",
      fresh: "0.8.64",
    });
  });

  it("detects provider list drift (added Anthropic)", () => {
    const committed = freshFacts({
      providers: [{ id: "deepseek", label: "DeepSeek", env: "DEEPSEEK_API_KEY" }],
    });
    const fresh = freshFacts({
      providers: [
        { id: "deepseek", label: "DeepSeek", env: "DEEPSEEK_API_KEY" },
        { id: "anthropic", label: "Anthropic", env: "ANTHROPIC_API_KEY" },
      ],
    });
    const diffs = diffFacts(committed, fresh);
    expect(diffs).toHaveLength(1);
    expect(diffs[0].field).toBe("providers");
  });

  it("detects stale default model", () => {
    const committed = freshFacts({ defaultModel: "deepseek-v3" });
    const fresh = freshFacts({ defaultModel: "deepseek-v4-pro" });
    const diffs = diffFacts(committed, fresh);
    expect(diffs).toHaveLength(1);
    expect(diffs[0].field).toBe("defaultModel");
  });

  it("detects tool count drift", () => {
    const committed = freshFacts({ toolCount: 70 });
    const fresh = freshFacts({ toolCount: 78 });
    const diffs = diffFacts(committed, fresh);
    expect(diffs).toHaveLength(1);
    expect(diffs[0].field).toBe("toolCount");
  });

  it("detects multiple field drifts at once", () => {
    const committed = freshFacts({
      version: "0.8.62",
      toolCount: 70,
    });
    const fresh = freshFacts({
      version: "0.8.64",
      toolCount: 78,
    });
    const diffs = diffFacts(committed, fresh);
    expect(diffs).toHaveLength(2);
    expect(diffs.map((d) => d.field).sort()).toEqual(["toolCount", "version"]);
  });

  it("ignores generatedAt and exact-build provenance changes", () => {
    const committed = freshFacts({ generatedAt: "old" });
    const fresh = freshFacts({
      generatedAt: "new",
      sourceRevision: "a".repeat(40),
      sourceCommittedAt: "2026-07-21T22:00:00Z",
    });
    expect(diffFacts(committed, fresh)).toEqual([]);
  });

  it("detects latest-published-release drift", () => {
    const committed = freshFacts();
    const fresh = freshFacts({
      latestPublishedRelease: {
        tag: "v0.8.64",
        version: "0.8.64",
        publishedAt: "2026-06-02T00:00:00Z",
        url: "https://github.com/Hmbown/CodeWhale/releases/tag/v0.8.64",
      },
    });
    const diffs = diffFacts(committed, fresh);
    expect(diffs).toHaveLength(1);
    expect(diffs[0].field).toBe("latestPublishedRelease");
  });

  it("handles null-to-value drift for license", () => {
    const committed = freshFacts({ license: null });
    const fresh = freshFacts({ license: "MIT" });
    const diffs = diffFacts(committed, fresh);
    expect(diffs).toHaveLength(1);
    expect(diffs[0].field).toBe("license");
  });

  it("handles empty arrays in committed vs populated arrays in fresh", () => {
    const committed = freshFacts({ crates: [], providers: [] });
    const fresh = freshFacts({
      crates: ["cli"],
      providers: [{ id: "deepseek", label: "DeepSeek", env: "DEEPSEEK_API_KEY" }],
    });
    const diffs = diffFacts(committed, fresh);
    expect(diffs).toHaveLength(2);
    expect(diffs.map((d) => d.field).sort()).toEqual(["crates", "providers"]);
  });
});
