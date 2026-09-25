#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { renderInstallGuideModule } from "./install-guide-lib.mjs";

const source = readFileSync(new URL("../../docs/INSTALL.md", import.meta.url), "utf8");
const target = new URL("../lib/install-guide.generated.ts", import.meta.url);
const generated = renderInstallGuideModule(source);
if (process.argv.includes("--check")) {
  if (readFileSync(target, "utf8") !== generated) {
    console.error("[install-guide] stale generated guide; run npm run prebuild");
    process.exit(1);
  }
  console.log("[install-guide] canonical guide is current");
} else {
  writeFileSync(target, generated);
  console.log("[install-guide] rendered docs/INSTALL.md");
}
