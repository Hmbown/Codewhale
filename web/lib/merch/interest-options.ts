/** Willingness-to-pay research, in major currency units. Never checkout prices or FX conversions. */
export const MERCH_INTEREST_ITEMS = [
  { id: "signature-tee", group: "cotton", prices: { cny: [59, 69, 89], usd: [12, 15, 19] } },
  { id: "classic-tee", group: "cotton", prices: { cny: [59, 69, 89], usd: [12, 15, 19] } },
  { id: "whale-bro-tee", group: "cotton", prices: { cny: [59, 69, 89], usd: [12, 15, 19] } },
  { id: "mascot-tee", group: "cotton", prices: { cny: [59, 69, 89], usd: [12, 15, 19] } },
  { id: "tide-tee", group: "stretch", prices: { cny: [69, 89, 109], usd: [15, 19, 23] } },
  { id: "signature-polo", group: "stretch", prices: { cny: [99, 129, 159], usd: [20, 25, 30] } },
  { id: "everyday-hoodie", group: "stretch", prices: { cny: [129, 159, 199], usd: [25, 30, 36] } },
  { id: "mascot-hoodie", group: "stretch", prices: { cny: [99, 129, 159], usd: [22, 28, 34] } },
  { id: "mousepad", group: "desk", prices: { cny: [29, 39, 49], usd: [6, 8, 10] } },
  { id: "deskmat", group: "desk", prices: { cny: [49, 69, 89], usd: [12, 15, 19] } },
  { id: "stickers", group: "desk", prices: { cny: [9, 19, 29], usd: [3, 5, 8] } },
  { id: "plush", group: "plush", prices: { cny: [99, 129, 149], usd: [20, 25, 30] } },
] as const;

export type MerchInterestItemId = typeof MERCH_INTEREST_ITEMS[number]["id"];
export type MerchInterestCurrency = "cny" | "usd";
export type MerchInterestPriceChoice = "low" | "mid" | "high" | "none" | "unsure";
export const MERCH_INTEREST_PRICE_CHOICES = ["low", "mid", "high", "none", "unsure"] as const;
