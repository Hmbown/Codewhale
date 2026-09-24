import { readFileSync } from "node:fs";

/**
 * The hand-written site stylesheet as one string, in cascade order.
 *
 * `app/globals.css` is an import hub; its rules live in `app/styles/*.css`.
 * This inlines each `./styles/` import in place so the contract tests can read
 * the stylesheet the way the browser resolves it. The generated `tokens.css`
 * is left out, as it was before the split.
 *
 * Node-only (`node:fs`): imported by the contract tests, never by a component.
 */
export function siteCss(): string {
  const appDir = new URL("../app/", import.meta.url);
  const hub = readFileSync(new URL("globals.css", appDir), "utf8");
  return hub.replace(/^@import\s+"\.\/(styles\/[\w-]+\.css)";$/gm, (_, path: string) =>
    readFileSync(new URL(path, appDir), "utf8"),
  );
}
