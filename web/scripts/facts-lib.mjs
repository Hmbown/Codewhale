/**
 * facts-lib.mjs — shared derivation logic for website fact generation and
 * drift checking. Imported by both derive-facts.mjs (prebuild) and
 * check-facts.mjs (CI gate).
 *
 * Sources of truth:
 *   - <repo>/Cargo.toml                         → version, workspace crates
 *   - <repo>/crates/tui/src/sandbox/mod.rs      → enforced sandbox markers
 *   - <repo>/crates/tui/src/config.rs           → provider list (ApiProvider enum), DEFAULT_TEXT_MODEL
 *   - <repo>/npm/codewhale/package.json         → node engines
 *   - <repo>/crates/tui/src/tools/*.rs          → tool count (ToolSpec impls)
 *   - <repo>/LICENSE                            → license
 *   - <repo>/web/data/latest-published-release.json → latest published release
 */
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
// __dirname is web/scripts; REPO_ROOT is the workspace root (two levels up).
export const REPO_ROOT = resolve(__dirname, "..", "..");

function read(rel) {
  const p = join(REPO_ROOT, rel);
  if (!existsSync(p)) return null;
  return readFileSync(p, "utf-8");
}

export function deriveVersion() {
  const cargo = read("Cargo.toml");
  if (!cargo) return null;
  const m = cargo.match(/^version\s*=\s*"([^"]+)"/m);
  return m ? m[1] : null;
}

export function deriveCrates() {
  const cargo = read("Cargo.toml");
  if (!cargo) return [];
  const block = cargo.match(/members\s*=\s*\[([\s\S]*?)\]/);
  if (!block) return [];
  return [...block[1].matchAll(/"crates\/([^"]+)"/g)].map((m) => m[1]).sort();
}

export function deriveSandboxBackends() {
  const source = read("crates/tui/src/sandbox/mod.rs");
  return source ? deriveSandboxBackendsFromSource(source) : [];
}

export function deriveSandboxBackendsFromSource(source) {
  const marker = source.match(
    /pub const PUBLIC_SANDBOX_BACKENDS\s*:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\];/,
  );
  if (!marker) return [];
  return [...marker[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]);
}

/**
 * Provider label map — the single source of truth for provider → website
 * display mapping. MUST be kept in sync with the copy in
 * web/lib/facts-drift.ts (for the runtime Cloudflare cron path).
 *
 * Excluded variants: DeepseekCN (not wired through shared ProviderKind,
 * #1104), Custom (dynamic meta-provider, #1519), and Antigravity
 * (a non-runnable legacy config tombstone, permanently excluded from public
 * provider facts).
 */
