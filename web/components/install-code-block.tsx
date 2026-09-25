"use client";

import { useState } from "react";
import { recordUsage } from "./usage-counting";

interface Props {
  cmd: string;
  copyLabel?: string;
  copiedLabel?: string;
  copyLocale?: string;
}

export function InstallCodeBlock({ cmd, copyLabel = "Copy", copiedLabel = "Copied ✓", copyLocale }: Props) {
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
    <div className="relative">
      <button
        lang={copyLocale}
        onClick={copy}
        aria-label={copied ? copiedLabel : copyLabel}
        data-copied={copied}
        className="copy-btn absolute top-3 right-3 z-10 px-3 py-1 bg-paper hairline-t hairline-b hairline-l hairline-r rounded text-xs hover:bg-indigo hover:text-paper transition-colors"
      >
        {copied ? copiedLabel : copyLabel}
      </button>
      <pre className="code-block text-[0.78rem] m-0 max-w-full pr-20">{cmd}</pre>
    </div>
  );
}
