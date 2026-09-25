import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import worker from "../src/index";
import { POSTHOG_TIMEOUT_MS } from "../src/posthog";
import {
  CWC_PRODUCT_SCHEMA, ENVELOPE_FIELDS, OPERATIONS_FIELDS,
  PRODUCT_COUNTER_FIELDS, validateBatch,
} from "../src/schema";
import { goldenBatch, harness, postJson } from "./support";

const browserV2 = () => JSON.parse(readFileSync(new URL("./golden/browser-v2.json", import.meta.url), "utf8"));
const browser = () => JSON.parse(readFileSync(new URL("./golden/browser-v3.json", import.meta.url), "utf8"));
const current = () => JSON.parse(readFileSync(new URL("../../crates/telemetry/tests/golden/v2.json", import.meta.url), "utf8"));
const configured = () => ({
  ...harness().env,
  POSTHOG_HOST: "https://us.i.posthog.com",
  POSTHOG_PROJECT_TOKEN: "phc_local_test_fixture",
  POSTHOG_IP_SAFE_EGRESS_VERIFIED: "true",
});

afterEach(() => vi.unstubAllGlobals());

describe("versioned processor policy", () => {
  it("accepts the original v1 byte shape first-party only even with an active sink", async () => {
    const fetch = vi.fn(); vi.stubGlobal("fetch", fetch);
    const { env, written } = harness();
    expect((await worker.fetch(postJson(goldenBatch()), { ...env, ...configured(), TELEMETRY: env.TELEMETRY })).status).toBe(204);
    expect(written).toHaveLength(4);
    expect(fetch).not.toHaveBeenCalled();
    expect(written.every((point) => point.blobs[19] === "")).toBe(true);
  });

  it.each([undefined, 0, 3, 5, "4", true])("rejects missing or non-current consent %s before any sink", async (consent) => {
    const fetch = vi.fn(); vi.stubGlobal("fetch", fetch);
    const batch = browserV2(); batch.consent_version = consent;
    const { env, written } = harness();
    expect((await worker.fetch(postJson(batch), { ...configured(), ...env })).status).toBe(400);
    expect(written).toHaveLength(0);
    expect(fetch).not.toHaveBeenCalled();
  });

  it.each([undefined, 0, 4, 6, "5", true])("rejects missing or invalid v3 notice %s before any sink", async (notice) => {
    const fetch = vi.fn(); vi.stubGlobal("fetch", fetch);
    const batch = browser(); batch.notice_version = notice;
    const { env, written } = harness();
    expect((await worker.fetch(postJson(batch), { ...configured(), ...env })).status).toBe(400);
    expect(written).toHaveLength(0);
    expect(fetch).not.toHaveBeenCalled();
  });

  it("keeps explicit v2 consent distinct from default-on v3 notice metadata", async () => {
    const fetch = vi.fn().mockResolvedValue(new Response(null, { status: 200 }));
    vi.stubGlobal("fetch", fetch);
    const { env, written } = harness();
    for (const batch of [browserV2(), browser()]) {
      expect((await worker.fetch(postJson(batch), { ...configured(), ...env })).status).toBe(204);
    }
    expect(written.map((point) => point.blobs.slice(18))).toEqual([["2", "4"], ["3", "5"]]);
    const properties = fetch.mock.calls.map(([, init]) => JSON.parse(init.body).batch[0].properties);
    expect(properties[0]).toMatchObject({ schema_version: 2, consent_version: 4 });
    expect(properties[0]).not.toHaveProperty("notice_version");
    expect(properties[1]).toMatchObject({ schema_version: 3, notice_version: 5 });
    expect(properties[1]).not.toHaveProperty("consent_version");

    fetch.mockClear();
    for (const batch of [{ ...browserV2(), notice_version: 5 }, { ...browser(), consent_version: 4 }]) {
      expect((await worker.fetch(postJson(batch), { ...configured(), ...env })).status).toBe(400);
    }
    expect(written).toHaveLength(2);
    expect(fetch).not.toHaveBeenCalled();
  });

  it("does not retrofit legacy batches with current consent or new events", async () => {
    for (const batch of [
      { ...goldenBatch(), consent_version: 4 },
      { ...goldenBatch(), notice_version: 5 },
      { ...goldenBatch(), events: browser().events },
      { ...goldenBatch(), events: current().events.slice(-1) },
    ]) {
      expect((await worker.fetch(postJson(batch), configured())).status).toBe(400);
    }
  });
});

