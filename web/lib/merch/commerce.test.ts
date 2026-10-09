import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DatabaseSync, type SQLInputValue } from "node:sqlite";
import { readFileSync } from "node:fs";
import Stripe from "stripe";
import { handleMerch } from "./server";
import { operatorAction, supplierRequest } from "./operator";
import { parseConfig, reviewedRoute, validateQuoteInput, priceQuote } from "./model";
import { destinationRate, yoycolHeaders, yoycolRequest } from "./yoycol";
import type { MerchConfig, MerchDatabase, MerchEnv, D1Statement, Quote } from "./types";

class Sqlite implements MerchDatabase {
  db = new DatabaseSync(":memory:");
  constructor() { this.db.exec(readFileSync(new URL("../../migrations/merch/0001_merch.sql", import.meta.url), "utf8")); }
  prepare(sql: string): D1Statement {
    let values: SQLInputValue[] = [];
    const result: D1Statement = {
      bind: (...args) => { values = args as SQLInputValue[]; return result; },
      first: async <T>() => this.db.prepare(sql).get(...values) as T ?? null,
      all: async <T>() => ({ results: this.db.prepare(sql).all(...values) as T[] }),
      run: async () => ({ meta: { changes: Number(this.db.prepare(sql).run(...values).changes) } }),
    };
    return result;
  }
  async batch(statements: D1Statement[]) {
    this.db.exec("BEGIN");
    try { const result = []; for (const statement of statements) result.push(await statement.run()); this.db.exec("COMMIT"); return result; }
    catch (error) { this.db.exec("ROLLBACK"); throw error; }
  }
}
const input = { productId: "whale-bro", size: "XL", color: "White", quantity: 1, country: "US", address: { name: "Sample Tester", line1: "1 Test Street", line2: "", city: "Test City", region: "CA", postalCode: "94107", phone: "+15555550100" } };
function configuration(): MerchConfig {
  return { mode: "test", checkoutEnabled: true, supplierSubmissionEnabled: false, siteOrigin: "http://localhost:3187", countries: { US: { regionCode: "US", shippingLevelCode: "FIXTURE", currency: "usd", usdExchangeRate: 1, feeRate: .054, flatFeeUsd: .30, taxReviewed: true, taxNote: "Fixture: import charges excluded.", supplierPricesUsdReviewed: true, addressPolicyReviewed: true, allowedPostalPrefixes: ["941"], blockedPostalPrefixes: ["94199"] } }, products: { "whale-bro": { US: { approved: true, designCode: "fixture-design", skuBySize: { XL: "fixture-sku" }, productionCostUsd: 5.12 } }, contributor: { US: { approved: true, designCode: "", contributorDesigns: { en: ["fixture-en-0", "fixture-en-1", "fixture-en-2"] }, skuBySize: { XL: "fixture-sku" }, productionCostUsd: 5.12 } } } };
}
let db: Sqlite, env: MerchEnv, config: MerchConfig;
let currentSession: Stripe.Checkout.Session;
let sent: { url: string; method: string; body: string; idempotency: string | null }[];
let supplierFailure = false;
beforeEach(() => {
  db = new Sqlite(); config = configuration(); sent = []; supplierFailure = false;
  env = { MERCH_DB: db, MERCH_RATE_LIMITER: { limit: async () => ({ success: true }) }, MERCH_CONFIG_JSON: JSON.stringify(config), MERCH_STRIPE_KEY: "sk_test_fixture", MERCH_STRIPE_WEBHOOK_SECRET: "whsec_fixture", MERCH_PAYMENT_CONFIGURATION: "pmc_fixture", YOYCOL_ACCESS_KEY: "fixture-key", YOYCOL_SECRET_KEY: "fixture-secret" };
  currentSession = { object: "checkout.session", id: "cs_test_fixture", livemode: false, metadata: {}, client_reference_id: "", currency: "usd", amount_total: 1725, payment_status: "unpaid", status: "complete", url: "https://checkout.stripe.com/c/pay/fixture" } as Stripe.Checkout.Session;
  vi.stubGlobal("fetch", vi.fn(async (url: URL | string, init?: RequestInit) => {
    const u = new URL(String(url)), method = init?.method ?? "GET", body = String(init?.body ?? "");
    sent.push({ url: u.href, method, body, idempotency: new Headers(init?.headers).get("idempotency-key") });
    if (u.hostname === "www.yoycol.com") {
      if (method === "POST") {
        if (supplierFailure) throw new Error("ambiguous connection loss");
        const value = JSON.parse(body);
        return Response.json({ code: "100000", data: { orderId: 42, storeOrderSn: value.storeOrderSn, currency: "USD", orderRealAmount: 7.37, canPay: true } });
      }
      return Response.json({ code: "100000", data: [{ regionCode: "US", levels: [{ levelCode: "FIXTURE", levelName: "Fixture service", firstPrice: 2.25, additionalPrice: .75 }] }] });
    }
    if (u.hostname === "api.stripe.com") {
      if (method === "POST") {
        const data = new URLSearchParams(body);
        currentSession.client_reference_id = data.get("client_reference_id");
        currentSession.metadata = { merch_order_id: data.get("metadata[merch_order_id]")!, merch_product_id: data.get("metadata[merch_product_id]")! };
        currentSession.currency = data.get("line_items[0][price_data][currency]");
        currentSession.amount_total = Number(data.get("line_items[0][price_data][unit_amount]")) + Number(data.get("line_items[1][price_data][unit_amount]"));
      }
      return Response.json(currentSession);
    }
    throw new Error("No unmocked network is permitted in commerce tests");
  }));
});
afterEach(() => { vi.unstubAllGlobals(); db.db.close(); });
function request(action: string, data: unknown, origin = config.siteOrigin) {
  return new Request("http://localhost:3187/api/merch/" + action, { method: "POST", headers: { "Content-Type": "application/json", Origin: origin }, body: JSON.stringify(data) });
}
async function quoted() {
  const response = await handleMerch(request("quote", input), "quote", env);
  expect(response.status).toBe(200);
  return response.json() as Promise<Quote>;
}
async function checkout() {
  const quote = await quoted();
  const response = await handleMerch(request("checkout", { quoteId: quote.quoteId }), "checkout", env);
  expect(response.status).toBe(200); return quote;
}
async function event(type = "checkout.session.completed", eventId = "evt_fixture", valid = true, livemode = config.mode === "live") {
  const payload = JSON.stringify({ id: eventId, object: "event", livemode, type, created: Math.floor(Date.now()/1000), data: { object: { id: currentSession.id, metadata: currentSession.metadata } } });
  const signature = Stripe.webhooks.generateTestHeaderString({ payload, secret: "whsec_fixture" });
  return handleMerch(new Request("http://localhost/api/merch/webhook", { method: "POST", body: payload, headers: { "stripe-signature": valid ? signature : "invalid" } }), "webhook", env);
}
function state(id: string) { return db.db.prepare("SELECT payment_state,fulfillment_state FROM merch_orders WHERE id=?").get(id); }

