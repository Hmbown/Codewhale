"use client";

import { useId, useState } from "react";
import Link from "next/link";
import { WORK_SURFACES, WORK_SURFACES_COPY } from "@/lib/content/work-surfaces";
import { getHome, pickText } from "@/lib/i18n/dictionaries";
import { InstallCodeBlock } from "./install-code-block";

export function WorkSurfaceLauncher({ locale }: { locale: string }) {
  const [selected, setSelected] = useState(0);
  const id = useId();
  const surface = WORK_SURFACES[selected];
  const home = getHome(locale);

  return (
    <section className="work-launcher" aria-label={pickText(WORK_SURFACES_COPY.label, locale)}>
      <div className="work-launcher-tabs" role="tablist" aria-label={pickText(WORK_SURFACES_COPY.label, locale)}>
        {WORK_SURFACES.map((item, index) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            id={`${id}-${item.id}`}
            aria-selected={selected === index}
            aria-controls={`${id}-panel`}
            tabIndex={selected === index ? 0 : -1}
            onClick={() => setSelected(index)}
            onKeyDown={(event) => {
              const rtl = event.currentTarget.closest('[dir="rtl"]') !== null;
              const direction = event.key === "ArrowRight" ? (rtl ? -1 : 1) : event.key === "ArrowLeft" ? (rtl ? 1 : -1) : 0;
              if (!direction && event.key !== "Home" && event.key !== "End") return;
              event.preventDefault();
              const next = event.key === "Home" ? 0 : event.key === "End" ? WORK_SURFACES.length - 1 : (index + direction + WORK_SURFACES.length) % WORK_SURFACES.length;
              setSelected(next);
              document.getElementById(`${id}-${WORK_SURFACES[next].id}`)?.focus();
            }}
          >
            {pickText(item.label, locale)}
          </button>
        ))}
      </div>
      <div className="work-launcher-panel" id={`${id}-panel`} role="tabpanel" aria-labelledby={`${id}-${surface.id}`} tabIndex={0}>
        <div>
          <h2>{pickText(surface.title, locale)}</h2>
          <p>{pickText(surface.body, locale)}</p>
          <Link className="folio-link" href={`/${locale}${surface.href}`}>{pickText(surface.link, locale)} <span aria-hidden="true">→</span></Link>
        </div>
        <div className="work-launcher-command">
          <InstallCodeBlock key={surface.id} cmd={surface.command} copyLabel={home.copy} copiedLabel={home.copied} trackInstall={false} />
          <p>{pickText(WORK_SURFACES_COPY.prerequisite, locale)} <Link href={`/${locale}/install`}>{pickText(WORK_SURFACES_COPY.install, locale)}</Link></p>
        </div>
      </div>
    </section>
  );
}
