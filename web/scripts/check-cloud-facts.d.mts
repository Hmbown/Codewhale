import type { TrustedKey } from "../lib/cloud-facts/keys";
export function parseRustKeys(text: string): TrustedKey[];
export function parseTsKeys(text: string): TrustedKey[];
export function checkCloudFacts(): { failures: string[]; factsVersion: number; activeKeys: number };
