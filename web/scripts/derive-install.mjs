#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { renderInstallGuideModule } from "./install-guide-lib.mjs";

const source = readFileSync(new URL("../../docs/INSTALL.md", import.meta.url), "utf8");
const target = new URL("../lib/install-guide.generated.ts", import.meta.url);
const generated = renderInstallGuideModule(source);
// The output is not tracked (web/.gitignore): prebuild, dev and the vitest
// global setup write it, so it can never go stale against a merged main.
// Rendering throws on a broken internal link, which is the gate.
writeFileSync(target, generated);
console.log("[install-guide] rendered docs/INSTALL.md");