describe("commerce trust boundaries", () => {
  it("fails closed without storage, keys or reviewed rates", async () => {
    expect(await (await handleMerch(new Request("http://localhost/status"), "status", {})).json()).toEqual({ checkoutConfigured: false, readyProductIds: [] });
    expect((await handleMerch(request("quote", input), "quote", {})).status).toBe(503);
    config.countries.US.addressPolicyReviewed = false; env.MERCH_CONFIG_JSON = JSON.stringify(config);
    expect((await handleMerch(request("quote", input), "quote", env)).status).toBe(409);
    expect(sent).toHaveLength(0);
  });
  it("accepts only a real catalog variant and bounded quantity", () => {
    for (const value of [{ ...input, productId: "deskmat" }, { ...input, productId: "field" }, { ...input, quantity: 0 }, { ...input, quantity: 6 }, { ...input, size: "10XL" }, { ...input, color: "Black" }, { ...input, country: "ZZ" }]) expect(() => validateQuoteInput(value)).toThrow();
    expect(validateQuoteInput(input).size).toBe("XL");
  });
  it("rejects remote/unreviewed postcodes and unapproved contributor print languages", () => {
    expect(() => reviewedRoute(config, validateQuoteInput({ ...input, address: { ...input.address, postalCode: "94199" } }))).toThrow();
    const c = validateQuoteInput({ ...input, productId: "contributor", contributor: { github: "", contribution: "https://github.com/codewhale-hq/CodeWhale/pull/123", language: "zh", phraseIndex: 1 } });
    expect(() => reviewedRoute(config, c)).toThrow();
    c.contributor!.language = "en";
    expect(reviewedRoute(config, c).designCode).toBe("fixture-en-1");
  });
  it("rejects non-boolean activation and insecure production origins", () => {
    expect(parseConfig(JSON.stringify({ ...config, checkoutEnabled: "true" }))).toBeNull();
    expect(parseConfig(JSON.stringify({ ...config, mode: "live" }))).toBeNull();
    expect(parseConfig(JSON.stringify({ ...config, siteOrigin: "ftp://localhost" }))).toBeNull();
  });
  it("requires same origin and a shared rate limit before contacting a supplier", async () => {
    expect((await handleMerch(request("quote", input, "https://other.example"), "quote", env)).status).toBe(403);
    env.MERCH_RATE_LIMITER = { limit: async () => ({ success: false }) };
    expect((await handleMerch(request("quote", input), "quote", env)).status).toBe(429);
    expect(sent).toHaveLength(0);
  });
  it("bounds streamed requests and ignores client prices", async () => {
    expect((await handleMerch(request("quote", { ...input, extra: "x".repeat(9000) }), "quote", env)).status).toBe(413);
    const response = await handleMerch(request("quote", { ...input, total: 1 }), "quote", env);
    expect((await response.json()).total).toBe(1725);
  });
  it("grosses contributor costs up without a profit reserve", () => {
    const c = validateQuoteInput({ ...input, productId: "contributor", contributor: { github: "123456", contribution: "", language: "en", phraseIndex: 0 } });
    const priced = priceQuote(c, config.countries.US, 5.12, 2.25);
    expect(priced).toEqual({ merchandise: 512, shipping: 225, processing: 74, total: 811 });
    expect(priceQuote(validateQuoteInput(input), config.countries.US, 5.12, 2.25).processing).toBe(0);
    expect(() => priceQuote(validateQuoteInput(input), config.countries.US, 14, 2.25)).toThrow("price review");
  });
  it("selects the exact supplier region/service and additional-piece price", () => {
    const rates = [{ regionCode: "US", levels: [{ levelCode: "EXP", firstPrice: 12, additionalPrice: 5 }, { levelCode: "STD", firstPrice: 3, additionalPrice: 1 }] }];
    expect(destinationRate(rates, "US", "STD", 3).shippingUsd).toBe(5);
    expect(() => destinationRate(rates, "CN", "STD", 1)).toThrow();
    expect(() => destinationRate(rates, "US", "UNKNOWN", 1)).toThrow();
  });
  it("pins HTTPS and cannot POST to shipping or follow arbitrary endpoints", async () => {
    await expect(yoycolRequest(env, "POST", "/api/2025/open/v4/shipping/levels")).rejects.toThrow();
    await expect(yoycolRequest(env, "GET", "https://other.example/")).rejects.toThrow();
    expect(sent).toHaveLength(0);
  });
  it("signs canonical raw sorted query values, independent of URL encoding", () => {
    const headers = yoycolHeaders("GET", "/api/2025/open/v4/shipping/sku_quotes", { z: "two words", a: "x/y" }, "key", "secret", "1700000000000", "0123456789abcdef0123456789abcdef");
    expect(headers["X-API-Signature"]).toBe("ojijq4/UJG5lcMwoYJXopBrxYJ8LTYICrzmKmiIE0jE=");
  });
});

