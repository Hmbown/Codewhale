import assert from "node:assert/strict";
import test from "node:test";

import { classifyTarget, decisionFor, sensitiveField, withDecision } from "../src/policy.js";

test("http and https pages classify to an origin and a match pattern", () => {
  const classified = classifyTarget("https://example.com/a/b?c=d#e");
  assert.deepEqual(classified, {
    ok: true,
    origin: "https://example.com",
    host: "example.com",
    pattern: "https://example.com/*",
  });
});

test("a port is part of the origin, so a grant does not leak across ports", () => {
  const eight = classifyTarget("http://localhost:8080/app");
  const nine = classifyTarget("http://localhost:9090/app");
  assert.equal(eight.ok && eight.origin, "http://localhost:8080");
  assert.notEqual(eight.ok && eight.origin, nine.ok && nine.origin);
});

test("browser-internal and local schemes are refused outright", () => {
  for (const url of [
    "chrome://settings/passwords",
    "chrome-extension://abcdefghijklmnop/panel.html",
    "devtools://devtools/bundled/inspector.html",
    "file:///Users/someone/.ssh/id_rsa",
    "view-source:https://example.com",
    "javascript:alert(1)",
    "data:text/html,<h1>hi</h1>",
    "about:blank",
  ]) {
    const classified = classifyTarget(url);
    assert.equal(classified.ok, false, `${url} must not be usable`);
    assert.match(classified.reason, /Chromewhale/);
  }
});

test("the Chrome Web Store is named rather than failing opaquely", () => {
  const classified = classifyTarget("https://chromewebstore.google.com/detail/whatever");
  assert.equal(classified.ok, false);
  assert.match(classified.reason, /chromewebstore\.google\.com/);
});

test("a missing or unparseable tab URL is a refusal, not a crash", () => {
  assert.equal(classifyTarget(undefined).ok, false);
  assert.equal(classifyTarget("").ok, false);
  assert.equal(classifyTarget("not a url").ok, false);
  assert.equal(classifyTarget(42).ok, false);
});

test("an origin with no stored decision is asked about", () => {
  assert.equal(decisionFor({}, "https://example.com"), "ask");
  assert.equal(decisionFor(undefined, "https://example.com"), "ask");
  assert.equal(decisionFor({ "https://example.com": "maybe" }, "https://example.com"), "ask");
  assert.equal(decisionFor({ "https://example.com": "allow" }, "https://example.com"), "allow");
  assert.equal(decisionFor({ "https://example.com": "block" }, "https://example.com"), "block");
});

test("withDecision replaces one origin, keeps the rest, and drops junk", () => {
  const store = { "https://a.test": "allow", "https://b.test": "block", "https://c.test": "junk" };
  const next = withDecision(store, "https://a.test", "block");
  assert.deepEqual(next, { "https://b.test": "block", "https://a.test": "block" });
});

test("forgetting an origin removes it, which restores the ask", () => {
  const next = withDecision({ "https://a.test": "allow" }, "https://a.test", "ask");
  assert.deepEqual(next, {});
  assert.equal(decisionFor(next, "https://a.test"), "ask");
});

test("password, one-time-code, and card fields are never typeable", () => {
  assert.equal(sensitiveField({ tag: "input", type: "password" }).sensitive, true);
  assert.equal(sensitiveField({ tag: "input", autocomplete: "current-password" }).sensitive, true);
  assert.equal(sensitiveField({ tag: "input", autocomplete: "one-time-code" }).sensitive, true);
  assert.equal(sensitiveField({ tag: "input", autocomplete: "cc-number" }).sensitive, true);
  assert.equal(sensitiveField({ tag: "input", autocomplete: "section-billing cc-csc" }).sensitive, true);
  assert.equal(sensitiveField({ tag: "input", name: "user_password" }).sensitive, true);
  assert.equal(sensitiveField({ tag: "input", ariaLabel: "CVV" }).sensitive, true);
});

test("ordinary fields stay typeable", () => {
  assert.equal(sensitiveField({ tag: "input", type: "text", name: "query" }).sensitive, false);
  assert.equal(sensitiveField({ tag: "textarea", name: "comment" }).sensitive, false);
  assert.equal(sensitiveField({ tag: "input", autocomplete: "email" }).sensitive, false);
  assert.equal(sensitiveField({}).sensitive, false);
  assert.equal(sensitiveField(undefined).sensitive, false);
});

test("a refusal explains itself, because the model reports the reason to the user", () => {
  const verdict = sensitiveField({ tag: "input", type: "password" });
  assert.equal(verdict.sensitive, true);
  assert.match(verdict.reason, /password/);
});
