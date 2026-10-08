/** Public launch selection. Prices are targets until an approved destination rate is quoted. */
export const MERCH_SIZES = ["S", "M", "L", "XL", "2XL", "3XL"] as const;
export const MERCH_PRODUCTS = [
  { id: "signature", title: "Codewhale Signature", caption: "Small left-chest signature", image: "/merch/signature-core.svg", kind: "regular", collection: "launch" },
  { id: "classic", title: "Codewhale Classic", caption: "The whale and the name", image: "/merch/classic-core.svg", kind: "regular", collection: "launch" },
  { id: "wordmark", title: "Codewhale Wordmark", caption: "Just the name", image: "/merch/wordmark-core.svg", kind: "regular", collection: "launch" },
  { id: "ocean-line", title: "Ocean line", caption: "A quiet horizon", image: "/merch/ocean-line-core.svg", kind: "regular", collection: "launch" },
  { id: "use-pocket", title: "use codewhale.", caption: "Small left-chest print", image: "/merch/use-pocket-core.svg", kind: "regular", collection: "launch" },
  { id: "field", title: "Codewhale Field", caption: "Front and back · separate quote", image: "/merch/field-front-core.svg", kind: "concept", collection: "later" },
  { id: "stage-dive", title: "Open source. Open mic.", caption: "Desk gear · stage dive / deep dive", image: "/merch/developer-desk-kit.png", kind: "concept", collection: "later" },
  { id: "use-codewhale", title: "use codewhale.", caption: "Small whale. Short line.", image: "/merch/use-codewhale-simple.svg", kind: "regular", collection: "launch" },
  { id: "compiling", title: "Compiling…", caption: "Just one more crate.", image: "/merch/compiling-simple.svg", kind: "regular", collection: "launch" },
  { id: "whale-bro", title: "鲸鱼兄弟", caption: "Whale Bro", image: "/merch/whale-bro-simple.svg", kind: "regular", collection: "launch" },
  { id: "contributor", title: "Built together.", caption: "Contributor edition · at cost", image: "/merch/open-ocean-patchwork.png", kind: "contributor", collection: "launch" },
  { id: "china-tour", title: "China Tour 2026", caption: "October 15–19 · Datawhale", image: "/merch/china-tour.png", kind: "regular", collection: "seasonal" },
  { id: "ocean-ombre", title: "Ocean ombré", caption: "Aqua to midnight", image: "/merch/ocean-ombre-reversed.png", kind: "concept", collection: "later" },
  { id: "hoodie", title: "Simple whale hoodie", caption: "Small left-chest mark · sample first", image: "/merch/hoodie-simple.svg", kind: "concept", collection: "later" },
  { id: "deskmat", title: "Open source deskmat", caption: "80 × 30 cm · cloth and rubber", image: "/merch/developer-desk-kit.png", kind: "concept", collection: "later" },
  { id: "mousepad", title: "Cache me if you can.", caption: "Mousepad · size to be selected", image: "/merch/developer-desk-kit.png", kind: "concept", collection: "later" },
  { id: "stickers", title: "Developer stickers", caption: "Compiling… · just one more crate", image: "/merch/developer-desk-kit.png", kind: "concept", collection: "later" },
  { id: "plush", title: "Codewhale plush doll", caption: "20 cm seated mascot · prototype planned", image: "/merch/seated-plush-concept.png", kind: "concept", collection: "prototype" },
  { id: "accessories", title: "Little companions", caption: "Tote · mug · stickers", image: "/merch/mascot-accessories.png", kind: "concept", collection: "later" },
] as const;
export type MerchProductId = typeof MERCH_PRODUCTS[number]["id"];
export const MERCH_COUNTRIES = [
  { code: "CN", name: "China" }, { code: "US", name: "United States" },
  { code: "CA", name: "Canada" }, { code: "GB", name: "United Kingdom" },
  { code: "DE", name: "Germany" }, { code: "FR", name: "France" },
  { code: "NL", name: "Netherlands" }, { code: "ES", name: "Spain" },
  { code: "IT", name: "Italy" }, { code: "SE", name: "Sweden" },
  { code: "IE", name: "Ireland" }, { code: "AU", name: "Australia" },
  { code: "NZ", name: "New Zealand" }, { code: "JP", name: "Japan" },
  { code: "KR", name: "South Korea" }, { code: "SG", name: "Singapore" },
  { code: "MY", name: "Malaysia" }, { code: "TH", name: "Thailand" },
  { code: "VN", name: "Vietnam" }, { code: "PH", name: "Philippines" },
  { code: "ID", name: "Indonesia" }, { code: "HK", name: "Hong Kong" },
  { code: "TW", name: "Taiwan" },
] as const;
// Countries are candidates for quoting, not a promise that a particular SKU/carrier serves every address.
export const MERCH_TARGET_PRICE = { cny: 6900, usd: 1500 } as const;
