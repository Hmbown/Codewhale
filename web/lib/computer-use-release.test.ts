import { afterEach, describe, expect, it, vi } from "vitest";
import { COMPUTER_USE_REPO, getComputerUseRelease, qualifiedComputerUseRelease } from "./computer-use-release";

const archive = "Codewhale-Computer-Use-0.3.0-macos-universal.zip";
const sha256 = "a".repeat(64);
const asset = (name: string, size: number) => ({ name, size, state: "uploaded",
  browser_download_url: `${COMPUTER_USE_REPO}/releases/download/v0.3.0/${name}`, digest: `sha256:${sha256}` });
const fixture = () => ({ tag_name: "v0.3.0", draft: false, prerelease: false,
  published_at: "2026-09-13T00:00:00Z", html_url: `${COMPUTER_USE_REPO}/releases/tag/v0.3.0`,
  assets: [asset(archive, 80000000), asset("release.json", 500)] });
const receipt = () => ({ version: "0.3.0", platform: "macos", arch: "universal", archive,
  sha256, size: 80000000, notarized: true });

const API_LATEST = "https://api.github.com/repos/Hmbown/codewhale-cu-plugin/releases/latest";
const WEB_RECEIPT = `${COMPUTER_USE_REPO}/releases/latest/download/release.json`;
const OBJECT_URL = "https://objects.githubusercontent.com/github-production-release-asset/1/release.json?X-Amz-Signature=x";
const status = (code: number) => new Response(null, { status: code });
const redirect = (location: string) => new Response(null, { status: 302, headers: { location } });
const stub = (...responses: unknown[]) => {
  const fetcher = vi.fn();
  for (const r of responses) {
    if (r instanceof Error) fetcher.mockRejectedValueOnce(r); else fetcher.mockResolvedValueOnce(r);
  }
  vi.stubGlobal("fetch", fetcher);
  return fetcher;
};

afterEach(() => { vi.unstubAllGlobals(); vi.unstubAllEnvs(); vi.restoreAllMocks(); });