const PROVIDER_LABEL_MAP = {
  Deepseek: { id: "deepseek", label: "DeepSeek", env: "DEEPSEEK_API_KEY" },
  DeepseekAnthropic: { id: "deepseek-anthropic", label: "DeepSeek Anthropic", env: "DEEPSEEK_API_KEY / ANTHROPIC_API_KEY" },
  NvidiaNim: { id: "nvidia-nim", label: "NVIDIA NIM", env: "NVIDIA_API_KEY / NVIDIA_NIM_API_KEY" },
  Openai: { id: "openai", label: "OpenAI-compatible", env: "OPENAI_API_KEY" },
  Atlascloud: { id: "atlascloud", label: "AtlasCloud", env: "ATLASCLOUD_API_KEY" },
  WanjieArk: { id: "wanjie-ark", label: "Wanjie Ark", env: "WANJIE_ARK_API_KEY / WANJIE_API_KEY / WANJIE_MAAS_API_KEY" },
  Volcengine: { id: "volcengine", label: "Volcengine Ark", env: "VOLCENGINE_API_KEY / VOLCENGINE_ARK_API_KEY / ARK_API_KEY" },
  Openrouter: { id: "openrouter", label: "OpenRouter", env: "OPENROUTER_API_KEY" },
  Orcarouter: { id: "orcarouter", label: "OrcaRouter", env: "ORCAROUTER_API_KEY" },
  XiaomiMimo: { id: "xiaomi-mimo", label: "Xiaomi MiMo", env: "XIAOMI_MIMO_TOKEN_PLAN_API_KEY / MIMO_TOKEN_PLAN_API_KEY / XIAOMI_MIMO_API_KEY / XIAOMI_API_KEY / MIMO_API_KEY" },
  Novita: { id: "novita", label: "Novita AI", env: "NOVITA_API_KEY" },
  Fireworks: { id: "fireworks", label: "Fireworks AI", env: "FIREWORKS_API_KEY" },
  Siliconflow: { id: "siliconflow", label: "SiliconFlow", env: "SILICONFLOW_API_KEY" },
  SiliconflowCn: { id: "siliconflow-CN", label: "SiliconFlow CN", env: "SILICONFLOW_API_KEY" },
  Arcee: { id: "arcee", label: "Arcee AI", env: "ARCEE_API_KEY" },
  Moonshot: { id: "moonshot", label: "Moonshot/Kimi", env: "MOONSHOT_API_KEY / KIMI_API_KEY" },
  Sglang: { id: "sglang", label: "SGLang", env: "SGLANG_API_KEY" },
  Vllm: { id: "vllm", label: "vLLM", env: "VLLM_API_KEY" },
  Ollama: { id: "ollama", label: "Ollama", env: "OLLAMA_API_KEY" },
  OllamaCloud: { id: "ollama-cloud", label: "Ollama Cloud", env: "OLLAMA_CLOUD_API_KEY / OLLAMA_API_KEY" },
  Huggingface: { id: "huggingface", label: "Hugging Face", env: "HUGGINGFACE_API_KEY / HF_TOKEN" },
  Deepinfra: { id: "deepinfra", label: "DeepInfra", env: "DEEPINFRA_API_KEY / DEEPINFRA_TOKEN" },
  Together: { id: "together", label: "Together AI", env: "TOGETHER_API_KEY" },
  Qianfan: { id: "qianfan", label: "Baidu Qianfan", env: "QIANFAN_API_KEY / BAIDU_QIANFAN_API_KEY" },
  OpenaiCodex: { id: "openai-codex", label: "OpenAI Codex", env: "ChatGPT OAuth via `codewhale auth chatgpt`; optional consented Codex CLI credentials (OPENAI_CODEX_ACCESS_TOKEN / CODEX_ACCESS_TOKEN override)" },
  OpencodeGo: { id: "opencode-go", label: "OpenCode Go", env: "OPENCODE_GO_API_KEY" },
  OpencodeZen: { id: "opencode-zen", label: "OpenCode Zen", env: "OPENCODE_ZEN_API_KEY / OPENCODE_API_KEY" },
  Anthropic: { id: "anthropic", label: "Anthropic", env: "ANTHROPIC_API_KEY" },
  Zai: { id: "zai", label: "Z.ai", env: "ZAI_API_KEY / Z_AI_API_KEY" },
  Stepfun: { id: "stepfun", label: "StepFun", env: "STEPFUN_API_KEY / STEP_API_KEY" },
  Minimax: { id: "minimax", label: "MiniMax", env: "MINIMAX_API_KEY" },
  MinimaxAnthropic: { id: "minimax-anthropic", label: "MiniMax (Anthropic-compatible)", env: "MINIMAX_API_KEY" },
  Openmodel: { id: "openmodel", label: "OpenModel", env: "OPENMODEL_API_KEY" },
  Sakana: { id: "sakana", label: "Sakana AI", env: "FUGU_API_KEY / SAKANA_API_KEY" },
  LongCat: { id: "longcat", label: "Meituan LongCat", env: "LONGCAT_API_KEY" },
  Meta: { id: "meta", label: "Meta Model API", env: "META_MODEL_API_KEY / MODEL_API_KEY" },
  Telecomjs: { id: "telecomjs", label: "TelecomJS TokenHub", env: "TELECOMJS_API_KEY" },
  Xai: { id: "xai", label: "xAI", env: "XAI_API_KEY" },
  Mistral: { id: "mistral", label: "Mistral AI", env: "MISTRAL_API_KEY" },
  Google: { id: "google", label: "Google Gemini", env: "GOOGLE_API_KEY / GEMINI_API_KEY" },
  Edenai: { id: "edenai", label: "Eden AI", env: "EDENAI_API_KEY" },
  Concentrate: { id: "concentrate", label: "Concentrate", env: "CONCENTRATE_API_KEY" },
  Codewhale: { id: "codewhale", label: "Codewhale", env: "CODEWHALE_API_KEY" },
  ModelstudioTokenPlan: { id: "modelstudio-token-plan", label: "Model Studio Token Plan", env: "MODELSTUDIO_API_KEY" },
  ModelstudioTokenPlanAnthropic: { id: "modelstudio-token-plan-anthropic", label: "Model Studio Token Plan (Anthropic-compatible)", env: "MODELSTUDIO_API_KEY" },
  ModelstudioCodingPlan: { id: "modelstudio-coding-plan", label: "Model Studio Coding Plan", env: "MODELSTUDIO_API_KEY" },
  ModelstudioCodingPlanAnthropic: { id: "modelstudio-coding-plan-anthropic", label: "Model Studio Coding Plan (Anthropic-compatible)", env: "MODELSTUDIO_API_KEY" },
};

