"use client";

import { useState } from "react";
import { Icon } from "./icon";
import { recordUsage } from "./usage-counting";

interface Props {
  cmd: string;
  copyLabel?: string;
  copiedLabel?: string;
  copyLocale?: string;
}

/**
 * A command in a code block with one icon button that copies it. The check
 * mark and the "Copied" announcement appear only after the clipboard write
 * succeeds; a failed write never shows a false confirmation.
 */
export function InstallCodeBlock({ cmd, copyLabel = "Copy", copiedLabel = "Copied", copyLocale }: Props) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    if (typeof navigator === "undefined" || !navigator.clipboard) return;
    try {
      await navigator.clipboard.writeText(cmd);
      setCopied(true);
      recordUsage("install_copy");
      setTimeout(() => setCopied(false), 1400);
    } catch {
      // Clipboard write failed (permissions, non-secure context, or a focused
      // write race) — never show a false "Copied" confirmation.
    }
  };

  return (
    <div className="code">
      <pre tabIndex={0} className="code-block">{cmd}</pre>
      <button
        type="button"
        lang={copyLocale}
        onClick={copy}
        aria-label={copyLabel}
        title={copyLabel}
        data-copied={copied}
        className="nav-icon-button nav-icon-button-sm code-copy"
      >
        <Icon name={copied ? "check" : "copy"} className="nav-icon" />
      </button>
      <span className="sr-only" aria-live="polite" lang={copyLocale}>
        {/* The check mark is drawn by the icon; announce only the word. */}
        {copied ? copiedLabel.replace(/\s*✓\s*$/u, "") : ""}
      </span>
    </div>
  );
}
