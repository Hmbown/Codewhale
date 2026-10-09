import { describe, expect, it } from "vitest";
import { tokenize } from "@/components/ratatui/highlight";

function kinds(code: string, language: "rust" | "toml" | "sh" = "rust") {
  return tokenize(code, language).map((span) => [span.text, span.kind] as const);
}

describe("code tinting", () => {
  it("marks Rust keywords, strings, numbers, macros and comments", () => {
    const spans = kinds('pub fn draw(area: Rect) {\n  // paint it\n  let n = 42;\n  vec!["x"]\n}');
    expect(spans).toContainEqual(["pub", "keyword"]);
    expect(spans).toContainEqual(["fn", "keyword"]);
    expect(spans).toContainEqual(["// paint it", "comment"]);
    expect(spans).toContainEqual(["42", "number"]);
    expect(spans).toContainEqual(["vec", "macro"]);
    expect(spans).toContainEqual(['"x"', "string"]);
  });

  it("keeps escaped quotes inside a string and never drops text", () => {
    const code = 'let s = "say \\"hi\\""; // done';
    const spans = kinds(code);
    expect(spans.map(([text]) => text).join("")).toBe(code);
    expect(spans).toContainEqual(['"say \\"hi\\""', "string"]);
  });

  it("tints shell and TOML comments without Rust keywords", () => {
    const sh = kinds("cargo run --example starter # go", "sh");
    expect(sh).toContainEqual(["# go", "comment"]);
    expect(sh.some(([, kind]) => kind === "keyword")).toBe(false);
    const toml = kinds('[dependencies]\nname = "x" # pinned', "toml");
    expect(toml).toContainEqual(["# pinned", "comment"]);
    expect(toml).toContainEqual(['"x"', "string"]);
  });
});
