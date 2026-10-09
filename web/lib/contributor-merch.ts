/** Local contributor draft only: no eligibility check, payment or vendor order. */
export type ContributorIdentity = { github: string; contribution: string };
export type IdentityError = "missing" | "badIdentity" | "badUrl";

export function validateContributorIdentity(value: ContributorIdentity): IdentityError | null {
  const github = value.github.trim().replace(/^@/, "");
  const contribution = value.contribution.trim();
  if (!github && !contribution) return "missing";
  if (github && (!/^[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,37}[a-zA-Z0-9])?$/.test(github) || github.includes("--"))) return "badIdentity";
  if (contribution) {
    try {
      const url = new URL(contribution);
      if (contribution.length > 2048 || url.protocol !== "https:" || url.username || url.password) return "badUrl";
    } catch { return "badUrl"; }
  }
  return null;
}

export const CONTRIBUTOR_CATALOG_SOURCE = "https://www.yoycol.com/product_detail/DJCTX/Unisex-O-neck-Short-Sleeve-T-shirt-180GSM-Cotton-DTF";

/** Catalog-floor estimate; artwork, tax, address and real FX are still unquoted. */
export function contributorCost({ production = 5.12, shipping = 1.11, rate = .039, flat = .30, fx = 6.70 } = {}) {
  if (![production, shipping, rate, flat, fx].every(Number.isFinite) || production < 0 || shipping < 0 || flat < 0 || rate < 0 || rate >= 1 || fx <= 0) throw new RangeError("Invalid cost inputs");
  const productionCny = production * fx;
  const shippingCny = shipping * fx;
  const unrounded = (productionCny + shippingCny + flat * fx) / (1 - rate);
  // Round the whole charge upward to avoid rounding below processing-inclusive cost.
  const total = Math.ceil((unrounded - Number.EPSILON) * 100) / 100;
  const postage = Math.round(shippingCny * 100) / 100;
  const made = Math.round(productionCny * 100) / 100;
  return { production: made, shipping: postage, processing: Math.round((total - made - postage) * 100) / 100, total, merchandise: Math.round((total - postage) * 100) / 100, fx, rate, flat };
}
