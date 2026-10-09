-- Separate commerce database: do not apply to the community KV or product balances.
CREATE TABLE IF NOT EXISTS merch_orders (
  id TEXT PRIMARY KEY,
  quote_json TEXT NOT NULL,
  expires_at INTEGER NOT NULL,
  stripe_session_id TEXT UNIQUE,
  checkout_url TEXT,
  payment_state TEXT NOT NULL DEFAULT 'quoted',
  fulfillment_state TEXT NOT NULL DEFAULT 'not_ready',
  supplier_order_id TEXT UNIQUE,
  supplier_receipt_json TEXT,
  tracking_json TEXT,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS merch_webhook_events (
  id TEXT PRIMARY KEY,
  order_id TEXT NOT NULL REFERENCES merch_orders(id),
  kind TEXT NOT NULL,
  received_at INTEGER NOT NULL
);