describe("closed aggregate schema", () => {
  it("accepts the runtime v2 fixture and both browser product versions", () => {
    expect(validateBatch(current()).ok).toBe(true);
    expect(validateBatch(browserV2()).ok).toBe(true);
    expect(validateBatch(browser()).ok).toBe(true);
  });

  it.each(PRODUCT_COUNTER_FIELDS)("requires bounded product count %s", (field) => {
    for (const invalid of [undefined, -1, 0.5, 4294967296, "1", "private work"]) {
      const batch = browser(); batch.events[0].counters[field] = invalid;
      expect(validateBatch(batch).ok).toBe(false);
    }
    const maximum = browser(); maximum.events[0].counters[field] = 4294967295;
    expect(validateBatch(maximum).ok).toBe(true);
  });

  it("rejects unknown fields, events, and prototype names", () => {
    const mutations = [
      (batch: any) => { batch.url = "https://private.invalid"; },
      (batch: any) => { batch.events[0].prompt = "private work"; },
      (batch: any) => { batch.events[0].counters.account_id = "private"; },
      (batch: any) => { batch.events[0] = { event: "toString" }; },
      (batch: any) => { batch.events[0] = { event: "$identify" }; },
    ];
    for (const mutate of mutations) {
      const batch = browser(); mutate(batch);
      expect(validateBatch(batch).ok).toBe(false);
    }
  });

  it("keeps service health on the control-plane with only aggregate u32 values", () => {
    const batch = current(); batch.events = batch.events.slice(-1);
    expect(validateBatch(batch).ok).toBe(true);
    expect(validateBatch({ ...batch, surface: "web-app" }).ok).toBe(false);
    for (const field of OPERATIONS_FIELDS) {
      const invalid = structuredClone(batch); invalid.events[0][field] = -1;
      expect(validateBatch(invalid).ok).toBe(false);
    }
    batch.events[0].requestDigest = "private";
    expect(validateBatch(batch).ok).toBe(false);
  });

  it("keeps the cross-repo JSON Schema derived from this authority", () => {
    const artifact = JSON.parse(readFileSync(new URL("../schema/cwc-product-v3.schema.json", import.meta.url), "utf8"));
    expect(artifact).toEqual(CWC_PRODUCT_SCHEMA);
    expect(Object.keys(artifact.properties).sort()).toEqual([...ENVELOPE_FIELDS].sort());
    expect(artifact.properties.surface.enum).toEqual(["web-app", "desktop"]);
    expect(Object.keys(artifact.properties.events.items.properties.counters.properties)).toEqual(PRODUCT_COUNTER_FIELDS);
    expect(artifact.additionalProperties).toBe(false);
    expect(artifact.properties.events.items.additionalProperties).toBe(false);
    expect(artifact.properties.events.items.properties.counters.additionalProperties).toBe(false);
  });
});

