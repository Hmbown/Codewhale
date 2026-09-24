"use client";

import Link from "next/link";
import Image from "next/image";
import { usePathname } from "next/navigation";
import { defaultLocale } from "@/lib/i18n/config";
import { getStates } from "@/lib/i18n/dictionaries";
import { pathLocale } from "@/lib/i18n/path";
import { RetryAction } from "./retry-action";
import { ErrorState, LoadingState } from "./surface-state";
import styles from "./not-found.module.css";

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
          <Link href={`/${locale}`} className="portal-button portal-button-secondary">
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
  return (
    <section className={styles.page} aria-labelledby="not-found-title">
      <div>
        <h1 id="not-found-title" className={styles.title}>
          <span className={styles.code}>404.</span>{" "}
          {t.notFoundTitle}
        </h1>
        <p className={styles.body}>{t.notFoundBody}</p>
        <div className={styles.actions}>
          <Link href={`/${locale}`} className="portal-button portal-button-primary">
            {t.notFoundHomeLink}
          </Link>
          <Link href={`/${locale}/docs`} className={styles.docs}>
            {t.docsIndexLink}
          </Link>
        </div>
      </div>
      <Image
        className={styles.poster}
        src="/codwhale-404.webp"
        alt={t.notFoundPosterAlt}
        width={1122}
        height={1402}
        priority
        unoptimized
      />
    </section>
  );
}
