import { describe, expect, it } from "vitest";
import { contributorCost, validateContributorIdentity } from "./contributor-merch";
import { MERCH_COPY, MERCH_LANGUAGES } from "./content/contributor-merch";
import { locales } from "./i18n/config";

describe("contributor at-cost estimate", () => {
  it("covers supplier production, separate postage and processing without a profit or reserve", () => {
    const quote = contributorCost();
    expect(quote.total).toBe(45.53);
    expect(quote.merchandise).toBe(38.09);
    expect(quote.shipping).toBe(7.44);
    expect(quote.production + quote.processing + quote.shipping).toBeCloseTo(quote.total, 10);
    const afterProcessorUsd = quote.total / 6.7 * .961 - .30;
    expect(afterProcessorUsd).toBeGreaterThanOrEqual(5.12 + 1.11);
    expect(afterProcessorUsd - (5.12 + 1.11)).toBeLessThan(.01 / 6.7);
  });
  it("grosses up different supplier and payment costs rather than adding a hidden margin", () => {
    const quote = contributorCost({ production: 4, shipping: 2, rate: .05, flat: .2, fx: 7 });
    const expected = Math.ceil((4 + 2 + .2) / .95 * 7 * 100) / 100;
    expect(quote.total).toBe(expected);
    expect(quote.shipping).toBe(14);
  });
  it.each([{ fx: 0 }, { rate: 1 }, { shipping: -1 }, { production: Number.NaN }])("rejects invalid cost inputs %o", (inputs) => {
    expect(() => contributorCost(inputs)).toThrow(RangeError);
  });
});

describe("contributor identity context", () => {
  it.each(["octocat", "@octocat", "1234567", "a-name"])("accepts identity alone: %s", (github) => {
    expect(validateContributorIdentity({ github, contribution: "" })).toBeNull();
  });
  it("accepts a contribution URL without requiring a GitHub identity or a merged-PR threshold", () => {
    expect(validateContributorIdentity({ github: "", contribution: "https://github.com/codewhale-hq/CodeWhale/issues/1" })).toBeNull();
    expect(validateContributorIdentity({ github: "", contribution: "https://example.org/documentation-improvement" })).toBeNull();
  });
  it("requires one identity reference", () => {
    expect(validateContributorIdentity({ github: " ", contribution: " " })).toBe("missing");
  });
  it.each(["a--b", "-name", "name-", "a b"])("rejects malformed identity: %s", (github) => {
    expect(validateContributorIdentity({ github, contribution: "" })).toBe("badIdentity");
  });
  it.each(["javascript:alert(1)", "http://example.org", "https://user:password@example.org", "not-a-url"])("rejects unsafe or malformed URL: %s", (contribution) => {
    expect(validateContributorIdentity({ github: "", contribution })).toBe("badUrl");
  });
});

describe("contributor languages and print text", () => {
  it("covers all currently routed website languages and Traditional Chinese", () => {
    const languages = MERCH_LANGUAGES.map((v) => v.code);
    expect(languages).toHaveLength(19);
    expect(new Set(languages).size).toBe(languages.length);
    expect(languages).toEqual(expect.arrayContaining([...locales, "zh-TW"]));
    for (const language of languages) {
      const copy = MERCH_COPY[language];
      expect(Object.keys(copy).sort()).toEqual(Object.keys(MERCH_COPY.en).sort());
      expect(Object.values(copy).every((value) => typeof value === "string" ? value.length > 0 : value.length === 3)).toBe(true);
      expect(copy.phrases[0]).toContain("Codewhale");
      expect(copy.phrases[2]).toContain("Codewhale");
      if (language !== "en") expect(copy.title).not.toBe(MERCH_COPY.en.title);
    }
  });
});