// DeepseekCN: not wired through shared ProviderKind (#1104).
// Custom: the dynamic OpenAI-compatible meta-provider (#1519) — a runtime
// catch-all for user-defined endpoints, not a website-listable provider.
// Antigravity is a non-runnable legacy config tombstone, never a website
// provider.
const EXCLUDED_PROVIDERS = new Set(["DeepseekCN", "Custom", "Antigravity"]);

function providerEnumVariants() {
  const cfg = read("crates/tui/src/config.rs");
  if (!cfg) return [];
  const enumBlock = cfg.match(/pub enum ApiProvider \{([\s\S]*?)\}/);
  if (!enumBlock) return [];
  return [...enumBlock[1].matchAll(/^\s*(\w+)\s*,\s*$/gm)].map((m) => m[1]);
}

/**
 * ApiProvider variants that are neither mapped to a website label nor
 * intentionally excluded. Exposed so the CI gate (`check-facts.mjs`) can
 * hard-fail on provider-inventory drift (#3772); the generator stays lenient
 * and merely warns so local `prebuild` is not blocked mid-development.
 */
export function unmappedProviderVariants() {
  return providerEnumVariants().filter(
    (v) => !EXCLUDED_PROVIDERS.has(v) && !PROVIDER_LABEL_MAP[v],
  );
}

export function deriveProviders() {
  const variants = providerEnumVariants();

  const unmapped = unmappedProviderVariants();
  if (unmapped.length > 0) {
    console.error(
      `[facts-lib] ApiProvider variants missing from PROVIDER_LABEL_MAP: ${unmapped.join(", ")}. ` +
        "Add them to PROVIDER_LABEL_MAP here AND in web/lib/facts-drift.ts (or to EXCLUDED_PROVIDERS if intentionally hidden).",
    );
    // The generator stays lenient and returns what it can map; the hard gate
    // lives in check-facts.mjs via unmappedProviderVariants() (#3772).
  }
  return variants
    .filter((v) => !EXCLUDED_PROVIDERS.has(v))
    .map((v) => PROVIDER_LABEL_MAP[v])
    .filter(Boolean);
}

export function deriveDefaultModel() {
  // DEFAULT_TEXT_MODEL's definition moved to config/models.rs in the #3311 split;
  // read both and match the const *definition* specifically (`= "..."`) so we
  // don't mis-bind to a later string at a mere use site.
  const cfg =
    (read("crates/tui/src/config/models.rs") ?? "") +
    "\n" +
    (read("crates/tui/src/config.rs") ?? "");
  if (!cfg.trim()) return null;
  const m = cfg.match(/DEFAULT_TEXT_MODEL\s*(?::\s*&str\s*)?=\s*"([^"]+)"/);
  return m ? m[1] : null;
}

