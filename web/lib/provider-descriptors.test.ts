import { test } from "vitest";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { parseProviderDescriptors } from "./provider-descriptors.mjs";
interface TestRow {
  kind: string; id: string; label: string; default_model: string;
  env_vars: unknown[]; retired: boolean; selectable: boolean;
  web?: { order: number; label?: string; env?: string };
  tui_wire_tag: string; catalog_id: string; catalog_source_id: string;
}
interface TestData {
  schema_version: number; providers: TestRow[];
  constant_refs: Record<string, { provider: string; field: string }>;
}
const data: TestData = JSON.parse(readFileSync(new URL("../../crates/config/assets/provider_descriptors.json", import.meta.url), "utf8"));

test("shared descriptor projection preserves OAuth explanation and released stable website identities", () => {
  const facts = parseProviderDescriptors(data);
  assert.ok(facts);
  assert.equal(facts.defaultModel, "deepseek-flash");
  assert.equal(Object.keys(facts.labels).length, 50);
  assert.equal(facts.labels["siliconflow-CN"].id, "siliconflow-CN");
  assert.equal(facts.labels.openai.label, "OpenAI-compatible");
  assert.match(facts.labels["openai-codex"].env, /ChatGPT OAuth/);
  assert.deepEqual(["antigravity", "custom", "deepseek-cn"].filter(k => k in facts.labels), []);
});
test("retired metadata cannot advertise the tombstone even with hostile public overrides", () => {
  const fixture = structuredClone(data);
  const retired = fixture.providers.find(r => r.kind === "Antigravity")!;
  retired.web = { order: 0, label: "Enable retired route", env: "bad" };
  const facts = parseProviderDescriptors(fixture);
  assert.ok(facts);
  assert.equal(facts.labels.openai.id, "openai");
  retired.retired = false;
  assert.equal(parseProviderDescriptors(fixture)?.labels.openai.id, "openai");
});
test("missing rows, duplicate identities, absent public labels and malformed defaults fail closed", () => {
  const mutations: Array<(data: TestData) => void> = [
    d => d.schema_version = 2,
    d => d.providers = [],
    d => d.providers.push(d.providers[0]),
    d => delete d.providers.find(r => r.kind === "Deepseek")!.web,
    d => d.providers.find(r => r.kind === "Deepseek")!.env_vars = [123],
    d => d.constant_refs.DEFAULT_TEXT_MODEL.provider = "not-installed",
    d => d.constant_refs.DEFAULT_TEXT_MODEL.field = "base_url",
    d => d.providers.find(r => r.kind === "Openai")!.web!.order = 0,
  ];
  for (const mutate of mutations) {
    const fixture = structuredClone(data); mutate(fixture);
    assert.equal(parseProviderDescriptors(fixture), null);
  }
  assert.equal(parseProviderDescriptors("not JSON"), null);
});
test("default and public rows follow supplied exact source without borrowing build labels", () => {
  const fixture = structuredClone(data);
  const row = fixture.providers.find(r => r.kind === "Deepseek")!;
  row.default_model = "exact-remote"; row.label = "Exact remote label"; row.env_vars = ["EXACT_REMOTE_ENV"];
  const facts = parseProviderDescriptors(JSON.stringify(fixture));
  assert.ok(facts);
  assert.equal(facts.defaultModel, "exact-remote");
  assert.deepEqual(facts.labels.deepseek, { id: "deepseek", label: "Exact remote label", env: "EXACT_REMOTE_ENV" });
});
