import { createHmac, randomBytes } from "node:crypto";
import { MerchError, object } from "./model";
import type { MerchEnv } from "./types";
import { readBoundedBody } from "../bounded-body";

const API = "https://www.yoycol.com";
export function yoycolHeaders(method: string, path: string, query: Record<string, string>, accessKey: string, secretKey: string, timestamp = String(Date.now()), nonce = randomBytes(16).toString("hex")) {
  const lines = ["method=" + method.toUpperCase(), "path=" + path, "timestamp=" + timestamp, "nonce=" + nonce, "accessKey=" + accessKey, "algorithm=HmacSHA256", "version=4.0"];
  const params = Object.keys(query).sort().map(key => key + "=" + query[key]).join("&");
  if (params) lines.push("params=" + params);
  return { "X-API-Access-Key": accessKey, "X-API-Timestamp": timestamp, "X-API-Nonce": nonce, "X-API-Algorithm": "HmacSHA256", "X-API-Version": "4.0", "X-API-Signature": createHmac("sha256", secretKey).update(lines.join("\n")).digest("base64") };
}
/** Official v4 API. POST orders is never retried: its external idempotency is undocumented. */
export async function yoycolRequest(env: MerchEnv, method: "GET" | "POST", path: string, query: Record<string, string> = {}, body?: unknown, send: typeof fetch = fetch): Promise<unknown> {
  if (!env.YOYCOL_ACCESS_KEY || !env.YOYCOL_SECRET_KEY || !/^\/api\/2025\/open\/v4\/(shipping\/(sku_quotes|levels)|orders(\/\d+(\/tracking)?)?)$/.test(path) || (method === "POST" && path !== "/api/2025/open/v4/orders")) throw new MerchError(503, "vendor_unavailable", "Fulfillment is not configured yet.");
  const url = new URL(path, API); Object.entries(query).forEach(([k,v]) => url.searchParams.set(k,v));
  const response = await send(url, { method, headers: { ...yoycolHeaders(method, path, query, env.YOYCOL_ACCESS_KEY, env.YOYCOL_SECRET_KEY), "Content-Type": "application/json" }, ...(body ? { body: JSON.stringify(body) } : {}), signal: AbortSignal.timeout(12000), redirect: "error" });
  if (!response.ok) throw new MerchError(503, "vendor_unavailable", "Fulfillment prices are temporarily unavailable.");
  const envelope = object(JSON.parse(new TextDecoder().decode(await readBoundedBody(response, 262144))));
  if (String(envelope.code) !== "100000") throw new MerchError(503, "vendor_unavailable", "The supplier could not confirm this request.");
  return envelope.data;
}
export function destinationRate(data: unknown, regionCode: string, levelCode: string, quantity: number) {
  if (!Array.isArray(data)) throw new MerchError(503, "quote_unavailable", "The supplier could not confirm a delivery rate.");
  const region = data.map(object).find(r => r.regionCode === regionCode);
  const levels = region?.levels;
  const level = Array.isArray(levels) ? levels.map(object).find(l => l.levelCode === levelCode) : null;
  if (!level || typeof level.firstPrice !== "number" || typeof level.additionalPrice !== "number" || !Number.isFinite(level.firstPrice) || !Number.isFinite(level.additionalPrice) || level.firstPrice < 0 || level.additionalPrice < 0 || !Number.isInteger(quantity) || quantity < 1 || quantity > 5) throw new MerchError(409, "address_unavailable", "No reviewed delivery service is available for this destination.");
  return { shippingUsd: level.firstPrice + (quantity - 1) * level.additionalPrice, name: typeof level.levelName === "string" ? level.levelName : levelCode };
}