export function deriveNodeEngines() {
  const pkg = read("npm/codewhale/package.json");
  if (!pkg) return null;
  try {
    return JSON.parse(pkg).engines?.node ?? null;
  } catch {
    return null;
  }
}

// --- Models ---------------------------------------------------------------
//
// The public "which models" list. Two source files are read, never scraped
// from live provider APIs:
//
//   crates/tui/src/model_registry.rs          → SEED_MODEL_IDS tuples carry
//     the canonical model ids Codewhale makes first-class promises about,
//     each with its coarse ModelProvider grouping.
//   crates/models/assets/model_catalog.bundled.json → the bundled metadata
//     snapshot (context window, max output, reasoning flag) that the
//     runtime's offline catalog layer ships.
//
// Provider-prefixed spellings of the same model (`deepseek/`,
// `deepseek-ai/`, `z-ai/`, `moonshotai/`, `minimax/`, `qwen/`, `arcee-ai/`,
// `opencode-go/`, `nvidia/`) collapse into one canonical row — they are wire
// aliases for one model, not separate models.
//
// `addedAt` is the honest recency the repo itself can prove: the commit date
// on which the model id first appeared in the model *declaration* paths
// below — where a model becomes selectable, not where it is mentioned in
// tests or docs. It means "first supported in Codewhale source", which is
// the claim the page makes; it is NOT the provider's own release date.

const MODEL_REGISTRY_PATH = "crates/tui/src/model_registry.rs";
const MODEL_CATALOG_PATH = "crates/models/assets/model_catalog.bundled.json";
const MODEL_DECLARATION_PATHS = [
  "crates/config",
  "crates/models",
  "crates/tui/src/model_registry.rs",
  "crates/tui/src/config",
  "crates/tui/src/model_routing.rs",
];

const MODEL_PROVIDER_LABELS = {
  DeepSeek: "DeepSeek",
  Anthropic: "Anthropic",
  OpenAi: "OpenAI",
  OpenAiCodex: "OpenAI Codex",
  Moonshot: "Moonshot/Kimi",
  Zai: "Z.ai",
  Minimax: "MiniMax",
  Qwen: "Qwen",
  Arcee: "Arcee",
  Together: "Together",
  XiaomiMimo: "Xiaomi MiMo",
  Meta: "Meta",
  Xai: "xAI",
  Mistral: "Mistral",
  Google: "Google",
  Other: null,
};

// Longest prefixes first so `deepseek-ai/` wins over `deepseek/`.
const MODEL_ID_PREFIXES = [
  "deepseek-ai", "moonshotai", "opencode-go", "arcee-ai", "minimax",
  "deepseek", "huggingface", "together", "nvidia", "qwen", "z-ai",
  "openai", "google", "xai", "mistral", "stepfun", "meta",
];

const MODEL_PREFIX_PROVIDERS = {
  "deepseek-ai": "DeepSeek", deepseek: "DeepSeek", "z-ai": "Zai",
  moonshotai: "Moonshot", minimax: "Minimax", qwen: "Qwen",
  "arcee-ai": "Arcee", together: "Together", nvidia: "Other",
  "opencode-go": "Moonshot", openai: "OpenAi", google: "Google",
  xai: "Xai", mistral: "Mistral", stepfun: "Other", meta: "Meta",
  huggingface: "Other",
};

function canonicalModelId(id) {
  const slash = id.indexOf("/");
  if (slash < 0) return { canonical: id, prefix: null };
  const prefix = id.slice(0, slash);
  return MODEL_ID_PREFIXES.includes(prefix)
    ? { canonical: id.slice(slash + 1), prefix }
    : { canonical: id, prefix: null };
}

