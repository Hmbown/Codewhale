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

afterEach(() => { vi.unstubAllGlobals(); vi.unstubAllEnvs(); });

describe("Computer Use download qualification", () => {
  it("offers the exact archive when the release, receipt and GitHub digest agree", () => {
    expect(qualifiedComputerUseRelease(fixture(), receipt())).toMatchObject({
      status: "ready", version: "0.3.0", sha256, downloadUrl: asset(archive, 80000000).browser_download_url,
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
    const fetcher = vi.fn().mockResolvedValueOnce(Response.json(fixture())).mockResolvedValueOnce(Response.json(receipt()));
    vi.stubGlobal("fetch", fetcher);
    expect((await getComputerUseRelease()).status).toBe("ready");
    expect(fetcher.mock.calls.map(c => c[0])).toEqual([
      "https://api.github.com/repos/Hmbown/codewhale-cu-plugin/releases/latest",
      `${COMPUTER_USE_REPO}/releases/download/v0.3.0/release.json`,
    ]);
  });
  it("distinguishes no published installer from a failed availability check", async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(new Response(null, { status: 404 }))
      .mockResolvedValueOnce(new Response(null, { status: 503 })).mockRejectedValueOnce(new Error("offline"));
    vi.stubGlobal("fetch", fetcher);
    expect((await getComputerUseRelease()).status).toBe("pending");
    expect((await getComputerUseRelease()).status).toBe("unavailable");
    expect((await getComputerUseRelease()).status).toBe("unavailable");
  });
  it("bounds malformed or oversized responses and does not fetch an unqualified receipt", async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(new Response("x".repeat(128 * 1024 + 1)))
      .mockResolvedValueOnce(Response.json({ ...fixture(), draft: true }));
    vi.stubGlobal("fetch", fetcher);
    expect((await getComputerUseRelease()).status).toBe("unavailable");
    expect((await getComputerUseRelease()).status).toBe("pending");
    expect(fetcher).toHaveBeenCalledTimes(2);
  });
  it("keeps production builds offline without claiming that a release is available", async () => {
    vi.stubEnv("NEXT_PHASE", "phase-production-build");
    const fetcher = vi.fn(); vi.stubGlobal("fetch", fetcher);
    expect((await getComputerUseRelease()).status).toBe("unavailable");
    expect(fetcher).not.toHaveBeenCalled();
  });
});
