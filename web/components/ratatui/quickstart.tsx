"use client";

import type { RatatuiCopy } from "@/lib/content/ratatui";
import { CopyButton } from "./explorer";
import { CodeBlock } from "./highlight";
import { INSTALL, STARTER_SOURCE, TRY_STARTER, getRecipe, learningGuideUrl } from "@/lib/ratatui/recipes";
import "./quickstart.css";

export function RatatuiQuickstart({ copy }: { copy: RatatuiCopy }) {
  const composer = getRecipe("composer");
  return <section id="ratatui-install" className="rat-quickstart" aria-labelledby="rat-start-title">
    <div className="rat-start-intro">
      <h2 id="rat-start-title">{copy.firstAppTitle}</h2>
      <p>{copy.firstAppDescription}</p>
      <div className="rat-start-links">
        <a href={STARTER_SOURCE}>{copy.starterSource} →</a>
        <a href={learningGuideUrl()}>{copy.guide} →</a>
      </div>
    </div>
    <div className="rat-start-command">
      <CopyButton text={TRY_STARTER} copy={copy} />
      <CodeBlock code={TRY_STARTER} language="sh" />
      <p>{copy.rustVersion} · {copy.license}</p>
    </div>
    <details className="rat-existing-app">
      <summary>{copy.existingApp}</summary>
      <div className="rat-existing-body">
        <p>{copy.installationDescription}</p>
        <CopyButton text={INSTALL} copy={copy} />
        <CodeBlock code={INSTALL} language="toml" />
        <p>{copy.themeNote}</p>
        <CodeBlock code={"let theme = codewhale_ratatui::Theme::detect().tui();"} language="rust" />
        {composer && <><CopyButton text={composer.code} copy={copy} /><CodeBlock code={composer.code} language="rust" /></>}
        <dl className="rat-host-flow">
          <div><dt>{copy.stateTitle}</dt><dd>{copy.stateDescription}</dd></div>
          <div><dt>{copy.paintTitle}</dt><dd>{copy.paintDescription}</dd></div>
          <div><dt>{copy.actionsTitle}</dt><dd>{copy.actionsDescription}</dd></div>
        </dl>
      </div>
    </details>
  </section>;
}
