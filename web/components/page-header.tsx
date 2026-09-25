import type { ReactNode } from "react";
import { Icon, type IconName } from "./icon";
import { WhalePose, type WhalePoseName } from "./whale-pose";

/**
 * The one page header: what the reader knows (the title), why they are here
 * (one lede), what to do next (the actions). An optional kicker names the
 * place; an optional whale pose shows the page's job.
 */
export function PageHeader({
  title,
  lede,
  kicker,
  kickerIcon,
  pose,
  actions,
  meta,
  children,
  titleId,
}: {
  title: ReactNode;
  lede?: ReactNode;
  kicker?: ReactNode;
  kickerIcon?: IconName;
  pose?: WhalePoseName;
  actions?: ReactNode;
  meta?: ReactNode;
  children?: ReactNode;
  titleId?: string;
}) {
  return (
    <header className="page-head">
      <div className="page-head-inner">
        <div className="page-head-text">
          {kicker ? (
            <p className="page-kicker">
              {kickerIcon ? <Icon name={kickerIcon} /> : null}
              {kicker}
            </p>
          ) : null}
          <h1 className="page-title" id={titleId}>
            {title}
          </h1>
          {lede ? <p className="page-lede">{lede}</p> : null}
          {meta ? <div className="page-meta">{meta}</div> : null}
          {actions ? <div className="actions">{actions}</div> : null}
          {children}
        </div>
        {pose ? <WhalePose pose={pose} className="page-head-pose" priority /> : null}
      </div>
    </header>
  );
}

/** A page section with one title, an optional scope line and one link. */
export function Section({
  id,
  title,
  scope,
  link,
  children,
  className = "",
}: {
  id: string;
  title: ReactNode;
  scope?: ReactNode;
  link?: ReactNode;
  children?: ReactNode;
  className?: string;
}) {
  return (
    <section className={`page-section ${className}`.trim()} aria-labelledby={id}>
      <div className="section-head">
        <div className="section-head-text">
          <h2 className="section-title" id={id}>
            {title}
          </h2>
          {scope ? <p className="section-scope">{scope}</p> : null}
        </div>
        {link}
      </div>
      {children}
    </section>
  );
}
