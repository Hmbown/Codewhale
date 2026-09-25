"use client";

import { useState, useMemo, useRef, useCallback, useEffect } from "react";
import { faqSourceHref } from "@/lib/faq-source";
import { fill, getFaq } from "@/lib/i18n/dictionaries";
import { extractText } from "@/lib/react-text";
import { highlightSpan } from "@/lib/search-utils";
import { Icon } from "./icon";
import { WhalePose } from "./whale-pose";

export interface FaqSearchItem {
  q: string;
  a: React.ReactNode;
  sources?: string[];
}

/* ------------------------------------------------------------------ */
/*  Highlight helper                                                   */
/* ------------------------------------------------------------------ */

function highlight(text: string, query: string): React.ReactNode {
  // Index arithmetic lives in search-utils: lowercasing can change a
  // string's length, so `text` cannot be sliced with indices taken from
  // its lowercased copy.
  const span = highlightSpan(text, query);
  if (!span) return text;
  return (
    <>
      {span.before}
      <mark className="search-highlight">{span.match}</mark>
      {span.after}
    </>
  );
}

/* ------------------------------------------------------------------ */
/*  Component                                                           */
/* ------------------------------------------------------------------ */

export function FaqSearch({
  items,
  locale,
}: {
  items: FaqSearchItem[];
  locale: string;
}) {
  const t = getFaq(locale);
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  // Precompute haystack for each item (question + answer text + sources).
  const haystacks = useMemo(
    () =>
      items.map((item) => {
        const parts = [
          item.q,
          extractText(item.a),
          ...(item.sources ?? []),
        ];
        return parts.join(" ").toLowerCase();
      }),
    [items],
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items.map((item, i) => ({ item, i }));
    return items
      .map((item, i) => ({ item, i }))
      .filter(({ i }) => haystacks[i].includes(q));
  }, [query, haystacks, items]);

  // Keyboard shortcut: focus search on "/".
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === "/" && document.activeElement?.tagName !== "INPUT") {
      e.preventDefault();
      inputRef.current?.focus();
    }
  }, []);

  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  const total = items.length;
  const matched = filtered.length;
  const hasQuery = query.trim().length > 0;

  return (
    <>
      <div className="faq-search">
        <div className="search-field">
          <Icon name="search" className="search-field-icon" />
          <input
            ref={inputRef}
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t.searchPlaceholder}
            className="search-input"
            aria-label={t.searchLabel}
            aria-keyshortcuts="/"
          />
          {hasQuery ? (
            <button
              type="button"
              onClick={() => setQuery("")}
              className="nav-icon-button nav-icon-button-sm search-field-clear"
              aria-label={t.searchClear}
              title={t.searchClear}
            >
              <Icon name="x" className="nav-icon" />
            </button>
          ) : (
            <kbd className="search-field-kbd" aria-hidden="true">/</kbd>
          )}
        </div>
        <p className="faq-search-count" aria-live="polite">
          {hasQuery
            ? matched > 0
              ? fill(t.searchMatches, { matched, total, query: query.trim() })
              : fill(t.searchNoMatches, { query: query.trim() })
            : ""}
        </p>
      </div>

      {matched > 0 ? (
        <div className="group-card">
          {filtered.map(({ item, i }) => (
            <details key={i} className="disclosure-row">
              <summary>
                <span className="faq-index tabular" aria-hidden="true">{i + 1}</span>
                <span className="faq-question">{highlight(item.q, query)}</span>
                <Icon name="chevron-down" className="disclosure-chevron" />
              </summary>
              <div className={`disclosure-body prose faq-answer ${t.answerClassName}`.trimEnd()}>
                <div>{item.a}</div>
                {item.sources && item.sources.length > 0 && (
                  <p className="faq-sources">
                    <span>{t.sourcesLabel}:</span>
                    {item.sources.map((s) => {
                      const href = faqSourceHref(s);
                      return href ? (
                        <a key={s} href={href} target="_blank" rel="noreferrer">
                          {s}
                        </a>
                      ) : (
                        <code key={s}>{s}</code>
                      );
                    })}
                  </p>
                )}
              </div>
            </details>
          ))}
        </div>
      ) : (
        <div className="empty-state" role="status">
          <WhalePose pose="search" />
          <p className="empty-state-title">{t.noResultsTitle}</p>
          <p className="empty-state-reason">{t.noResultsBody}</p>
        </div>
      )}
    </>
  );
}