describe("Computer Use download qualification", () => {
  it("offers the exact archive when the release, receipt and GitHub digest agree", () => {
    expect(qualifiedComputerUseRelease(fixture(), receipt())).toMatchObject({
      status: "ready", version: "0.3.0", sha256, downloadUrl: asset(archive, 80000000).browser_download_url,
      verification: "github-digest",
    });
  });
  it.each([
    { notarized: false }, { version: "0.2.2" }, { size: 1 }, { sha256: "b".repeat(64) },
    { platform: "windows" }, { arch: "arm64" }, { archive: "unqualified.zip" },
  ])("withholds mismatched or unqualified receipts: %j", change => {
    expect(qualifiedComputerUseRelease(fixture(), { ...receipt(), ...change }).status).toBe("pending");
  });
  it("refuses drafts, prereleases, missing assets and foreign download URLs", () => {
    const foreign = fixture(); foreign.assets[0].browser_download_url = "https://example.com/app.zip";
    const duplicate = fixture(); duplicate.assets.push(duplicate.assets[0]);
    const unsigned = fixture(); unsigned.assets[0].digest = "";
    const tooLarge = fixture(); tooLarge.assets[0].size = 300 * 1024 * 1024;
    for (const release of [{ ...fixture(), draft: true }, { ...fixture(), prerelease: true },
      { ...fixture(), tag_name: "v0.3.0-rc1" }, { ...fixture(), assets: [] }, foreign, duplicate, unsigned, tooLarge]) {
      expect(qualifiedComputerUseRelease(release, receipt()).status).toBe("pending");
    }
  });
  it("loads only the canonical release and its matching receipt", async () => {
    const fetcher = stub(Response.json(fixture()), Response.json(receipt()));
    expect(await getComputerUseRelease()).toMatchObject({ status: "ready", verification: "github-digest" });
    expect(fetcher.mock.calls.map(c => c[0])).toEqual([API_LATEST, `${COMPUTER_USE_REPO}/releases/download/v0.3.0/release.json`]);
  });
  it("sends the server-held token to the API exactly when one is passed, and never elsewhere", async () => {
    const fetcher = stub(status(404), status(404), status(404));
    await getComputerUseRelease("ghp_secret");
    await getComputerUseRelease();
    const headers = (call: number) => fetcher.mock.calls[call][1].headers as Record<string, string>;
    expect(headers(0).Authorization).toBe("Bearer ghp_secret");
    expect(headers(1)).not.toHaveProperty("Authorization");
    await getComputerUseRelease("");
    expect(headers(2)).not.toHaveProperty("Authorization");
  });
  it("reports no published installer when both the API and the release endpoint say so", async () => {
    const fetcher = stub(status(404));
    expect((await getComputerUseRelease()).status).toBe("pending");
    expect(fetcher).toHaveBeenCalledTimes(1);
  });
  it("falls back to the release web endpoint when the API refuses, and stays honest when that fails too", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    stub(status(403), status(404));
    expect((await getComputerUseRelease()).status).toBe("pending");
    expect(error).toHaveBeenCalledWith("computer-use release check", 403);
    stub(status(503), new Error("offline"));
    expect((await getComputerUseRelease("ghp_secret")).status).toBe("unavailable");
    stub(new Error("offline"), status(404));
    expect((await getComputerUseRelease("ghp_secret")).status).toBe("pending");
    expect(error).toHaveBeenCalledWith("computer-use release check failed", "offline");
    stub(status(403), status(500));
    expect((await getComputerUseRelease("ghp_secret")).status).toBe("unavailable");
    expect(error.mock.calls.flat().join(" ")).not.toContain("ghp_");
  });
  it("qualifies the download from the receipt when the API is unreachable", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const fetcher = stub(status(403), redirect(OBJECT_URL), Response.json(receipt()), redirect(OBJECT_URL));
    expect(await getComputerUseRelease()).toEqual({
      status: "ready", version: "0.3.0", sha256, size: 80000000, verification: "receipt",
      url: `${COMPUTER_USE_REPO}/releases/tag/v0.3.0`,
      downloadUrl: `${COMPUTER_USE_REPO}/releases/download/v0.3.0/${archive}`,
      receiptUrl: `${COMPUTER_USE_REPO}/releases/download/v0.3.0/release.json`,
    });
    expect(fetcher.mock.calls.map(c => [c[0], c[1].method ?? "GET", c[1].redirect])).toEqual([
      [API_LATEST, "GET", undefined], [WEB_RECEIPT, "GET", "manual"], [OBJECT_URL, "GET", "manual"],
      [`${COMPUTER_USE_REPO}/releases/download/v0.3.0/${archive}`, "HEAD", "manual"],
    ]);
    expect(fetcher.mock.calls.every(c => !("Authorization" in (c[1].headers ?? {})))).toBe(true);
  });
  it("accepts a directly served archive and a permanent redirect for the receipt", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    stub(status(403), new Response(null, { status: 301, headers: { location: OBJECT_URL } }), Response.json(receipt()), status(200));
    expect(await getComputerUseRelease()).toMatchObject({ status: "ready", verification: "receipt" });
  });
  it.each([
    ["a disallowed host", redirect("https://example.com/release.json")],
    ["a redirect without a location", new Response(null, { status: 302 })],
    ["plain http", redirect("http://objects.githubusercontent.com/release.json")],
  ])("refuses a receipt redirect onto %s", async (_label, hop) => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const fetcher = stub(status(403), hop, Response.json(receipt()));
    expect((await getComputerUseRelease()).status).toBe("unavailable");
    expect(fetcher).toHaveBeenCalledTimes(2);
  });
  it("withholds the fallback when the receipt is unqualified or the archive is not served", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    stub(status(403), redirect(OBJECT_URL), Response.json({ ...receipt(), notarized: false }));
    expect((await getComputerUseRelease()).status).toBe("unavailable");
    stub(status(403), redirect(OBJECT_URL), Response.json(receipt()), status(404));
    expect((await getComputerUseRelease()).status).toBe("unavailable");
    const fetcher = stub(status(403), redirect(OBJECT_URL), redirect(OBJECT_URL), redirect(OBJECT_URL), redirect(OBJECT_URL), redirect(OBJECT_URL));
    expect((await getComputerUseRelease()).status).toBe("unavailable");
    expect(fetcher).toHaveBeenCalledTimes(5);
  });
  it("bounds malformed or oversized responses and does not fetch an unqualified receipt", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const fetcher = stub(new Response("x".repeat(128 * 1024 + 1)), new Error("offline"),
      Response.json({ ...fixture(), draft: true }));
    expect((await getComputerUseRelease()).status).toBe("unavailable");
    expect((await getComputerUseRelease()).status).toBe("pending");
    expect(fetcher).toHaveBeenCalledTimes(3);
  });
  it("keeps production builds offline without claiming that a release is available", async () => {
    vi.stubEnv("NEXT_PHASE", "phase-production-build");
    const fetcher = vi.fn(); vi.stubGlobal("fetch", fetcher);
    expect((await getComputerUseRelease()).status).toBe("unavailable");
    expect(fetcher).not.toHaveBeenCalled();
  });
});
