import Link from "next/link";
import { ACCOUNT_ENTRY_COPY } from "@/lib/content/account-entry";
import { pickText } from "@/lib/i18n/dictionaries";
import { publicAuthAppDestination, type PublicAuthKind } from "@/lib/public-auth-routes";
import { WhalePose } from "./whale-pose";

/**
 * The public sign-in and create-account doors. codewhale.net is not the
 * signed-in app: one primary action sends the reader to it, one secondary
 * action installs locally (no account needed), and one line switches
 * between signing in and creating an account. The whale listens.
 */
export function PublicAccountEntry({
  locale,
  kind,
}: {
  locale: string;
  kind: Exclude<PublicAuthKind, "callback">;
}) {
  const creating = kind === "sign-up";
  const copy = creating ? ACCOUNT_ENTRY_COPY.signUp : ACCOUNT_ENTRY_COPY.signIn;
  const appHref = publicAuthAppDestination(kind, locale);
  const otherHref = `/${locale}/${creating ? "signin" : "signup"}`;

  return (
    <div className="account-entry">
      <WhalePose pose="listen" className="account-entry-pose" priority />
      <h1 className="page-title">{pickText(copy.title, locale)}</h1>
      <p className="page-lede">{pickText(ACCOUNT_ENTRY_COPY.lede, locale)}</p>
      <div className="actions">
        <a className="btn btn-primary btn-lg" href={appHref} data-usage={creating ? "signup" : "login"}>
          {pickText(copy.action, locale)}
        </a>
        <Link className="btn btn-secondary btn-lg" href={`/${locale}/install`}>
          {pickText(ACCOUNT_ENTRY_COPY.installLocally, locale)}
        </Link>
      </div>
      <p className="page-meta">
        {pickText(copy.switchPrompt, locale)}{" "}
        <Link href={otherHref} className="link">
          {pickText(copy.switchLabel, locale)}
        </Link>
      </p>
    </div>
  );
}
