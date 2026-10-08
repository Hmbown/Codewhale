import { describe, expect, it } from "vitest";
import { isSlashShortcut, type ShortcutKeyEvent } from "./slash-shortcut";

function key(overrides: Partial<ShortcutKeyEvent> = {}): ShortcutKeyEvent {
  return {
    key: "/",
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    isComposing: false,
    target: { tagName: "BODY", isContentEditable: false },
    ...overrides,
  };
}

describe("isSlashShortcut (WCAG 2.1.4 character key shortcut)", () => {
  it("fires for an unmodified slash typed outside a field", () => {
    expect(isSlashShortcut(key())).toBe(true);
    expect(isSlashShortcut(key({ target: null }))).toBe(true);
  });

  it("ignores other keys", () => {
    expect(isSlashShortcut(key({ key: "?" }))).toBe(false);
  });

  it("ignores modified keys and IME composition", () => {
    expect(isSlashShortcut(key({ ctrlKey: true }))).toBe(false);
    expect(isSlashShortcut(key({ metaKey: true }))).toBe(false);
    expect(isSlashShortcut(key({ altKey: true }))).toBe(false);
    expect(isSlashShortcut(key({ isComposing: true }))).toBe(false);
  });

  it.each(["INPUT", "TEXTAREA", "SELECT", "textarea"])("leaves typing in a %s alone", (tagName) => {
    expect(isSlashShortcut(key({ target: { tagName } }))).toBe(false);
  });

  it("leaves typing in contenteditable content alone", () => {
    expect(isSlashShortcut(key({ target: { tagName: "DIV", isContentEditable: true } }))).toBe(false);
  });

  it("leaves a key an inner widget already handled alone", () => {
    expect(isSlashShortcut(key({ defaultPrevented: true }))).toBe(false);
  });

  it.each(["textbox", "searchbox", "combobox", " TextBox "])("leaves typing in a role=%s widget alone", (role) => {
    const target = { tagName: "DIV", getAttribute: (name: string) => (name === "role" ? role : null) };
    expect(isSlashShortcut(key({ target }))).toBe(false);
  });

  it("still fires over a role that is not a text widget", () => {
    const target = { tagName: "DIV", getAttribute: (name: string) => (name === "role" ? "button" : null) };
    expect(isSlashShortcut(key({ target }))).toBe(true);
  });

  it("looks through a shadow root to the element that received the key", () => {
    const host = { tagName: "MY-EDITOR" };
    const inner = { tagName: "INPUT" };
    expect(isSlashShortcut(key({ target: host, composedPath: () => [inner, host] }))).toBe(false);
    expect(isSlashShortcut(key({ target: host, composedPath: () => [host] }))).toBe(true);
  });
});