describe("bounded optional PostHog delivery", () => {
  it.each([undefined, "", "false", "TRUE", "1"])("requires the exact operator egress prerequisite %s", async (verified) => {
    const fetch = vi.fn(); vi.stubGlobal("fetch", fetch);
    expect((await worker.fetch(postJson(browser()), {
      ...configured(), POSTHOG_IP_SAFE_EGRESS_VERIFIED: verified,
    })).status).toBe(204);
    expect(fetch).not.toHaveBeenCalled();
  });

  it.each([
    undefined, "", "http://us.i.posthog.com", "https://posthog.com",
    "https://us.i.posthog.com/", "https://us.i.posthog.com?key=bad",
    "https://us.i.posthog.com#fragment", "https://us.i.posthog.com:443",
    "https://us.i.posthog.com.attacker.invalid", "https://us.i.posthog.com@attacker.invalid",
    "https://user:password@us.i.posthog.com", "http://127.0.0.1",
  ])("is inert with an absent/untrusted host %s", async (host) => {
    const fetch = vi.fn(); vi.stubGlobal("fetch", fetch);
    expect((await worker.fetch(postJson(browser()), { ...configured(), POSTHOG_HOST: host })).status).toBe(204);
    expect(fetch).not.toHaveBeenCalled();
  });

  it.each([undefined, "", "not_a_project_token", "phc_bad\nvalue"])("is inert with an absent/invalid project token %s", async (token) => {
    const fetch = vi.fn(); vi.stubGlobal("fetch", fetch);
    expect((await worker.fetch(postJson(browser()), { ...configured(), POSTHOG_PROJECT_TOKEN: token })).status).toBe(204);
    expect(fetch).not.toHaveBeenCalled();
  });

  it.each(["https://us.i.posthog.com", "https://eu.i.posthog.com"])("sends anonymous aggregate properties to %s without request metadata", async (host) => {
    const fetch = vi.fn().mockResolvedValue(new Response(null, { status: 200 }));
    vi.stubGlobal("fetch", fetch);
    const request = postJson(browser());
    request.headers.set("cookie", "private-cookie");
    request.headers.set("authorization", "private-credential");
    request.headers.set("user-agent", "private-agent");
    const { env, written } = harness();
    expect((await worker.fetch(request, { ...configured(), ...env, POSTHOG_HOST: host })).status).toBe(204);
    expect(written[0].blobs[17]).toBe(JSON.stringify(browser().events[0].counters));
    expect(written[0].doubles.every((count) => count === 0)).toBe(true);
    expect(fetch).toHaveBeenCalledTimes(1);
    const [url, init] = fetch.mock.calls[0];
    expect(url).toBe(`${host}/batch/`);
    expect(init).toMatchObject({ method: "POST", headers: { "content-type": "application/json" }, redirect: "manual", credentials: "omit" });
    expect(init.signal).toBeInstanceOf(AbortSignal);
    expect(init.body).not.toContain("private-");
    const captured = JSON.parse(init.body).batch;
    expect(captured).toHaveLength(1);
    expect(captured[0]).toMatchObject({
      event: "codewhale_product_usage", timestamp: browser().sent_at,
      properties: {
        schema_version: 3, notice_version: 5, surface: "website",
        distinct_id: `codewhale:${browser().install_id}`,
        $process_person_profile: false, $geoip_disable: true, $ip: null,
        counters: browser().events[0].counters,
      },
    });
    expect(Object.keys(JSON.parse(init.body)).sort()).toEqual(["api_key", "batch"]);
    expect(captured[0].properties).not.toHaveProperty("install_id");
    expect(captured[0].properties).not.toHaveProperty("consent_version");
  });

  it.each(["throws", 302, 429, 500])("isolates processor failure %s from first-party success without retry", async (failure) => {
    const fetch = failure === "throws" ? vi.fn().mockRejectedValue(new Error("private failure"))
      : vi.fn().mockResolvedValue(new Response(null, { status: failure as number }));
    vi.stubGlobal("fetch", fetch);
    const { env, written } = harness();
    expect((await worker.fetch(postJson(browser()), { ...configured(), ...env })).status).toBe(204);
    expect(written).toHaveLength(1);
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("aborts a stalled processor while preserving the first-party response", async () => {
    const fetch = vi.fn((_url, init) => new Promise((_resolve, reject) => {
      init.signal.addEventListener("abort", () => reject(init.signal.reason), { once: true });
    }));
    vi.stubGlobal("fetch", fetch);
    const start = Date.now();
    expect((await worker.fetch(postJson(browser()), configured())).status).toBe(204);
    expect(Date.now() - start).toBeGreaterThanOrEqual(POSTHOG_TIMEOUT_MS - 20);
    expect(Date.now() - start).toBeLessThan(POSTHOG_TIMEOUT_MS + 1000);
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("never combines distinct installations in one processor batch", async () => {
    const fetch = vi.fn().mockResolvedValue(new Response(null, { status: 200 }));
    vi.stubGlobal("fetch", fetch);
    const second = browser(); second.install_id = "3f2a9c1e-0000-4000-8000-000000000001";
    await Promise.all([browser(), second].map((batch) => worker.fetch(postJson(batch), configured())));
    expect(fetch).toHaveBeenCalledTimes(2);
    expect(fetch.mock.calls.map(([, init]) => JSON.parse(init.body).batch.map((event: any) => event.properties.distinct_id))).toEqual([
      [`codewhale:${browser().install_id}`], [`codewhale:${second.install_id}`],
    ]);
  });
});