// Catalog-only ids carry no provider prefix; the model family is still
// obvious from the name for these. Everything else stays "Other" and renders
// as "—" rather than a guessed attribution.
const MODEL_NAME_FAMILIES = [
  [/^deepseek/i, "DeepSeek"],
  [/^claude/i, "Anthropic"],
  [/^(gpt|o[0-9]|codex|chatgpt)/i, "OpenAi"],
  [/^(kimi|moonshot)/i, "Moonshot"],
  [/^(glm-|zai)/i, "Zai"],
  [/^(minimax|abab)/i, "Minimax"],
  [/^(qwen|qwq)/i, "Qwen"],
  [/^(arcee|trinity|afm|virtuoso|maestro|spotlight|blitz)/i, "Arcee"],
  [/^(mimo|xiaomi)/i, "XiaomiMimo"],
  [/^(llama|meta-llama)/i, "Meta"],
  [/^grok/i, "Xai"],
  [/^(mistral|codestral|devstral|magistral|mixtral|pixtral|ministral|voxtral)/i, "Mistral"],
  [/^(gemini|gemma|learnlm)/i, "Google"],
];

function inferModelFamily(canonical) {
  for (const [re, provider] of MODEL_NAME_FAMILIES) {
    if (re.test(canonical)) return provider;
  }
  return "Other";
}

function seedModelRows() {
  const src = read(MODEL_REGISTRY_PATH);
  if (!src) return [];
  return [...src.matchAll(/\("([^"]+)",\s*ModelProvider::(\w+)\)/g)]
    .map((m) => ({ id: m[1], provider: m[2] }));
}

function bundledCatalogEntries() {
  const raw = read(MODEL_CATALOG_PATH);
  if (!raw) return new Map();
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" && parsed.entries
      ? new Map(Object.entries(parsed.entries))
      : new Map();
  } catch {
    return new Map();
  }
}

/**
 * First-appearance dates for every model alias in ONE history pass. The
 * `-G` alternation restricts emitted diffs to commits that touched a model
 * id line, keeping the output small; walking it oldest-first and matching
 * `"id"` on added lines gives each alias its introduction commit. A commit
 * that only removed an id can never be its earliest hit.
 */
function modelFirstSeen(aliases) {
  const needles = [...new Set(aliases)].map((id) =>
    `"${id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"`,
  );
  if (needles.length === 0) return new Map();
  const res = spawnSync(
    "git",
    [
      "log", "--reverse", "--format=%x00%cI", "--no-renames",
      "-G", needles.join("|"),
      "-p", "--", ...MODEL_DECLARATION_PATHS,
    ],
    { cwd: REPO_ROOT, encoding: "utf8", maxBuffer: 256 * 1024 * 1024 },
  );
  const firstSeen = new Map();
  if (res.status !== 0 || typeof res.stdout !== "string") return firstSeen;
  for (const chunk of res.stdout.split("\0").slice(1)) {
    const nl = chunk.indexOf("\n");
    const when = chunk.slice(0, nl).trim().slice(0, 10);
    const added = chunk
      .split("\n")
      .filter((line) => line.startsWith("+") && !line.startsWith("+++"))
      .join("\n");
    for (const id of aliases) {
      if (!firstSeen.has(id) && added.includes(`"${id}"`)) {
        firstSeen.set(id, when);
      }
    }
  }
  return firstSeen;
}

