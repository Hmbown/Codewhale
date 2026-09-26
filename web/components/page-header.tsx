import type { ReactNode } from "react";
import { Icon, type IconName } from "./icon";
import { Seal } from "./seal";
import { WhalePose, type WhalePoseName } from "./whale-pose";

/**
 * The one page header: what the reader knows (the title), why they are here
 * (one lede), what to do next (the actions). An optional kicker names the
 * place, optionally stamped with a seal; an optional aside repeats the title
 * in the other script (the bilingual Han title); an optional whale pose shows
 * the page's job.
 */
export function PageHeader({
  title,
  titleAside,
  titleAsideLang,
  lede,
  kicker,
  kickerIcon,
  seal,
  pose,
  actions,
  meta,
  children,
  titleId,
}: {
  title: ReactNode;
  /** The title in the other script, set beside it in the ombre ink. */
  titleAside?: ReactNode;
  titleAsideLang?: string;
  lede?: ReactNode;
  kicker?: ReactNode;
  kickerIcon?: IconName;
  /** One Han glyph stamped beside the kicker (components/seal.tsx). */
  seal?: string;
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
          {kicker || seal ? (
            <p className="page-kicker">
              {seal ? <Seal char={seal} /> : null}
              {kickerIcon ? <Icon name={kickerIcon} /> : null}
              {kicker}
            </p>
          ) : null}
          <h1 className="page-title" id={titleId}>
            {title}
            {titleAside ? (
              <>
                {" "}
                <span className="page-title-aside" lang={titleAsideLang}>{titleAside}</span>
              </>
            ) : null}
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

/**
 * A page section with one title, an optional scope line and one link.
 *
 * `layout="split"` is the editorial form: the title, scope and link hold the
 * left column and the content (usually a ruled list) the right. `label` is a
 * small running head above the title ("01 / Your terminal"); `seal` stamps a
 * Han glyph beside it.
 */
export function Section({
  id,
  title,
  scope,
  link,
  label,
  seal,
  layout = "stack",
  children,
  className = "",
}: {
  id: string;
  title: ReactNode;
  scope?: ReactNode;
  link?: ReactNode;
  label?: ReactNode;
  seal?: string;
  layout?: "stack" | "split";
  children?: ReactNode;
  className?: string;
}) {
  const head = (
    <div className="section-head-text">
      {label || seal ? (
        <p className="section-label">
          {seal ? <Seal char={seal} size="sm" /> : null}
          {label}
        </p>
      ) : null}
      <h2 className="section-title" id={id}>
        {title}
      </h2>
      {scope ? <p className="section-scope">{scope}</p> : null}
      {layout === "split" ? link : null}
    </div>
  );
  if (layout === "split") {
    return (
      <section className={`page-section section-split ${className}`.trim()} aria-labelledby={id}>
        <div className="section-split-head">{head}</div>
        <div className="section-split-body">{children}</div>
      </section>
    );
  }
  return (
    <section className={`page-section ${className}`.trim()} aria-labelledby={id}>
      <div className="section-head">
        {head}
        {link}
      </div>
      {children}
    </section>
  );
}
