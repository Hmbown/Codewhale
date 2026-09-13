/** The helper has its own release lifecycle, independent of the Codewhale CLI. */
export const COMPUTER_USE_REPO = "https://github.com/Hmbown/codewhale-cu-plugin";
const API = "https://api.github.com/repos/Hmbown/codewhale-cu-plugin";

type RecordValue = Record<string, unknown>;
function record(value: unknown): RecordValue {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as RecordValue : {};
}

export type ComputerUseRelease = {
  status: "ready";
  version: string;
  url: string;
  downloadUrl: string;
  receiptUrl: string;
  sha256: string;
  size: number;
} | { status: "pending" | "unavailable" };

function releaseAssets(value: unknown) {
  const release = record(value);
  const version = typeof release.tag_name === "string"
    ? /^v(\d+\.\d+\.\d+)$/.exec(release.tag_name)?.[1] : undefined;
  if (!version || release.draft !== false || release.prerelease !== false
    || typeof release.published_at !== "string" || !Number.isFinite(Date.parse(release.published_at))
    || release.html_url !== `${COMPUTER_USE_REPO}/releases/tag/v${version}`
    || !Array.isArray(release.assets)) return null;
  const assets = release.assets.map(record);
  const archive = `Codewhale-Computer-Use-${version}-macos-universal.zip`;
  const asset = (name: string) => {
    const matches = assets.filter(a => a.name === name);
    const a = matches[0];
    return matches.length === 1 && a.state === "uploaded"
      && a.browser_download_url === `${COMPUTER_USE_REPO}/releases/download/v${version}/${name}` ? a : null;
  };
  const zip = asset(archive), receipt = asset("release.json");
  if (!zip || !receipt || !Number.isSafeInteger(zip.size)
    || (zip.size as number) <= 0 || (zip.size as number) > 256 * 1024 * 1024
    || !Number.isSafeInteger(receipt.size) || (receipt.size as number) <= 0
    || (receipt.size as number) > 16 * 1024
    || typeof zip.digest !== "string" || !/^sha256:[0-9a-f]{64}$/.test(zip.digest)) return null;
  return { version, archive, zip, receipt };
}

/** A tag alone is not a download. Require the packager's qualification receipt
 * to match the uploaded archive and GitHub's independently computed digest. */
export function qualifiedComputerUseRelease(release: unknown, value: unknown): ComputerUseRelease {
  const assets = releaseAssets(release), receipt = record(value);
  if (!assets) return { status: "pending" };
  const { version, archive, zip } = assets;
  const sha256 = (zip.digest as string).slice(7);
  if (receipt.version !== version || receipt.archive !== archive || receipt.platform !== "macos"
    || receipt.arch !== "universal" || receipt.notarized !== true
    || receipt.sha256 !== sha256 || receipt.size !== zip.size) return { status: "pending" };
  return {
    status: "ready", version, sha256, size: zip.size as number,
    url: `${COMPUTER_USE_REPO}/releases/tag/v${version}`,
    downloadUrl: zip.browser_download_url as string,
    receiptUrl: assets.receipt.browser_download_url as string,
  };
}

async function boundedJson(response: Response, limit: number): Promise<unknown> {
  if (!response.body) throw new Error("Missing release response");
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let length = 0, text = "";
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.byteLength;
      if (length > limit) throw new Error("Release response exceeds size limit");
      text += decoder.decode(value, { stream: true });
    }
    return JSON.parse(text + decoder.decode());
  } finally { await reader.cancel(); reader.releaseLock(); }
}

export async function getComputerUseRelease(): Promise<ComputerUseRelease> {
  // Match the site's offline build policy; ISR resolves availability after deployment.
  if (process.env.NEXT_PHASE === "phase-production-build") return { status: "unavailable" };
  try {
    const options = { next: { revalidate: 300 }, signal: AbortSignal.timeout(5000) };
    const response = await fetch(`${API}/releases/latest`, {
      ...options,
      headers: { Accept: "application/vnd.github+json", "User-Agent": "codewhale-web", "X-GitHub-Api-Version": "2022-11-28" },
    });
    if (response.status === 404) return { status: "pending" };
    if (!response.ok) return { status: "unavailable" };
    const release = await boundedJson(response, 128 * 1024);
    const assets = releaseAssets(release);
    if (!assets) return { status: "pending" };
    const receipt = await fetch(assets.receipt.browser_download_url as string, {
      next: { revalidate: 300 }, signal: AbortSignal.timeout(5000),
    });
    if (!receipt.ok) return { status: "unavailable" };
    return qualifiedComputerUseRelease(release, await boundedJson(receipt, 16 * 1024));
  } catch { return { status: "unavailable" }; }
}
