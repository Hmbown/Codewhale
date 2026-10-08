import { afterEach, describe, expect, it, vi } from "vitest";
import { localeDirection, locales } from "./i18n/config";
import * as dictionaries from "./i18n/dictionaries";

vi.mock("next/font/local", () => ({ default: () => ({ variable: "font" }) }));

const { default: LocaleLayout, generateMetadata } = await import("@/app/[locale]/layout");
const { default: RootNotFound } = await import("@/app/not-found");

function render(locale: string) {
  return LocaleLayout({ children: null, params: Promise.resolve({ locale }) });
}

afterEach(() => vi.restoreAllMocks());

describe("locale layout", () => {
  // Middleware leaves dotted files alone; unknown files reach `[locale]`
  // just like unknown directory segments, including their child routes.
  const invalidLocales = ["foo.txt", "llms-full.txt", "robots.json", "wp-login.php", "xx", ""];
  it.each(invalidLocales)("rejects %j before reading chrome dictionaries", async (locale) => {
    const chrome = vi.spyOn(dictionaries, "getChrome");
    await expect(render(locale)).rejects.toMatchObject({ digest: "NEXT_HTTP_ERROR_FALLBACK;404" });
    expect(chrome).not.toHaveBeenCalled();
  });

  // The layout cannot catch its own notFound(), so the root boundary answers.
  // Without app/not-found.tsx that was the framework's bare page, with no
  // document shell, nav, or footer.
  it("answers that not-found inside the default locale's shell", async () => {
    const element = RootNotFound();
    expect(element.type).toBe(LocaleLayout);
    const document = await LocaleLayout(element.props);
    expect(document.type).toBe("html");
    expect(document.props.lang).toBe("en");
  });

  // Otherwise the home page's title, canonical, and hreflang stream into it.
  it.each(invalidLocales)("gives %j noindex metadata without a home dictionary", async (locale) => {
    const home = vi.spyOn(dictionaries, "getHome");
    const metadata = await generateMetadata({ params: Promise.resolve({ locale }) });
    expect(metadata.title).toBe("Not found · Codewhale");
    expect(metadata.robots).toEqual({ index: false, follow: true });
    expect(metadata.alternates).toEqual({});
    expect(home).not.toHaveBeenCalled();
  });

  it.each(locales)("renders the registered locale %s", async (locale) => {
    const element = await render(locale);
    expect(element.props.lang).toBe(locale);
    expect(element.props.dir).toBe(localeDirection(locale));
  });
});
