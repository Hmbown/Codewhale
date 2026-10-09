#!/usr/bin/env node
// OpenNext still reads the legacy config shape. Derive it with Cloudflare's
// converter, keeping cloudflare.config.ts as the only Worker configuration.
import { convertToWranglerConfig, loadAndParseConfig } from "@cloudflare/config";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const manifestLoader = resolve(
  dirname(fileURLToPath(import.meta.resolve("@opennextjs/cloudflare"))),
  "../cli/build/patches/plugins/load-manifest.js",
);
const loader = readFileSync(manifestLoader, "utf8");
const digest = (value) => createHash("sha256").update(value).digest("hex");
const originalHash = "d9b01ab939652e9c120da66dbbfc25c18b249b53258e4e21bcf59ac63412d700";
const patchedHash = "06287c108cf324ab20494dc714e7eeadbd405661f54e01108c5fa2e079e46867";
if (digest(loader) !== patchedHash) {
  if (digest(loader) !== originalHash) throw new Error("Unreviewed OpenNext manifest loader; review or retire the preview-props patch");
  const patched = loader.replace(
    "**/{*-manifest,required-server-files,prefetch-hints}.json",
    "**/{*-manifest,required-server-files,prefetch-hints,preview-props}.json",
  );
  if (digest(patched) !== patchedHash) throw new Error("OpenNext preview-props patch digest mismatch");
  writeFileSync(manifestLoader, patched);
}

const { result } = await loadAndParseConfig(resolve("cloudflare.config.ts"), {
  isPreview: false,
  mode: undefined,
});
if (!result.success) throw new Error(`Invalid Cloudflare configuration: ${result.error}`);
const config = convertToWranglerConfig(result.data);
config.main = resolve(config.main);
config.assets = { ...config.assets, directory: resolve(".open-next/assets") };
mkdirSync(".cloudflare", { recursive: true });
writeFileSync(".cloudflare/opennext.json", `${JSON.stringify(config, null, 2)}\n`);
