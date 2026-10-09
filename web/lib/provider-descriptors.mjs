/** Pure website projection of the Engine's committed descriptor identities. */
export function parseProviderDescriptors(input) {
  let data;
  try { data = typeof input === "string" ? JSON.parse(input) : input; } catch { return null; }
  if (!data || data.schema_version !== 3 || !Array.isArray(data.providers) || !data.providers.length) return null;
  const labels = Object.create(null);
  const ids = new Set(), kinds = new Set(), tags = new Set(), orders = new Set();
  const publicRows = [];
  for (const row of data.providers) {
    if (!row || typeof row.id !== "string" || !row.id || typeof row.kind !== "string" || !row.kind ||
        typeof row.label !== "string" || !row.label || typeof row.default_model !== "string" ||
        typeof row.retired !== "boolean" || typeof row.selectable !== "boolean" ||
        !Array.isArray(row.env_vars) || !row.env_vars.every(v => typeof v === "string") ||
        typeof row.tui_wire_tag !== "string" || !row.tui_wire_tag || tags.has(row.tui_wire_tag) ||
        typeof row.catalog_id !== "string" || !row.catalog_id ||
        typeof row.catalog_source_id !== "string" || !row.catalog_source_id ||
        ids.has(row.id) || kinds.has(row.kind)) return null;
    ids.add(row.id); kinds.add(row.kind); tags.add(row.tui_wire_tag);
    if (row.retired || row.kind === "Antigravity" || row.kind === "Custom") continue;
    if (!row.web || !Number.isSafeInteger(row.web.order) || row.web.order < 0 || orders.has(row.web.order) ||
        Object.hasOwn(row.web, "variant") ||
        (row.web.label !== undefined && typeof row.web.label !== "string") ||
        (row.web.env !== undefined && typeof row.web.env !== "string")) return null;
    orders.add(row.web.order);
    const fact = { id: row.id, label: row.web.label ?? row.label, env: row.web.env ?? row.env_vars.join(" / ") };
    labels[row.id] = fact;
    publicRows.push({ order: row.web.order, fact });
  }
  if (data.providers.some(row => !ids.has(row.catalog_id) || !ids.has(row.catalog_source_id))) return null;
  publicRows.sort((a,b) => a.order - b.order);
  if (!publicRows.length || publicRows.some((row,index) => row.order !== index)) return null;
  const ref = data.constant_refs?.DEFAULT_TEXT_MODEL;
  if (!ref || ref.field !== "default_model" || typeof ref.provider !== "string") return null;
  const defaultRow = data.providers.find(row => row.id === ref.provider);
  if (!defaultRow || defaultRow.retired || !defaultRow.default_model) return null;
  return { labels, providers: publicRows.map(row => row.fact), defaultModel: defaultRow.default_model };
}
