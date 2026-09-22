/**
 * The four functions Chromewhale injects into a granted page.
 *
 * `chrome.scripting.executeScript({ func })` serializes the function source and
 * re-evaluates it in the target frame, so **each function here must be
 * self-contained**: no imports, no module-scope references, every helper nested
 * inside. Adding a shared top-level helper and calling it from one of these
 * silently breaks injection at runtime, not at load.
 *
 * They run in the default ISOLATED world, whose globals survive between
 * `executeScript` calls on the same document but are thrown away on navigation.
 * That is exactly the lifetime a snapshot's element refs should have: after a
 * page load the refs are gone, and a click against them reports staleness
 * instead of hitting whatever element now sits at that index.
 *
 * Known limitations:
 * - Only the top frame is walked. Text and controls inside cross-origin iframes
 *   are invisible to a snapshot, and a ref can never point into one.
 * - Open shadow roots are traversed; closed ones are not reachable by design.
 * - `element.click()` dispatches an untrusted event. Sites that gate on
 *   `event.isTrusted` will ignore it, and nothing here can change that.
 */

/**
 * Walk the visible top frame, numbering interactive elements.
 *
 * @param {number} budget maximum characters of outline to return
 * @returns {{url: string, title: string, outline: string, refCount: number,
 *            truncated: boolean}}
 */
export function snapshotPage(budget) {
  const INTERACTIVE =
    'a[href],button,input,select,textarea,summary,[role="button"],[role="link"],' +
    '[role="tab"],[role="checkbox"],[role="radio"],[role="switch"],[role="menuitem"],' +
    '[role="option"],[role="searchbox"],[role="textbox"],[contenteditable=""],' +
    '[contenteditable="true"]';
  const SKIP = new Set([
    "SCRIPT",
    "STYLE",
    "NOSCRIPT",
    "TEMPLATE",
    "HEAD",
    "SVG",
    "CANVAS",
    "IFRAME",
    "OBJECT",
    "EMBED",
    "AUDIO",
    "VIDEO",
  ]);
  const MAX_ELEMENTS = 8000;

  const refs = [];
  const lines = [];
  let used = 0;
  let truncated = false;
  let seen = 0;

  const store = (globalThis.__chromewhale = globalThis.__chromewhale || { refs: [] });

  function collapse(value) {
    return String(value == null ? "" : value)
      .replace(/\s+/g, " ")
      .trim();
  }

  function clip(value, max) {
    const text = collapse(value);
    return text.length <= max ? text : `${text.slice(0, max)}…`;
  }

  function visible(element) {
    if (typeof element.checkVisibility === "function") {
      if (!element.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true })) {
        return false;
      }
    }
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }

  function accessibleName(element) {
    const aria = element.getAttribute("aria-label");
    if (aria) {
      return clip(aria, 120);
    }
    const labelledBy = element.getAttribute("aria-labelledby");
    if (labelledBy) {
      const parts = labelledBy
        .split(/\s+/)
        .map((id) => document.getElementById(id))
        .filter(Boolean)
        .map((node) => node.textContent);
      if (parts.length) {
        return clip(parts.join(" "), 120);
      }
    }
    if (element.labels && element.labels.length) {
      const labelled = clip(
        Array.from(element.labels)
          .map((label) => label.textContent)
          .join(" "),
        120,
      );
      if (labelled) {
        return labelled;
      }
    }
    for (const attribute of ["alt", "placeholder", "title", "name"]) {
      const value = element.getAttribute(attribute);
      if (value) {
        return clip(value, 120);
      }
    }
    return clip(element.textContent, 120);
  }

  function sensitive(element) {
    const type = (element.getAttribute("type") || "").toLowerCase();
    if (type === "password") {
      return true;
    }
    const autocomplete = (element.getAttribute("autocomplete") || "").toLowerCase();
    return /password|one-time-code|cc-number|cc-csc|cc-exp/.test(autocomplete);
  }

  function describe(element) {
    const tag = element.tagName.toLowerCase();
    const role = element.getAttribute("role");
    const type = (element.getAttribute("type") || "").toLowerCase();
    let kind = role || tag;
    if (tag === "a") {
      kind = "link";
    } else if (tag === "input") {
      kind = `input(${type || "text"})`;
    }
    const parts = [kind];
    const label = accessibleName(element);
    if (label) {
      parts.push(`"${label}"`);
    }
    if (tag === "a") {
      const href = element.getAttribute("href");
      if (href) {
        parts.push(`-> ${clip(href, 80)}`);
      }
    }
    if ((tag === "input" || tag === "textarea") && !sensitive(element)) {
      const value = clip(element.value, 60);
      if (value) {
        parts.push(`value="${value}"`);
      }
    }
    if (sensitive(element)) {
      parts.push("(protected)");
    }
    if (element.checked === true) {
      parts.push("checked");
    }
    if (element.disabled === true) {
      parts.push("disabled");
    }
    if (tag === "select") {
      const selected = clip(element.options?.[element.selectedIndex]?.text, 60);
      if (selected) {
        parts.push(`selected="${selected}"`);
      }
    }
    return parts.join(" ");
  }

  function push(line) {
    if (truncated || !line) {
      return;
    }
    if (used + line.length + 1 > budget) {
      truncated = true;
      return;
    }
    lines.push(line);
    used += line.length + 1;
  }

  function walk(element) {
    if (truncated || seen >= MAX_ELEMENTS) {
      return;
    }
    seen += 1;
    if (SKIP.has(element.tagName) || element.getAttribute("aria-hidden") === "true") {
      return;
    }
    if (!visible(element)) {
      return;
    }
    if (element.matches(INTERACTIVE)) {
      refs.push(element);
      push(`[e${refs.length}] ${describe(element)}`);
      return;
    }
    if (/^H[1-6]$/.test(element.tagName)) {
      const heading = clip(element.textContent, 200);
      if (heading) {
        push(`${"#".repeat(Number(element.tagName[1]))} ${heading}`);
      }
      return;
    }
    const root = element.shadowRoot;
    const children = root ? [...root.childNodes, ...element.childNodes] : element.childNodes;
    for (const child of children) {
      if (truncated) {
        return;
      }
      if (child.nodeType === 3) {
        push(clip(child.nodeValue, 400));
      } else if (child.nodeType === 1) {
        walk(child);
      }
    }
  }

  if (document.body) {
    walk(document.body);
  }
  store.refs = refs;

  return {
    url: location.href,
    title: document.title,
    outline: lines.join("\n"),
    refCount: refs.length,
    truncated,
  };
}

