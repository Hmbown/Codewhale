"use client";

import { useState } from "react";
import { INSTALL_COMMANDS } from "@/lib/content/install";
import { InstallCodeBlock } from "./install-code-block";

/**
 * The hero's copyable install plate: one segmented choice between the
 * checked shell installer (macOS and Linux) and npm (any platform with
 * Node 18+), above the same copy block the install page uses. The option
 * labels are code-owned proper nouns; only the group's accessible name is
 * translated.
 */
const OPTIONS = [
  { id: "shell", label: "macOS · Linux", cmd: INSTALL_COMMANDS.shell },
  { id: "npm", label: "npm", cmd: INSTALL_COMMANDS.npm },
] as const;

export function HeroInstall({
  ariaLabel,
  copyLabel,
  copiedLabel,
}: {
  ariaLabel: string;
  copyLabel: string;
  copiedLabel: string;
}) {
  const [selected, setSelected] = useState<(typeof OPTIONS)[number]["id"]>("shell");
  const option = OPTIONS.find((o) => o.id === selected) ?? OPTIONS[0];

  return (
    <div className="hero-install plate">
      <div className="segmented" role="group" aria-label={ariaLabel}>
        {OPTIONS.map((o) => (
          <button
            key={o.id}
            type="button"
            onClick={() => setSelected(o.id)}
            aria-pressed={o.id === selected}
          >
            {o.label}
          </button>
        ))}
      </div>
      <InstallCodeBlock cmd={option.cmd} copyLabel={copyLabel} copiedLabel={copiedLabel} />
    </div>
  );
}
