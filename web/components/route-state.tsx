"use client";

import Image from "next/image";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { defaultLocale } from "@/lib/i18n/config";
import { getStates } from "@/lib/i18n/dictionaries";
import { pathLocale } from "@/lib/i18n/path";
import { RetryAction } from "./retry-action";
import { ErrorState, LoadingState } from "./surface-state";

/**
 * Route boundaries — `loading.tsx`, `error.tsx`, `not-found.tsx` — receive
 * no params, so these thin client shims read the locale from the pathname
 * and render the shared surface states with dictionary copy.
 */
function useRouteLocale(): string {
  const pathname = usePathname();
  return pathLocale(pathname ?? "") ?? defaultLocale;
}

export function LoadingRoute() {
  const locale = useRouteLocale();
  return <LoadingState locale={locale} lines={4} />;
}

export function ErrorRoute({ reset, digest }: { reset: () => void; digest?: string }) {
  const locale = useRouteLocale();
  const t = getStates(locale);
  return (
    <ErrorState
      locale={locale}
      titleAs="h1"
      body={digest ? `${t.errorBody} (${digest})` : t.errorBody}
      action={
        <>
          <RetryAction label={t.retry} onRetry={reset} />
          <Link href={`/${locale}`} className="btn btn-secondary">
            {t.homeLink}
          </Link>
        </>
      }
    />
  );
}

export function NotFoundRoute() {
  const locale = useRouteLocale();
  const t = getStates(locale);
  // The founder's own joke: the Codwhale game poster, kept whole at every
  // width, in a navy frame beside the way back.
  return (
    <section className="not-found" aria-labelledby="not-found-title">
      <div className="not-found-copy">
        <h1 id="not-found-title" className="not-found-title">
          <span className="not-found-code">404.</span> {t.notFoundTitle}
        </h1>
        <p className="not-found-body">{t.notFoundBody}</p>
        <div className="actions">
          <Link href={`/${locale}`} className="btn btn-primary btn-lg">
            {t.notFoundHomeLink}
          </Link>
          <Link href={`/${locale}/docs`} className="btn btn-secondary btn-lg">
            {t.docsIndexLink}
          </Link>
        </div>
      </div>
      <div className="not-found-frame stage">
        <Image
          className="not-found-poster"
          src="/codwhale-404.webp"
          alt={t.notFoundPosterAlt}
          width={1122}
          height={1402}
          priority
          unoptimized
        />
      </div>
    </section>
  );
}
