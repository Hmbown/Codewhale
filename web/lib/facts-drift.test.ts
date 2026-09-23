import { afterEach, describe, expect, it, vi } from "vitest";
import { deriveFactsFromRemote } from "./facts-drift";

const REVISION = "b".repeat(40);

function response(body: string, status = 200): Response {
  return new Response(body, { status });
}

function installGitHubFixture(toolCountSource: string | null): void {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: string | URL | Request) => {
      const url = String(input);
      if (url.endsWith("/commits/main")) {
        return response(
          JSON.stringify({
            sha: REVISION,
            commit: { committer: { date: "2026-07-21T23:00:00Z" } },
          }),
        );
      }
      if (url.endsWith("/releases/latest")) {
        return response(
          JSON.stringify({
            tag_name: "v0.9.0",
            published_at: "2026-07-16T20:05:39Z",
            html_url: "https://github.com/Hmbown/CodeWhale/releases/tag/v0.9.0",
          }),
        );
      }
      const rawPath = url.split(`/${REVISION}/`)[1];
      const sources: Record<string, string> = {
        "Cargo.toml": 'version = "0.9.2"\nmembers = ["crates/tui"]',
        "crates/tui/src/config.rs":
          'pub enum ApiProvider {\n    Deepseek,\n}\nconst DEFAULT_TEXT_MODEL: &str = "remote-model";',
        "crates/tui/src/config/models.rs": "",
        "crates/tui/src/sandbox/mod.rs": `
          pub const PUBLIC_SANDBOX_BACKENDS: &[&str] = &[
            "seatbelt (macOS, when available)",
            "bubblewrap (Linux, opt-in when installed)",
          ];
        `,
        "npm/codewhale/package.json": JSON.stringify({ engines: { node: ">=18" } }),
        LICENSE: "MIT License\n",
      };
      if (rawPath === "web/lib/facts.generated.ts") {
        return toolCountSource === null ? response("not found", 404) : response(toolCountSource);
      }
      return rawPath && rawPath in sources
        ? response(sources[rawPath])
        : response("not found", 404);
    }),
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("deriveFactsFromRemote", () => {
  it("derives tool count and model rows from the same exact remote revision", async () => {
    installGitHubFixture(
      'export const FACTS: RepoFacts = {"toolCount":73,"models":[' +
        '{"id":"deepseek-v4-pro","provider":"DeepSeek","contextWindow":1000000,' +
        '"maxOutput":128000,"reasoning":true,"addedAt":"2026-07-01"}' +
        "]};",
    );

    const facts = await deriveFactsFromRemote();

    expect(facts?.sourceRevision).toBe(REVISION);
    expect(facts?.version).toBe("0.9.2");
    expect(facts?.toolCount).toBe(73);
    expect(facts?.models).toEqual([
      {
        id: "deepseek-v4-pro",
        provider: "DeepSeek",
        contextWindow: 1000000,
        maxOutput: 128000,
        reasoning: true,
        addedAt: "2026-07-01",
      },
    ]);
    expect(facts?.sandboxBackends).toEqual([
      "seatbelt (macOS, when available)",
      "bubblewrap (Linux, opt-in when installed)",
    ]);
    const fetchMock = vi.mocked(fetch);
    expect(
      fetchMock.mock.calls.some(([input]) => String(input).includes("/contents/")),
    ).toBe(false);
  });

  it("fails derivation when the exact revision has no valid tool count", async () => {
    installGitHubFixture(null);

    await expect(deriveFactsFromRemote()).resolves.toBeNull();
  });

  it("fails derivation when the exact revision has malformed model rows", async () => {
    installGitHubFixture(
      'export const FACTS: RepoFacts = {"toolCount":73,"models":[{"id":42}]};',
    );

    await expect(deriveFactsFromRemote()).resolves.toBeNull();
  });
});
