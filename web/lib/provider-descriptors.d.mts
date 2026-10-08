import type { ProviderFact } from "./facts.generated";
export function parseProviderDescriptors(input: unknown): {
  labels: Readonly<Record<string, ProviderFact>>;
  providers: ProviderFact[];
  defaultModel: string;
} | null;