describe("checkout and durable payment receipts", () => {
  it("creates a 15-minute quote and a stable idempotent session with separate shipping", async () => {
    const quote = await checkout();
    expect(quote.expiresAt-Date.now()).toBeGreaterThan(14*60*1000);
    expect(quote.expiresAt-Date.now()).toBeLessThanOrEqual(15*60*1000);
    const creation = sent.find(r => new URL(r.url).hostname === "api.stripe.com" && r.method === "POST")!;
    expect(creation.idempotency).toBe("codewhale-merch:" + quote.quoteId);
    const params = new URLSearchParams(creation.body);
    expect(params.get("payment_method_configuration")).toBe("pmc_fixture");
    expect(params.get("line_items[1][price_data][unit_amount]")).toBe("225");
    expect(params.get("line_items[1][price_data][product_data][name]")).toBe("Delivery · Fixture service");
    expect(params.has("shipping_options[0][shipping_rate_data][fixed_amount][amount]")).toBe(false);
    expect(params.get("payment_intent_data[shipping][address][postal_code]")).toBe("94107");
    expect(params.has("shipping_address_collection[allowed_countries][0]")).toBe(false);
    expect(Number(params.get("expires_at"))*1000-quote.expiresAt).toBeGreaterThan(29*60*1000);
    expect((await handleMerch(request("checkout", { quoteId: quote.quoteId }), "checkout", env)).status).toBe(200);
    expect(sent.filter(r => new URL(r.url).hostname === "api.stripe.com" && r.method === "POST")).toHaveLength(1);
  });
  it("rejects an expired quote and changed saved artwork", async () => {
    const quote = await quoted();
    config.products["whale-bro"]!.US.designCode = "different-art"; env.MERCH_CONFIG_JSON = JSON.stringify(config);
    expect((await handleMerch(request("checkout", { quoteId: quote.quoteId }), "checkout", env)).status).toBe(409);
    db.db.prepare("UPDATE merch_orders SET expires_at=0").run();
    expect((await handleMerch(request("checkout", { quoteId: quote.quoteId }), "checkout", env)).status).toBe(409);
  });
  it("revokes outstanding quotes when shipping, costs, fees or tax disclosure change", async () => {
    const changes = [
      () => { config.countries.US.shippingLevelCode = "REPLACEMENT"; },
      () => { config.products["whale-bro"]!.US.productionCostUsd = 14; },
      () => { config.countries.US.feeRate = .08; },
      () => { config.countries.US.taxNote = "Updated delivery charge disclosure."; },
    ];
    for (const change of changes) {
      config = configuration(); env.MERCH_CONFIG_JSON = JSON.stringify(config);
      const quote = await quoted();
      change(); env.MERCH_CONFIG_JSON = JSON.stringify(config);
      const result = await handleMerch(request("checkout", { quoteId: quote.quoteId }), "checkout", env);
      expect(result.status).toBe(409);
      expect((await result.json()).code).toBe("quote_expired");
    }
    expect(sent.filter(r => new URL(r.url).hostname === "api.stripe.com")).toHaveLength(0);
  });
  it("rejects bad signatures, test/live mismatch and unreconciled amounts", async () => {
    const quote = await checkout();
    expect((await event(undefined, undefined, false)).status).toBe(400);
    expect((await event(undefined, undefined, true, true)).status).toBe(400);
    currentSession.amount_total = 1; currentSession.payment_status = "paid";
    expect((await event()).status).toBe(409);
    expect(state(quote.quoteId)?.payment_state).toBe("quoted");
  });
  it("does not fulfill completed-but-unpaid events or a redirect", async () => {
    const quote = await checkout();
    expect(state(quote.quoteId)?.payment_state).toBe("quoted");
    expect((await event()).status).toBe(200);
    expect(state(quote.quoteId)?.fulfillment_state).toBe("not_ready");
    expect(sent.filter(r => r.method === "POST" && r.url.includes("yoycol"))).toHaveLength(0);
  });
  it("accepts delayed payment after failure once, and duplicate events do not regress it", async () => {
    const quote = await checkout(); await event("checkout.session.async_payment_failed", "evt_fail");
    expect(state(quote.quoteId)?.payment_state).toBe("failed");
    currentSession.payment_status = "paid";
    await event("checkout.session.async_payment_succeeded", "evt_paid");
    await event("checkout.session.async_payment_succeeded", "evt_paid");
    currentSession.payment_status = "unpaid"; await event("checkout.session.expired", "evt_late");
    expect(state(quote.quoteId)).toEqual(expect.objectContaining({ payment_state: "paid", fulfillment_state: "awaiting_supplier_review" }));
    expect(db.db.prepare("SELECT count(*) AS n FROM merch_webhook_events WHERE id='evt_paid'").get()?.n).toBe(1);
  });
  it("keeps webhooks working when new sales are paused", async () => {
    const quote = await checkout(); currentSession.payment_status = "paid";
    config.checkoutEnabled = false; env.MERCH_CONFIG_JSON = JSON.stringify(config);
    env.YOYCOL_ACCESS_KEY = undefined; env.MERCH_RATE_LIMITER = undefined;
    expect((await event()).status).toBe(200);
    expect(state(quote.quoteId)?.payment_state).toBe("paid");
  });
});