export function deriveModels() {
  const catalog = bundledCatalogEntries();

  // Canonical id → merged row.
  const rows = new Map();
  const note = (rawId, providerKey) => {
    const { canonical, prefix } = canonicalModelId(rawId);
    const existing = rows.get(canonical);
    const provider =
      providerKey ??
      MODEL_PREFIX_PROVIDERS[prefix] ??
      inferModelFamily(canonical);
    if (existing) {
      if (existing.provider === "Other" && provider !== "Other") {
        existing.provider = provider;
      }
      existing.catalogIds.push(rawId);
      return existing;
    }
    const row = { id: canonical, provider, catalogIds: [rawId] };
    rows.set(canonical, row);
    return row;
  };

  for (const seed of seedModelRows()) {
    note(seed.id, seed.provider);
  }
  for (const [id, entry] of catalog) {
    const row = note(id, null);
    row.entry = row.entry ?? entry;
  }

  // A model's date is the earliest first-appearance across all of its wire
  // spellings — an alias arriving later must not move the model's date.
  const allAliases = [...rows.values()].flatMap((row) => row.catalogIds);
  const firstSeen = modelFirstSeen(allAliases);
  const models = [...rows.values()].map((row) => {
    const seen = row.catalogIds
      .map((cid) => firstSeen.get(cid))
      .filter(Boolean)
      .sort();
    return {
      id: row.id,
      provider: MODEL_PROVIDER_LABELS[row.provider] ?? null,
      contextWindow: row.entry?.context_window ?? null,
      maxOutput: row.entry?.max_output ?? null,
      reasoning: row.entry?.supports_reasoning === true,
      addedAt: seen[0] ?? null,
    };
  });

  // Newest first; undated rows sink to the end alphabetically.
  models.sort((a, b) => {
    if (a.addedAt && b.addedAt && a.addedAt !== b.addedAt) {
      return b.addedAt.localeCompare(a.addedAt);
    }
    if (a.addedAt !== b.addedAt) return a.addedAt ? -1 : 1;
    return a.id.localeCompare(b.id);
  });
  return models;
}

export function deriveToolCount() {
  const dir = join(REPO_ROOT, "crates/tui/src/tools");
  if (!existsSync(dir)) return null;
  let count = 0;
  for (const f of readdirSync(dir)) {
    if (!f.endsWith(".rs")) continue;
    const body = readFileSync(join(dir, f), "utf-8");
    count += (body.match(/^impl ToolSpec for /gm) ?? []).length;
  }
  return count > 0 ? count : null;
}

export function deriveLicense() {
  const lic = read("LICENSE");
  if (!lic) return null;
  const first = lic.split(/\r?\n/).find((l) => l.trim().length > 0);
  if (!first) return null;
  if (/^MIT License/i.test(first)) return "MIT";
  if (/Apache.*2\.0/i.test(first)) return "Apache-2.0";
  return first.trim();
}

export function deriveLatestPublishedRelease() {
  const raw = read("web/data/latest-published-release.json");
  if (!raw) return null;
  try {
    const release = JSON.parse(raw);
    if (
      typeof release.tag !== "string" ||
      typeof release.version !== "string" ||
      release.tag !== `v${release.version}` ||
      typeof release.publishedAt !== "string" ||
      !Number.isFinite(Date.parse(release.publishedAt)) ||
      typeof release.url !== "string" ||
      release.url !== `https://github.com/Hmbown/CodeWhale/releases/tag/${release.tag}`
    ) {
      return null;
    }
    return release;
  } catch {
    return null;
  }
}

/**
 * Re-derive all mechanical facts from the current workspace. The returned
 * object is the same shape as web/lib/facts.generated.ts → RepoFacts.
 */
export function buildFacts() {
  const providers = deriveProviders();
  // In check mode, missing provider mappings are a warning, not a crash.
  // But if we truly have zero mapped providers, that signals something
  // went wrong (e.g. config.rs renamed) — still return an empty array
  // rather than null so the checker can report it.

  const facts = {
    generatedAt: new Date().toISOString(),
    // next.config.ts injects these from the exact checkout into the built
    // artifact. They stay null in the tracked snapshot to avoid a
    // self-referential generated-file diff after every commit.
    sourceRevision: null,
    sourceCommittedAt: null,
    version: deriveVersion(),
    crates: deriveCrates(),
    sandboxBackends: deriveSandboxBackends(),
    providers,
    models: deriveModels(),
    defaultModel: deriveDefaultModel(),
    nodeEngines: deriveNodeEngines(),
    toolCount: deriveToolCount(),
    license: deriveLicense(),
    latestPublishedRelease: deriveLatestPublishedRelease(),
  };

  return facts;
}