/**
 * Report what an element ref points at, without touching it.
 *
 * The typing guard runs on this descriptor in `policy.js`, where it is pure and
 * unit-tested, rather than inside the page where it could only be asserted
 * through a browser.
 *
 * @param {string} ref
 */
export function inspectRef(ref) {
  const store = globalThis.__chromewhale;
  const index = Number.parseInt(String(ref).replace(/^e/i, ""), 10);
  if (!store || !Array.isArray(store.refs) || store.refs.length === 0) {
    return { ok: false, stale: true, error: "No snapshot for this page. Call browser_snapshot first." };
  }
  const element = Number.isFinite(index) ? store.refs[index - 1] : undefined;
  if (!element) {
    return { ok: false, error: `Unknown element ref "${ref}". Call browser_snapshot again.` };
  }
  if (!element.isConnected) {
    return { ok: false, stale: true, error: `Element ${ref} is no longer on the page. Snapshot again.` };
  }
  return {
    ok: true,
    tag: element.tagName.toLowerCase(),
    type: (element.getAttribute("type") || "").toLowerCase(),
    autocomplete: element.getAttribute("autocomplete") || "",
    name: element.getAttribute("name") || "",
    id: element.id || "",
    ariaLabel: element.getAttribute("aria-label") || "",
    editable:
      element.isContentEditable === true ||
      element.tagName === "INPUT" ||
      element.tagName === "TEXTAREA",
    disabled: element.disabled === true,
    label: (element.getAttribute("aria-label") || element.textContent || "")
      .replace(/\s+/g, " ")
      .trim()
      .slice(0, 120),
  };
}

/**
 * Click the element a ref points at.
 *
 * @param {string} ref
 */
export function clickRef(ref) {
  const store = globalThis.__chromewhale;
  const index = Number.parseInt(String(ref).replace(/^e/i, ""), 10);
  if (!store || !Array.isArray(store.refs) || store.refs.length === 0) {
    return { ok: false, error: "No snapshot for this page. Call browser_snapshot first." };
  }
  const element = Number.isFinite(index) ? store.refs[index - 1] : undefined;
  if (!element) {
    return { ok: false, error: `Unknown element ref "${ref}". Call browser_snapshot again.` };
  }
  if (!element.isConnected) {
    return { ok: false, error: `Element ${ref} is no longer on the page. Snapshot again.` };
  }
  if (element.disabled === true) {
    return { ok: false, error: `Element ${ref} is disabled.` };
  }
  const label = (element.getAttribute("aria-label") || element.textContent || "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 120);
  element.scrollIntoView({ block: "center", inline: "center" });
  element.focus?.();
  element.click();
  return { ok: true, label, url: location.href };
}

/**
 * Type into the element a ref points at.
 *
 * The password check is repeated here as defence in depth. The richer policy
 * lives in `policy.js` and runs before this is injected; this copy exists so a
 * future caller that forgets the pre-check still cannot fill a password box.
 *
 * @param {string} ref
 * @param {string} text
 * @param {boolean} clear
 * @param {boolean} submit
 */
export function typeRef(ref, text, clear, submit) {
  const store = globalThis.__chromewhale;
  const index = Number.parseInt(String(ref).replace(/^e/i, ""), 10);
  if (!store || !Array.isArray(store.refs) || store.refs.length === 0) {
    return { ok: false, error: "No snapshot for this page. Call browser_snapshot first." };
  }
  const element = Number.isFinite(index) ? store.refs[index - 1] : undefined;
  if (!element || !element.isConnected) {
    return { ok: false, error: `Element ${ref} is not on the page. Call browser_snapshot again.` };
  }
  if ((element.getAttribute("type") || "").toLowerCase() === "password") {
    return { ok: false, error: "Chromewhale never types into a password field." };
  }
  if (element.disabled === true || element.readOnly === true) {
    return { ok: false, error: `Element ${ref} does not accept input.` };
  }
  element.scrollIntoView({ block: "center", inline: "center" });
  element.focus();
  if (element.isContentEditable) {
    if (clear) {
      element.textContent = "";
    }
    element.textContent += text;
  } else {
    if (clear) {
      element.value = "";
    }
    element.value += text;
  }
  element.dispatchEvent(new Event("input", { bubbles: true }));
  element.dispatchEvent(new Event("change", { bubbles: true }));
  if (submit) {
    for (const type of ["keydown", "keypress", "keyup"]) {
      element.dispatchEvent(
        new KeyboardEvent(type, { key: "Enter", code: "Enter", keyCode: 13, bubbles: true }),
      );
    }
    const form = element.form;
    if (form && typeof form.requestSubmit === "function") {
      form.requestSubmit();
    }
  }
  return { ok: true, url: location.href };
}
