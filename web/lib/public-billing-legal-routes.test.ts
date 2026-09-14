import { existsSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { redirect } from "next/navigation";
import PricingPage from "../app/[locale]/pricing/page";
import { footerLegalLinks } from "./i18n/links";
import { getChrome } from "./i18n/dictionaries";
import { LEGAL_UPDATED, PRIVACY_SECTIONS, TERMS_SECTIONS } from "./legal-copy";

const webRoot = new URL("../", import.meta.url);
vi.mock("next/navigation", () => ({ redirect: vi.fn() }));

describe("public legal routes and retired pricing", () => {
  it("ships real pages for the URLs that used to 404", () => {
    for (const path of [
      "app/[locale]/legal/terms/page.tsx",
      "app/[locale]/legal/privacy/page.tsx",
      "app/[locale]/privacy/page.tsx",
      "app/[locale]/terms/page.tsx",
    ]) {
      expect(existsSync(new URL(path, webRoot)), path).toBe(true);
    }
  });

  it("aliases /privacy and /terms onto the legal paths instead of inventing a second policy", () => {
    expect(footerLegalLinks("en", getChrome("en")).map((l) => l.href)).toEqual([
      "/en/legal/terms",
      "/en/legal/privacy",
    ]);
    expect(LEGAL_UPDATED).toBe("September 4, 2026");
    expect(TERMS_SECTIONS.some((s) => s.title === "Plans and charges")).toBe(true);
    expect(PRIVACY_SECTIONS.some((s) => s.title === "Retention and deletion")).toBe(true);
  });

  it("takes incoming pricing links to installation in the visitor's locale", async () => {
    for (const locale of ["en", "zh", "pt-BR"]) {
      await PricingPage({ params: Promise.resolve({ locale }) });
      expect(redirect).toHaveBeenLastCalledWith(`/${locale}/install`);
    }
  });
});
