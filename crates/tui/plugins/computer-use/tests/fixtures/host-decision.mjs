// Test host: what Codewhale does for the plugin it spawns. The first stdin
// line carries the per-connection decision key (and the ledger key), and a
// tools/call the user approved carries an attested decision in _meta.
import crypto from "node:crypto";

export const TEST_DECISION_KEY = "11".repeat(32);
export const TEST_LEDGER_KEY = "22".repeat(32);

export function hostKeysLine({ ledger = true } = {}) {
  return JSON.stringify({
    jsonrpc: "2.0",
    method: "codewhale/host_keys",
    params: { decision_key: TEST_DECISION_KEY, ...(ledger ? { ledger_key: TEST_LEDGER_KEY } : {}) },
  }) + "\n";
}

/** tools/call params with a decision attested under the test key. */
export function attest(params, key = TEST_DECISION_KEY) {
  if (!params || typeof params.name !== "string") return params;
  const argsJson = JSON.stringify(params.arguments ?? {});
  const nonce = crypto.randomBytes(16).toString("hex");
  const mac = crypto.createHmac("sha256", Buffer.from(key, "hex"))
    .update(Buffer.concat([Buffer.from(params.name), Buffer.from([0]), Buffer.from(argsJson), Buffer.from([0]), Buffer.from(nonce)]))
    .digest("hex");
  return { ...params, _meta: { ...(params._meta ?? {}), "codewhale/user_decision": { nonce, args_json: argsJson, mac } } };
}

export const attestParams = (method, params) => (method === "tools/call" ? attest(params) : params);

/** Sign a persisted allow the way the plugin does under TEST_LEDGER_KEY. */
export function ledgerMac(computerId, key, entry) {
  return crypto.createHmac("sha256", Buffer.from(TEST_LEDGER_KEY, "hex"))
    .update(`${computerId}\0${key}\0${entry.decision}\0${entry.at ?? ""}`)
    .digest("hex");
}
