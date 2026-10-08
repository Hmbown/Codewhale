import StripeClient from "stripe";
import { randomUUID } from "node:crypto";
import { getEnv } from "../kv";
import { readBoundedBody, BodyReadError } from "../bounded-body";
import { MERCH_PRODUCTS } from "./catalog";
import { MerchError, object, parseConfig, validateQuoteInput, reviewedRoute, priceQuote } from "./model";
import { destinationRate, yoycolRequest } from "./yoycol";
import type { MerchEnv, Quote, MerchConfig, MerchDatabase } from "./types";

type OrderRow = { id: string; quote_json: string; expires_at: number; stripe_session_id: string | null; checkout_url: string | null; payment_state: string; fulfillment_state: string; supplier_order_id: string | null; tracking_json: string | null };
export async function merchEnv(): Promise<MerchEnv> {
  return await getEnv();
}
function stripe(env: MerchEnv) {
  if (!env.MERCH_STRIPE_KEY) throw new MerchError(503, "checkout_unavailable", "Checkout is not configured yet.");
  return new StripeClient(env.MERCH_STRIPE_KEY, { httpClient: StripeClient.createFetchHttpClient(), maxNetworkRetries: 1, timeout: 15000 });
}
function paymentEnvironment(env: MerchEnv) {
  const config = parseConfig(env.MERCH_CONFIG_JSON);
  if (!config || !env.MERCH_DB || !env.MERCH_STRIPE_KEY || !env.MERCH_STRIPE_WEBHOOK_SECRET) throw new MerchError(503, "checkout_unavailable", "The first drop is awaiting samples and checkout setup.");
  const liveKey = /^(?:sk|rk)_live_/.test(env.MERCH_STRIPE_KEY);
  const testKey = /^(?:sk|rk)_test_/.test(env.MERCH_STRIPE_KEY);
  if (!(config.mode === "live" ? liveKey : testKey)) throw new MerchError(503, "checkout_unavailable", "Checkout configuration needs review.");
  return { config, db: env.MERCH_DB };
}
function configured(env: MerchEnv) {
  const result = paymentEnvironment(env);
  if (!result.config.checkoutEnabled || !env.MERCH_RATE_LIMITER || !env.MERCH_PAYMENT_CONFIGURATION || !env.YOYCOL_ACCESS_KEY || !env.YOYCOL_SECRET_KEY) throw new MerchError(503, "checkout_unavailable", "The first drop is awaiting samples and checkout setup.");
  return result;
}
export function merchStatus(env: MerchEnv) {
  try {
    const { config } = configured(env);
    const readyProductIds = MERCH_PRODUCTS.filter(p => p.kind !== "concept" && p.collection === "launch" && Object.entries(config.products[p.id] ?? {}).some(([country, product]) => {
      const route = config.countries[country];
      return product.approved === true && (product.designCode || (p.id === "contributor" && Object.keys(product.contributorDesigns ?? {}).length > 0)) && Object.keys(product.skuBySize ?? {}).length && route?.taxReviewed === true && route.addressPolicyReviewed === true && route.supplierPricesUsdReviewed === true && route.allowedPostalPrefixes?.length;
    })).map(p => p.id);
    return { checkoutConfigured: readyProductIds.length > 0, readyProductIds };
  } catch { return { checkoutConfigured: false, readyProductIds: [] }; }
}
function json(data: unknown, status = 200) {
  return Response.json(data, { status, headers: { "Cache-Control": "no-store" } });
}
async function limited(env: MerchEnv, request: Request, config: MerchConfig) {
  const origin = request.headers.get("origin");
  if (!origin || origin !== new URL(config.siteOrigin).origin || !request.headers.get("content-type")?.startsWith("application/json")) throw new MerchError(403, "request_blocked", "Refresh the page before trying again.");
  // Shared limiter never becomes a permissive per-isolate in-memory fallback.
  const result = await env.MERCH_RATE_LIMITER!.limit({ key: "codewhale-merch:public" });
  if (!result.success) throw new MerchError(429, "try_later", "Please wait a minute and try again.");
}
async function body(request: Request) {
  try { return JSON.parse(new TextDecoder().decode(await readBoundedBody(request, 8192))) as unknown; }
  catch (error) {
    if (error instanceof BodyReadError) throw new MerchError(error.status, "invalid_request", error.message);
    throw new MerchError(422, "invalid_request", "Check the form and try again.");
  }
}
async function row(db: MerchDatabase, id: string) {
  return db.prepare("SELECT * FROM merch_orders WHERE id = ?").bind(id).first<OrderRow>();
}
function safeId(value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value)) throw new MerchError(422, "invalid_quote", "Request a fresh shipping quote.");
  return value;
}
function publicQuote(quote: Quote) {
  const { quoteId, expiresAt, currency, merchandise, shipping, processing, total, shippingName, taxNote } = quote;
  return { quoteId, expiresAt, currency, merchandise, shipping, processing, total, shippingName, taxNote };
}
async function quote(request: Request, env: MerchEnv) {
  const { config, db } = configured(env); await limited(env, request, config);
  const input = validateQuoteInput(await body(request)), { route, product, designCode } = reviewedRoute(config, input);
  // API provides destination/SKU rates, not exact-address or mixed-cart landed quotes.
  const skuCode = product.skuBySize[input.size];
  const rate = destinationRate(await yoycolRequest(env, "GET", "/api/2025/open/v4/shipping/sku_quotes", { sku_code: skuCode }), route.regionCode, route.shippingLevelCode, input.quantity);
  const prices = priceQuote(input, route, product.productionCostUsd * input.quantity, rate.shippingUsd);
  const value: Quote = { quoteId: randomUUID(), mode: config.mode, routeApproval: JSON.stringify([route, product]), expiresAt: Date.now() + 15 * 60 * 1000, currency: route.currency, ...prices, shippingName: rate.name, taxNote: route.taxNote, input, skuCode, designCode, shippingLevelCode: route.shippingLevelCode, vendorProductionUsd: product.productionCostUsd * input.quantity, vendorShippingUsd: rate.shippingUsd };
  await db.prepare("INSERT INTO merch_orders (id,quote_json,expires_at,updated_at) VALUES (?,?,?,?)").bind(value.quoteId, JSON.stringify(value), value.expiresAt, Date.now()).run();
  return json(publicQuote(value));
}
async function checkout(request: Request, env: MerchEnv) {
  const { config, db } = configured(env); await limited(env, request, config);
  const id = safeId(object(await body(request)).quoteId), order = await row(db, id);
  if (!order || order.expires_at <= Date.now() || order.payment_state !== "quoted") throw new MerchError(409, "quote_expired", "Please request a fresh quote.");
  const value = JSON.parse(order.quote_json) as Quote;
  if (value.mode !== config.mode) throw new MerchError(409, "quote_expired", "Please request a fresh quote.");
  const reviewed = reviewedRoute(config, value.input);
  if (value.routeApproval !== JSON.stringify([reviewed.route, reviewed.product])) throw new MerchError(409, "quote_expired", "The print or delivery review changed. Please request a fresh quote.");
  if (order.checkout_url) return json({ url: order.checkout_url });
  const product = MERCH_PRODUCTS.find(p => p.id === value.input.productId)!;
  const client = stripe(env);
  const session = await client.checkout.sessions.create({
    mode: "payment", client_reference_id: id, expires_at: Math.floor((value.expiresAt + 30 * 60 * 1000) / 1000),
    integration_identifier: "codewhale_merch_" + Array.from(id.replaceAll("-", "").slice(0, 8), n => String.fromCharCode(97 + parseInt(n, 16))).join(""),
    payment_method_configuration: env.MERCH_PAYMENT_CONFIGURATION,
    line_items: [
      { price_data: { currency: value.currency, unit_amount: value.merchandise + value.processing, product_data: { name: "Codewhale · " + product.title, description: value.input.size + " · " + value.input.color + " · " + value.input.quantity + " item(s)" + (value.processing ? " · cost-recovery processing included" : "") } }, quantity: 1 },
      { price_data: { currency: value.currency, unit_amount: value.shipping, product_data: { name: "Delivery · " + value.shippingName } }, quantity: 1 },
    ],
    // Delivery is an explicit line item: no dependency on Stripe's address collector.
    // Do not collect a different shipping address after the destination quote.
    payment_intent_data: { shipping: { name: value.input.address.name, phone: value.input.address.phone, address: { line1: value.input.address.line1, line2: value.input.address.line2 || undefined, city: value.input.address.city, state: value.input.address.region || undefined, postal_code: value.input.address.postalCode || undefined, country: value.input.country } }, metadata: { merch_order_id: id } },
    metadata: { merch_order_id: id, merch_product_id: product.id },
    custom_text: { submit: { message: "Ships to the address provided on Codewhale. " + value.taxNote } },
    success_url: new URL("/en/merch?payment=received", config.siteOrigin).href,
    cancel_url: new URL("/en/merch?payment=cancelled", config.siteOrigin).href,
  }, { idempotencyKey: "codewhale-merch:" + id });
  if (session.livemode !== (config.mode === "live") || !session.url || new URL(session.url).hostname !== "checkout.stripe.com" || !session.url.startsWith("https:")) throw new MerchError(503, "checkout_unavailable", "Checkout needs review.");
  await db.prepare("UPDATE merch_orders SET stripe_session_id=?,checkout_url=?,updated_at=? WHERE id=? AND payment_state='quoted'").bind(session.id, session.url, Date.now(), id).run();
  return json({ url: session.url });
}
async function webhook(request: Request, env: MerchEnv) {
  // Pausing new sales must not discard payment events for existing sessions.
  const { config, db } = paymentEnvironment(env), client = stripe(env);
  let event: StripeClient.Event;
  try {
    const raw = new TextDecoder().decode(await readBoundedBody(request, 262144));
    event = await client.webhooks.constructEventAsync(raw, request.headers.get("stripe-signature") ?? "", env.MERCH_STRIPE_WEBHOOK_SECRET!, undefined, StripeClient.createSubtleCryptoProvider());
  } catch { throw new MerchError(400, "invalid_signature", "Webhook signature verification failed."); }
  if (event.livemode !== (config.mode === "live")) throw new MerchError(400, "wrong_mode", "Webhook mode mismatch.");
  if (!["checkout.session.completed", "checkout.session.async_payment_succeeded", "checkout.session.async_payment_failed", "checkout.session.expired"].includes(event.type)) return json({ received: true });
  const snapshot = event.data.object as StripeClient.Checkout.Session;
  const id = snapshot.metadata?.merch_order_id;
  if (!id) return json({ received: true });
  const order = await row(db, safeId(id));
  if (!order || order.stripe_session_id !== snapshot.id) throw new MerchError(409, "order_mismatch", "The payment needs reconciliation.");
  const value = JSON.parse(order.quote_json) as Quote;
  const session = await client.checkout.sessions.retrieve(snapshot.id, { expand: ["line_items"] });
  if (value.mode !== config.mode || session.livemode !== (config.mode === "live") || session.client_reference_id !== id || session.metadata?.merch_order_id !== id || session.metadata?.merch_product_id !== value.input.productId || session.currency !== value.currency || session.amount_total !== value.total) throw new MerchError(409, "amount_mismatch", "The payment needs reconciliation.");
  if (session.payment_status === "paid") {
    await db.batch([
      db.prepare("INSERT OR IGNORE INTO merch_webhook_events (id,order_id,kind,received_at) VALUES (?,?,?,?)").bind(event.id, id, event.type, Date.now()),
      db.prepare("UPDATE merch_orders SET payment_state='paid',fulfillment_state='awaiting_supplier_review',updated_at=? WHERE id=? AND payment_state IN ('quoted','failed','expired')").bind(Date.now(), id),
    ]);
  } else if (event.type === "checkout.session.async_payment_failed" || event.type === "checkout.session.expired") {
    await db.batch([
      db.prepare("INSERT OR IGNORE INTO merch_webhook_events (id,order_id,kind,received_at) VALUES (?,?,?,?)").bind(event.id, id, event.type, Date.now()),
      db.prepare("UPDATE merch_orders SET payment_state=?,updated_at=? WHERE id=? AND payment_state='quoted'").bind(event.type.endsWith("expired") ? "expired" : "failed", Date.now(), id),
    ]);
  }
  return json({ received: true });
}
/** Source-ready entrypoints. No supplier order/payment is triggered by a redirect or webhook. */
export async function handleMerch(request: Request, action: "status" | "quote" | "checkout" | "webhook", suppliedEnv?: MerchEnv): Promise<Response> {
  try {
    const env = suppliedEnv ?? await merchEnv();
    if (action === "status") return json(merchStatus(env));
    if (action === "quote") return await quote(request, env);
    if (action === "checkout") return await checkout(request, env);
    return await webhook(request, env);
  } catch (error) {
    if (error instanceof MerchError) return json({ error: error.message, code: error.code }, error.status);
    return json({ error: "This request could not be completed. Please try again later.", code: "temporarily_unavailable" }, 503);
  }
}
