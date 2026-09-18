import { describe, expect, it } from "vitest";
import { resolveFacts, type RepoFacts } from "./facts";

const release090 = {
  tag: "v0.9.0",
  version: "0.9.0",
  publishedAt: "2026-07-16T20:05:39Z",
  url: "https://github.com/Hmbown/CodeWhale/releases/tag/v0.9.0",
};

function snapshot(overrides: Partial<RepoFacts> = {}): RepoFacts {
  return {
    generatedAt: "2026-07-21T22:00:00Z",
    sourceRevision: "a".repeat(40),
    sourceCommittedAt: "2026-07-21T22:00:00Z",
    version: "0.9.1",
    crates: ["cli", "tui"],
    sandboxBackends: ["seatbelt (macOS)"],
    providers: [{ id: "deepseek", label: "DeepSeek", env: "DEEPSEEK_API_KEY" }],
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
    toolCount: 66,
    license: "MIT",
    latestPublishedRelease: release090,
    ...overrides,
  };
}

describe("resolveFacts", () => {
  it("keeps newer build facts over an older KV snapshot", () => {
    const build = snapshot();
    const cached = snapshot({
      sourceRevision: "b".repeat(40),
      sourceCommittedAt: "2026-07-21T21:00:00Z",
      version: "0.8.67",
      toolCount: 60,
    });

    const result = resolveFacts(build, cached);

    expect(result.source).toBe("build");
    expect(result.reason).toBe("build-source-newer-or-ambiguous");
    expect(result.facts.version).toBe("0.9.1");
    expect(result.facts.toolCount).toBe(66);
  });

  it("rejects a legacy KV snapshot with no source provenance", () => {
    const build = snapshot();
    const cached = snapshot({
      sourceRevision: null,
      sourceCommittedAt: null,
      version: "0.8.67",
    });

    const result = resolveFacts(build, cached);

    expect(result.source).toBe("build");
    expect(result.reason).toBe("kv-missing-source-provenance");
    expect(result.facts.version).toBe("0.9.1");
  });

  it("accepts KV facts derived from the exact build revision", () => {
    const build = snapshot();
    const cached = snapshot({ generatedAt: "2026-07-21T23:00:00Z", toolCount: 67 });

    const result = resolveFacts(build, cached);

    expect(result.source).toBe("kv");
    expect(result.reason).toBe("same-source-revision");
    expect(result.facts.toolCount).toBe(67);
  });

  it("accepts a source-proven newer KV snapshot", () => {
    const build = snapshot();
    const cached = snapshot({
      sourceRevision: "c".repeat(40),
      sourceCommittedAt: "2026-07-21T23:00:00Z",
      version: "0.9.2",
    });

    const result = resolveFacts(build, cached);

    expect(result.source).toBe("kv");
    expect(result.reason).toBe("kv-source-newer");
    expect(result.facts.version).toBe("0.9.2");
  });

  it("tracks the latest published release independently of stale source facts", () => {
    const build = snapshot();
    const cached = snapshot({
      sourceRevision: "b".repeat(40),
      sourceCommittedAt: "2026-07-21T21:00:00Z",
      version: "0.8.67",
      latestPublishedRelease: {
        tag: "v0.9.1",
        version: "0.9.1",
        publishedAt: "2026-07-22T01:00:00Z",
        url: "https://github.com/Hmbown/CodeWhale/releases/tag/v0.9.1",
      },
    });

    const result = resolveFacts(build, cached);

    expect(result.source).toBe("build");
    expect(result.facts.version).toBe("0.9.1");
    expect(result.facts.latestPublishedRelease?.tag).toBe("v0.9.1");
  });

  it("falls back to build facts for malformed cache data", () => {
    const result = resolveFacts(snapshot(), { version: "0.8.67" });

    expect(result.source).toBe("build");
    expect(result.reason).toBe("invalid-kv-snapshot");
    expect(result.facts.toolCount).toBe(66);
  });

  it.each([
    [
      "a malformed source revision",
      snapshot({
        sourceRevision: "not-a-git-sha",
        sourceCommittedAt: "2026-07-21T23:00:00Z",
        toolCount: 67,
      }),
    ],
    [
      "an unparseable source timestamp",
      snapshot({
        sourceRevision: "a".repeat(40),
        sourceCommittedAt: "not-a-timestamp",
        toolCount: 67,
      }),
    ],
    [
      "a source revision without its timestamp",
      snapshot({
        sourceRevision: "a".repeat(40),
        sourceCommittedAt: null,
        toolCount: 67,
      }),
    ],
    [
      "a source timestamp without its revision",
      snapshot({
        sourceRevision: null,
        sourceCommittedAt: "2026-07-21T23:00:00Z",
        toolCount: 67,
      }),
    ],
  ])("rejects %s before source precedence is evaluated", (_description, cached) => {
    const result = resolveFacts(snapshot(), cached);

    expect(result.source).toBe("build");
    expect(result.reason).toBe("invalid-kv-snapshot");
    expect(result.facts.toolCount).toBe(66);
  });

  it("rejects a published release whose URL is not canonical for its tag", () => {
    const cached = snapshot({
      latestPublishedRelease: {
        ...release090,
        url: "https://example.com/releases/tag/v0.9.0",
      },
      toolCount: 67,
    });

    const result = resolveFacts(snapshot(), cached);

    expect(result.source).toBe("build");
    expect(result.reason).toBe("invalid-kv-snapshot");
    expect(result.facts.toolCount).toBe(66);
  });
});
