import { MERCH_COUNTRIES, MERCH_PRODUCTS, MERCH_SIZES, MERCH_TARGET_PRICE } from "./catalog";
import { validateContributorIdentity } from "../contributor-merch";
import { MERCH_LANGUAGES } from "../content/contributor-merch";
import type { MerchConfig, QuoteInput, CountryRoute } from "./types";

export class MerchError extends Error {
  constructor(readonly status: number, readonly code: string, message: string) { super(message); }
}
export function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new MerchError(422, "invalid_request", "Check the form and try again.");
  return value as Record<string, unknown>;
}
function text(value: unknown, max: number, optional = false): string {
  if ((value === undefined || value === "") && optional) return "";
  if (typeof value !== "string" || !value.trim() || value.length > max || /[\u0000-\u001f\u007f]/.test(value)) throw new MerchError(422, "invalid_request", "Check the address and try again.");
  return value.trim();
}
export function validateQuoteInput(raw: unknown): QuoteInput {
  const value = object(raw);
  const product = MERCH_PRODUCTS.find(p => p.id === value.productId);
  if (!product || product.kind === "concept" || product.collection !== "launch") throw new MerchError(422, "product_unavailable", "This product is not ready to order.");
  if (!(MERCH_SIZES as readonly string[]).includes(String(value.size)) || value.color !== "White" || !Number.isInteger(value.quantity) || Number(value.quantity) < 1 || Number(value.quantity) > 5) throw new MerchError(422, "invalid_variant", "Select an available size and quantity.");
  const country = text(value.country, 2);
  if (!MERCH_COUNTRIES.some(c => c.code === country)) throw new MerchError(422, "country_unavailable", "This destination needs a shipping review.");
  const address = object(value.address);
  const input: QuoteInput = {
    productId: product.id, size: String(value.size), color: "White", quantity: Number(value.quantity), country,
    address: { name: text(address.name, 100), line1: text(address.line1, 150), line2: text(address.line2, 150, true), city: text(address.city, 100), region: text(address.region, 100, true), postalCode: text(address.postalCode, 24, true), phone: text(address.phone, 40) },
  };
  if (country === "US" || country === "CA") {
    if (!input.address.region || !input.address.postalCode) throw new MerchError(422, "invalid_address", "Add your state or province and postcode.");
  }
  if (product.kind === "contributor") {
    const c = object(value.contributor);
    const github = text(c.github, 50, true), contribution = text(c.contribution, 2048, true);
    if (validateContributorIdentity({ github, contribution }) || !MERCH_LANGUAGES.some(l => l.code === c.language) || !Number.isInteger(c.phraseIndex) || Number(c.phraseIndex) < 0 || Number(c.phraseIndex) > 2) throw new MerchError(422, "invalid_contributor", "Enter a GitHub identity or contribution link and select a print phrase.");
    input.contributor = { github, contribution, language: String(c.language), phraseIndex: Number(c.phraseIndex) };
  }
  return input;
}
export function parseConfig(raw: string | undefined): MerchConfig | null {
  if (!raw) return null;
  try {
    const value = JSON.parse(raw) as MerchConfig;
    const origin = new URL(value.siteOrigin);
    if (!["test", "live"].includes(value.mode) || typeof value.checkoutEnabled !== "boolean" || origin.pathname !== "/" || origin.search || origin.hash || origin.username || origin.password || !(origin.protocol === "https:" || (value.mode === "test" && origin.protocol === "http:" && ["localhost", "127.0.0.1"].includes(origin.hostname))) || !value.countries || !value.products || Array.isArray(value.countries) || Array.isArray(value.products)) return null;
    return value;
  } catch { return null; }
}
export function reviewedRoute(config: MerchConfig, input: QuoteInput) {
  const route = config.countries[input.country], product = config.products[input.productId]?.[input.country];
  const designCode = input.productId === "contributor" && input.contributor ? product?.contributorDesigns?.[input.contributor.language]?.[input.contributor.phraseIndex] : product?.designCode;
  if (!route || product?.approved !== true || !designCode || !product.skuBySize?.[input.size] || route.taxReviewed !== true || route.addressPolicyReviewed !== true || route.supplierPricesUsdReviewed !== true || !Array.isArray(route.allowedPostalPrefixes) || !route.allowedPostalPrefixes.length || route.allowedPostalPrefixes.some(p => typeof p !== "string" || !p.trim()) || !route.taxNote || route.taxNote.length > 250 || !route.regionCode || !route.shippingLevelCode || !["cny", "usd"].includes(route.currency) || ![route.usdExchangeRate, route.feeRate, route.flatFeeUsd, product.productionCostUsd].every(Number.isFinite) || route.usdExchangeRate <= 0 || (route.currency === "usd" && route.usdExchangeRate !== 1) || route.feeRate < 0 || route.feeRate >= .2 || route.flatFeeUsd < 0 || product.productionCostUsd <= 0) throw new MerchError(409, "route_unavailable", "This design and destination are still awaiting a shipping and print review.");
  const postcode = input.address.postalCode.replace(/\s/g, "").toUpperCase();
  if (route.blockedPostalPrefixes?.some(p => postcode.startsWith(p.toUpperCase())) || (route.allowedPostalPrefixes?.length && !route.allowedPostalPrefixes.some(p => postcode.startsWith(p.toUpperCase())))) throw new MerchError(409, "address_unavailable", "This address needs a separate delivery quote.");
  return { route, product, designCode };
}
export function priceQuote(input: QuoteInput, route: CountryRoute, productionUsd: number, shippingUsd: number) {
  if (![productionUsd, shippingUsd].every(Number.isFinite) || productionUsd <= 0 || shippingUsd < 0) throw new MerchError(503, "quote_unavailable", "Shipping prices are temporarily unavailable.");
  const shipping = Math.ceil(shippingUsd * route.usdExchangeRate * 100);
  const merchandise = input.productId === "contributor" ? Math.ceil(productionUsd * route.usdExchangeRate * 100) : MERCH_TARGET_PRICE[route.currency] * input.quantity;
  const base = merchandise + shipping;
  const total = input.productId === "contributor" ? Math.ceil((base + route.flatFeeUsd * route.usdExchangeRate * 100) / (1 - route.feeRate)) : base;
  if (!Number.isSafeInteger(total) || total <= 0 || total > 1000000) throw new MerchError(503, "quote_unavailable", "This order needs a separate quote.");
  if (input.productId !== "contributor") {
    const grossUsd = total / (route.usdExchangeRate * 100), itemUsd = merchandise / (route.usdExchangeRate * 100);
    const contributionUsd = grossUsd - productionUsd - shippingUsd - grossUsd * route.feeRate - route.flatFeeUsd - itemUsd * .05;
    if (contributionUsd < itemUsd * .30) throw new MerchError(409, "route_unavailable", "This destination needs a separate price review.");
  }
  return { merchandise, shipping, processing: total - base, total };
}
