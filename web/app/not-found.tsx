import { defaultLocale } from "@/lib/i18n/config";
import { NotFoundRoute } from "@/components/route-state";
import LocaleLayout from "./[locale]/layout";

export { metadata } from "./[locale]/not-found";

/**
 * Root not-found boundary. `[locale]/layout.tsx` calls `notFound()` for a first
 * segment that is not a locale (`/wp-login.php`); a layout's own not-found
 * boundary cannot catch what the layout throws, so the error lands here. The
 * root layout renders no document, so this wraps the not-found state in the
 * default locale's shell — nav, footer, and dictionary copy, like
 * `/en/nonexistent` — instead of the framework's bare page.
 */
export default function RootNotFound() {
  return (
    <LocaleLayout params={Promise.resolve({ locale: defaultLocale })}>
      <div className="route-state">
        <NotFoundRoute />
      </div>
    </LocaleLayout>
  );
}
