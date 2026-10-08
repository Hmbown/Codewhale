import { MerchError, object, parseConfig } from "./model";
import { yoycolRequest } from "./yoycol";
import type { MerchEnv, Quote } from "./types";

type Row = { id: string; quote_json: string; payment_state: string; fulfillment_state: string; supplier_order_id: string | null; updated_at: number };
function id(value: unknown) {
  if (typeof value !== "string" || !/^[0-9a-f-]{36}$/.test(value)) throw new MerchError(422, "invalid_order", "Select an existing paid order.");
  return value;
}
function supplierId(value: unknown) {
  if ((typeof value === "number" && Number.isSafeInteger(value) && value > 0) || (typeof value === "string" && /^[1-9]\d{0,18}$/.test(value))) return String(value);
  throw new MerchError(409, "supplier_receipt_invalid", "Reconcile the supplier order before retrying.");
}
/** Preview uses frozen approved mappings. Contributor identity is never sent to Yoycol. */
export function supplierRequest(value: Quote) {
  const a = value.input.address;
  return {
    storeOrderSn: "CW-" + value.quoteId, shippingLevelCode: value.shippingLevelCode,
    autoSwitchShippingLevel: false, currency: "USD",
    address: { firstName: a.name, lastName: "", phone: a.phone, country: value.input.country, province: a.region, city: a.city, address1: a.line1, address2: a.line2, zip: a.postalCode },
    items: [{ thirdItemId: value.quoteId, skuCode: value.skuCode, designCode: value.designCode, quantity: value.input.quantity, source: "productTemplate" }],
  };
}
/** Only the authenticated maintainer route may invoke this handler. Funding stays in Yoycol. */
export async function operatorAction(env: MerchEnv, raw: unknown) {
  const input = object(raw), config = parseConfig(env.MERCH_CONFIG_JSON), db = env.MERCH_DB;
  if (!config || !db) throw new MerchError(503, "operator_unavailable", "Commerce storage is not configured.");
  if (input.action === "list") return db.prepare("SELECT id,payment_state,fulfillment_state,supplier_order_id,updated_at FROM merch_orders WHERE payment_state='paid' ORDER BY updated_at DESC LIMIT 100").all();
  const orderId = id(input.orderId);
  const row = await db.prepare("SELECT * FROM merch_orders WHERE id=?").bind(orderId).first<Row>();
  if (!row || row.payment_state !== "paid") throw new MerchError(409, "not_paid", "Confirm the Stripe payment before fulfillment.");
  const value = JSON.parse(row.quote_json) as Quote;
  if (input.action === "preview") return { orderId, state: row.fulfillment_state, supplierRequest: supplierRequest(value), expectedProductionUsd: value.vendorProductionUsd, estimatedShippingUsd: value.vendorShippingUsd, note: "Review address, final supplier costs and print. Creation does not pay or start production." };
  if (input.action === "submit") {
    if (config.mode !== "live" || value.mode !== "live" || config.supplierSubmissionEnabled !== true || input.confirmation !== "CREATE_UNPAID_SUPPLIER_ORDER") throw new MerchError(409, "submission_disabled", "Supplier submission requires an explicitly enabled live operator workflow and a live paid receipt.");
    const locked = await db.prepare("UPDATE merch_orders SET fulfillment_state='supplier_submitting',updated_at=? WHERE id=? AND payment_state='paid' AND fulfillment_state='awaiting_supplier_review' AND supplier_order_id IS NULL").bind(Date.now(), orderId).run();
    if (locked.meta.changes !== 1) throw new MerchError(409, "already_submitted", "This order is already submitted or needs reconciliation.");
    try {
      const receipt = object(await yoycolRequest(env, "POST", "/api/2025/open/v4/orders", {}, supplierRequest(value)));
      const vendorId = supplierId(receipt.orderId);
      if (receipt.storeOrderSn !== "CW-" + orderId || receipt.currency !== "USD" || typeof receipt.orderRealAmount !== "number" || !Number.isFinite(receipt.orderRealAmount) || receipt.orderRealAmount < 0) throw new MerchError(409, "supplier_receipt_invalid", "The supplier receipt needs reconciliation.");
      await db.prepare("UPDATE merch_orders SET supplier_order_id=?,supplier_receipt_json=?,fulfillment_state='supplier_cost_review',updated_at=? WHERE id=? AND fulfillment_state='supplier_submitting'").bind(vendorId, JSON.stringify(receipt), Date.now(), orderId).run();
      return { orderId, supplierOrderId: vendorId, state: "supplier_cost_review", receipt, note: "Review final shipping and tax; fund manually in Yoycol after approval. No supplier payment was made here." };
    } catch {
      // A timeout or malformed receipt can follow a successful remote write. Never blindly POST again.
      await db.prepare("UPDATE merch_orders SET fulfillment_state='reconciliation_hold',updated_at=? WHERE id=? AND fulfillment_state='supplier_submitting'").bind(Date.now(), orderId).run();
      throw new MerchError(409, "reconciliation_required", "Check Yoycol for CW-" + orderId + ". Do not resubmit this order.");
    }
  }
  if (input.action === "reconcile" || input.action === "refresh") {
    if (row.fulfillment_state === "supplier_submitting" && (input.action !== "reconcile" || Date.now() - row.updated_at < 60000)) throw new MerchError(409, "submission_pending", "Wait for the current submission or reconcile it after confirming the supplier receipt.");
    const vendorId = supplierId(input.action === "reconcile" ? input.supplierOrderId : row.supplier_order_id);
    const detail = object(await yoycolRequest(env, "GET", "/api/2025/open/v4/orders/" + vendorId));
    const receipt = object(detail.order);
    if (receipt.storeOrderSn !== "CW-" + orderId || supplierId(receipt.orderId) !== vendorId) throw new MerchError(409, "supplier_receipt_invalid", "That supplier order does not match this receipt.");
    const tracking = await yoycolRequest(env, "GET", "/api/2025/open/v4/orders/" + vendorId + "/tracking");
    await db.prepare("UPDATE merch_orders SET supplier_order_id=?,supplier_receipt_json=?,tracking_json=?,fulfillment_state='supplier_status_recorded',updated_at=? WHERE id=? AND payment_state='paid'").bind(vendorId, JSON.stringify(receipt), JSON.stringify(tracking), Date.now(), orderId).run();
    return { orderId, receipt, tracking, note: "Raw supplier status is recorded; numeric status codes are not interpreted as paid, shipped or delivered." };
  }
  throw new MerchError(422, "unknown_action", "Select a supported operator action.");
}
