const REPOSITORY = "https://github.com/codewhale-hq/CodeWhale";

export function faqSourceHref(source: string): string | null {
  const issue = source.match(/^#(\d+)$/);
  if (issue) return `${REPOSITORY}/issues/${issue[1]}`;
  if (source.endsWith(".md") || source.includes("/")) {
    return `${REPOSITORY}/blob/main/${source}`;
  }
  return null;
}
