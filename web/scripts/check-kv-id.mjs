#!/usr/bin/env node
// Require existing storage identities before cf deploy can provision namespaces.
import { convertToWranglerConfig, loadAndParseConfig } from "@cloudflare/config";
import { fileURLToPath } from "node:url";

const { result } = await loadAndParseConfig(fileURLToPath(new URL("../cloudflare.config.ts", import.meta.url)), {
  isPreview: false,
  mode: undefined,
});
if (!result.success) throw new Error(`Invalid Cloudflare configuration: ${result.error}`);
let dirty = false;
for (const ns of convertToWranglerConfig(result.data).kv_namespaces ?? []) {
  if (!/^[a-f0-9]{32}$/i.test(ns.id ?? "")) {
    dirty = true;
    console.error("KV namespace %s needs its existing namespace ID in cloudflare.config.ts.", ns.binding);
  }
}
if (dirty) process.exit(1);
console.log("All KV namespace IDs are set.");
