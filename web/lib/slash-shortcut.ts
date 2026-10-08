/**
 * The "/" focus-search shortcut shared by the FAQ and docs search boxes.
 *
 * A single-character shortcut must not steal keys the user is typing
 * elsewhere (WCAG 2.1.4 Character Key Shortcuts): it ignores modified keys,
 * IME composition, keys an inner widget already handled (defaultPrevented),
 * and any event aimed at an editable control — native fields,
 * contenteditable, or an ARIA text widget — including one inside a shadow
 * root, which the window listener would otherwise only see as its host.
 */

type ShortcutTarget = {
  tagName?: string;
  isContentEditable?: boolean;
  getAttribute?: (name: string) => string | null;
} | null;

export type ShortcutKeyEvent = Pick<
  KeyboardEvent,
  "key" | "ctrlKey" | "metaKey" | "altKey" | "isComposing"
> & {
  target: EventTarget | ShortcutTarget | null;
  defaultPrevented?: boolean;
  composedPath?: () => ReadonlyArray<EventTarget | ShortcutTarget>;
};

const EDITABLE_TAGS = new Set(["INPUT", "TEXTAREA", "SELECT"]);
const EDITABLE_ROLES = new Set(["textbox", "searchbox", "combobox"]);

export function isEditableTarget(target: EventTarget | ShortcutTarget | null): boolean {
  if (!target || typeof target !== "object") return false;
  const el = target as ShortcutTarget & object;
  if (el.isContentEditable) return true;
  if (typeof el.tagName === "string" && EDITABLE_TAGS.has(el.tagName.toUpperCase())) return true;
  const role = typeof el.getAttribute === "function" ? el.getAttribute("role") : null;
  return role !== null && EDITABLE_ROLES.has(role.trim().toLowerCase());
}

export function isSlashShortcut(e: ShortcutKeyEvent): boolean {
  if (e.key !== "/") return false;
  if (e.defaultPrevented) return false;
  if (e.ctrlKey || e.metaKey || e.altKey || e.isComposing) return false;
  const origin = e.composedPath?.()[0] ?? e.target;
  return !isEditableTarget(origin);
}