describe("manual supplier handoff", () => {
  async function paid(live = false) {
    if (live) { config.mode = "live"; config.siteOrigin = "https://codewhale.net"; env.MERCH_CONFIG_JSON = JSON.stringify(config); env.MERCH_STRIPE_KEY = "sk_live_fixture"; currentSession.livemode = true; }
    const quote = await checkout(); currentSession.payment_status = "paid"; await event(); return quote;
  }
  it("refuses unpaid and test-mode supplier creation", async () => {
    const quote = await checkout();
    await expect(operatorAction(env, { action: "submit", orderId: quote.quoteId })).rejects.toThrow();
    currentSession.payment_status = "paid"; await event();
    await expect(operatorAction(env, { action: "submit", orderId: quote.quoteId, confirmation: "CREATE_UNPAID_SUPPLIER_ORDER" })).rejects.toThrow();
    config.mode = "live"; config.siteOrigin = "https://codewhale.net"; config.supplierSubmissionEnabled = true; env.MERCH_CONFIG_JSON = JSON.stringify(config);
    await expect(operatorAction(env, { action: "submit", orderId: quote.quoteId, confirmation: "CREATE_UNPAID_SUPPLIER_ORDER" })).rejects.toThrow();
  });
  it("previews frozen SKU/art/address without forwarding contributor identity", async () => {
    const quote = await paid();
    const stored = JSON.parse(db.db.prepare("SELECT quote_json FROM merch_orders WHERE id=?").get(quote.quoteId)?.quote_json as string) as Quote;
    stored.input.contributor = { github: "private-identity", contribution: "https://github.com/codewhale-hq/CodeWhale/pull/123", language: "en", phraseIndex: 1 };
    const body = JSON.stringify(supplierRequest(stored));
    expect(body).not.toContain("private-identity"); expect(body).not.toContain("github.com");
    expect(body).toContain("fixture-design"); expect(body).toContain("fixture-sku");
    await operatorAction(env, { action: "preview", orderId: quote.quoteId });
    expect(sent.filter(r => r.method === "POST" && r.url.includes("yoycol"))).toHaveLength(0);
  });
  it("atomically allows one submission and keeps supplier funding separate", async () => {
    const quote = await paid(true); config.supplierSubmissionEnabled = true; env.MERCH_CONFIG_JSON = JSON.stringify(config);
    const value = { action: "submit", orderId: quote.quoteId, confirmation: "CREATE_UNPAID_SUPPLIER_ORDER" };
    const results = await Promise.allSettled([operatorAction(env, value), operatorAction(env, value)]);
    expect(results.filter(r => r.status === "fulfilled")).toHaveLength(1);
    expect(sent.filter(r => r.method === "POST" && r.url.includes("yoycol"))).toHaveLength(1);
    expect(state(quote.quoteId)?.fulfillment_state).toBe("supplier_cost_review");
  });
  it("holds an ambiguous submission instead of automatically retrying", async () => {
    const quote = await paid(true); config.supplierSubmissionEnabled = true; env.MERCH_CONFIG_JSON = JSON.stringify(config); supplierFailure = true;
    const value = { action: "submit", orderId: quote.quoteId, confirmation: "CREATE_UNPAID_SUPPLIER_ORDER" };
    await expect(operatorAction(env, value)).rejects.toThrow("Do not resubmit");
    supplierFailure = false; await expect(operatorAction(env, value)).rejects.toThrow();
    expect(state(quote.quoteId)?.fulfillment_state).toBe("reconciliation_hold");
    expect(sent.filter(r => r.method === "POST" && r.url.includes("yoycol"))).toHaveLength(1);
  });
});
