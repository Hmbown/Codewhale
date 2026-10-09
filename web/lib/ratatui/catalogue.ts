import catalogue from "@/public/ratatui/catalogue.json";
import { discoveryText, displayApi, normalizeSearch } from "./learning";

export interface CatalogueEntry {
  name: string;
  title: string;
  family: string;
  width: number;
  height: number;
  description: string;
  api: string[];
  source: { file: string; function: string; line: number; url: string };
  previewPath: string;
}

export interface Catalogue {
  schemaVersion: number;
  source: { revision: string; digest: string; repository: string; nativeRevision: string };
  profiles: { id: string; label: string }[];
  sizes: { id: string; label: string }[];
  families: { id: string; title: string; description: string; count: number }[];
  entries: CatalogueEntry[];
}

export interface EntryPreview {
  name: string;
  width: number;
  height: number;
  previews: Record<string, Record<string, string>>;
  fixture: { code: string; imports: string; helpers?: { name: string; code: string; line: number }[]; source: unknown };
}

/** Bundled at build time: no filesystem or remote repository dependency in the Worker. */
export function readCatalogue(): Catalogue {
  return catalogue;
}

export function searchEntries(entries: readonly CatalogueEntry[], query: string, family = "all") {
  const terms = normalizeSearch(query).split(/\s+/).filter(Boolean);
  return entries.filter((entry) => {
    if (family !== "all" && entry.family !== family) return false;
    const text = normalizeSearch([entry.name, entry.title, ...displayApi(entry), discoveryText(entry)].join(" "));
    return terms.every((term) => text.includes(term));
  });
}

export function previewAssetPath(entry: CatalogueEntry) {
  if (!/^entries\/[a-z0-9_-]+\.json$/.test(entry.previewPath)) throw new Error("Invalid catalogue path");
  return `/ratatui/${entry.previewPath}`;
}

export function motionId(entry: CatalogueEntry, profile: string) {
  if (entry.name === "showcase-life" || ["whales", "whale-actions"].includes(entry.family)) return "whale";
  if (entry.family === "habitat") return "habitat";
  if (entry.family === "motion") return profile === "light-truecolor" ? "motion-light" : "motion";
  if (entry.family === "studio") return profile === "light-truecolor" ? "studio-light" : "studio";
  return null;
}
