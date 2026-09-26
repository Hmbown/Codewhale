import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

/**
 * Derive the untracked build inputs before any test file imports them.
 * `lib/changelog.generated.ts` and `lib/install-guide.generated.ts` are not
 * committed (see web/.gitignore), so a bare `vitest run` on a fresh checkout
 * must write them from CHANGELOG.md and docs/INSTALL.md first. Facts stay out
 * of this list on purpose: `facts.generated.ts` is committed, and rewriting it
 * here would hide the drift `npm run check:facts` exists to catch.
 */
export default function setup() {
  for (const script of ["derive-changelog.mjs", "derive-install.mjs"]) {
    execFileSync(process.execPath, [fileURLToPath(new URL(`./scripts/${script}`, import.meta.url))], {
      stdio: "inherit",
    });
  }
}
