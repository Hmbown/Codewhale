import "./highlight.css";

export type CodeLanguage = "rust" | "toml" | "sh";

type Span = { text: string; kind: "plain" | "comment" | "string" | "keyword" | "number" | "macro" };

const RUST_KEYWORDS = new Set(
  "as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while".split(" "),
);

/** Tiny dependency-free tinting for the small code samples on this page. */
export function tokenize(code: string, language: CodeLanguage): Span[] {
  const spans: Span[] = [];
  let rest = code;
  let plain = "";
  const flush = () => { if (plain) { spans.push({ text: plain, kind: "plain" }); plain = ""; } };
  const lineComment = language === "rust" ? "//" : "#";
  while (rest) {
    let match: RegExpMatchArray | null = null;
    if (rest.startsWith(lineComment)) {
      const end = rest.indexOf("\n");
      flush();
      spans.push({ text: end < 0 ? rest : rest.slice(0, end), kind: "comment" });
      rest = end < 0 ? "" : rest.slice(end);
      continue;
    }
    if (language === "rust" && rest.startsWith("/*")) {
      const end = rest.indexOf("*/");
      flush();
      const cut = end < 0 ? rest.length : end + 2;
      spans.push({ text: rest.slice(0, cut), kind: "comment" });
      rest = rest.slice(cut);
      continue;
    }
    if (rest[0] === '"') {
      let i = 1;
      while (i < rest.length && (rest[i] !== '"' || rest[i - 1] === "\\")) {
        if (rest[i] === "\n") break;
        i += 1;
      }
      flush();
      spans.push({ text: rest.slice(0, Math.min(i + 1, rest.length)), kind: "string" });
      rest = rest.slice(Math.min(i + 1, rest.length));
      continue;
    }
    if ((match = rest.match(/^[0-9][0-9_]*(\.[0-9_]+)?/))) {
      flush();
      spans.push({ text: match[0], kind: "number" });
      rest = rest.slice(match[0].length);
      continue;
    }
    if ((match = rest.match(/^[A-Za-z_][A-Za-z0-9_]*/))) {
      const word = match[0];
      const after = rest.slice(word.length);
      flush();
      if (language === "rust" && RUST_KEYWORDS.has(word)) spans.push({ text: word, kind: "keyword" });
      else if (after.startsWith("!")) {
        spans.push({ text: word, kind: "macro" });
        rest = after;
        continue;
      } else plain += word;
      rest = after;
      continue;
    }
    plain += rest[0];
    rest = rest.slice(1);
  }
  flush();
  return spans;
}

/** A code sample with gentle tinting. Copy still takes the raw text. */
export function CodeBlock({ code, language }: { code: string; language: CodeLanguage }) {
  return (
    <pre tabIndex={0} dir="ltr">
      <code>{tokenize(code, language).map((span, index) =>
        span.kind === "plain" ? <span key={index}>{span.text}</span>
          : <span key={index} className={`rat-tok-${span.kind}`}>{span.text}</span>,
      )}</code>
    </pre>
  );
}
