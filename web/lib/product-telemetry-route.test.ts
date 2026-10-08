import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { handleProductTelemetry } from "@/app/api/product-telemetry/route";

const APP_DIR = fileURLToPath(new URL("../app", import.meta.url));

function sourceFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    return /\.(m?[jt]sx?)$/.test(entry.name) ? [path] : [];
  });
}

// `export const runtime = "edge"`, `runtime: 'experimental-edge'`, and so on.
// Route segment config only takes effect in app/ files, so that is the scan.
const EDGE_RUNTIME =
  /(?:\bexport\s+const\s+runtime\s*=|\bruntime\s*:)\s*["'](?:experimental-)?edge["']/;

describe("/api/product-telemetry", () => {
  it("recognizes edge and experimental-edge declarations", () => {
    for (const source of [
      'export const runtime = "edge";',
      "export const runtime = 'experimental-edge';",
      'export const config = { runtime: "edge" };',
    ]) {
      expect(EDGE_RUNTIME.test(source), source).toBe(true);
    }
    expect(EDGE_RUNTIME.test('export const runtime = "nodejs";')).toBe(false);
  });

  // @opennextjs/cloudflare does not support the edge runtime (its migrate
  // command says to remove the declaration). This is a source guard; it does
  // not exercise the adapter or the deployed worker.
  it("no app route or page opts into the edge runtime", () => {
    const edge = sourceFiles(APP_DIR).filter((file) =>
      EDGE_RUNTIME.test(readFileSync(file, "utf8")),
    );
    expect(edge).toEqual([]);
  });

  it("answers an empty POST without a server error", async () => {
    const post = () =>
      new Request("https://codewhale.net/api/product-telemetry", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: "{}",
      });

    const disabled = await handleProductTelemetry(post(), { ingestUrl: null });
    expect(disabled.status).toBe(200);
    expect(await disabled.json()).toEqual({ accepted: false, reason: "disabled" });

    const enabled = await handleProductTelemetry(post(), {
      ingestUrl: "https://telemetry.codewhale.net/v1/telemetry",
      forward: async () => {
        throw new Error("an invalid envelope must not be forwarded");
      },
    });
    expect(enabled.status).toBe(422);
    expect(await enabled.json()).toEqual({ accepted: false, reason: "schema" });
  });
});
