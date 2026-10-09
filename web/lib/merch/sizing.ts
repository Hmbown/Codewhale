export type MerchSizingProductCode = "DJCTX" | "3MTXVYM32" | "3MOTS" | "3MCSPL56" | "3PH" | "3TPH" | "3MHZDM16";
export type MerchSizingUnit = "cm" | "in";
export type MerchSizingWarning = "poloShoulderNote" | "hoodieSleeveNote" | "cottonHoodieMaterialNote";

export interface MerchSizeRow {
  size: string;
  lengthCm: number;
  shoulderCm: number;
  /** Supplier's Bust column; its measurement convention has not been confirmed. */
  bustCm: number;
  sleeveCm: number | null;
}

export interface MerchSizeChart {
  code: MerchSizingProductCode;
  sourceUrl: string;
  toleranceCm: number;
  rows: readonly MerchSizeRow[];
  confirmationNeededSizes: readonly string[];
  warning?: MerchSizingWarning;
}

/** Public Yoycol charts captured 2026-10-04. Garment guidance, not body sizing or sample proof. */
export const MERCH_SIZE_CHARTS: readonly MerchSizeChart[] = [
  {
    code: "DJCTX",
    sourceUrl: "https://www.yoycol.com/product_detail/DJCTX/Unisex-O-neck-Short-Sleeve-T-shirt-180GSM-Cotton-DTF",
    toleranceCm: 3,
    confirmationNeededSizes: [],
    rows: [
      { size: "S", lengthCm: 67, shoulderCm: 42, bustCm: 94, sleeveCm: null },
      { size: "M", lengthCm: 70, shoulderCm: 44, bustCm: 100, sleeveCm: null },
      { size: "L", lengthCm: 73, shoulderCm: 46, bustCm: 106, sleeveCm: null },
      { size: "XL", lengthCm: 75, shoulderCm: 50, bustCm: 112, sleeveCm: null },
      { size: "2XL", lengthCm: 77, shoulderCm: 54, bustCm: 118, sleeveCm: null },
      { size: "3XL", lengthCm: 80, shoulderCm: 60, bustCm: 126, sleeveCm: null },
    ],
  },
  {
    code: "3MTXVYM32",
    sourceUrl: "https://www.yoycol.com/product_detail/3MTXVYM32",
    toleranceCm: 5,
    confirmationNeededSizes: [],
    rows: [
      { size: "XS", lengthCm: 68, shoulderCm: 37, bustCm: 82, sleeveCm: null },
      { size: "S", lengthCm: 71, shoulderCm: 40, bustCm: 92, sleeveCm: null },
      { size: "M", lengthCm: 74, shoulderCm: 43, bustCm: 102, sleeveCm: null },
      { size: "L", lengthCm: 76, shoulderCm: 46, bustCm: 112, sleeveCm: null },
      { size: "XL", lengthCm: 79, shoulderCm: 49, bustCm: 122, sleeveCm: null },
      { size: "2XL", lengthCm: 81, shoulderCm: 52, bustCm: 132, sleeveCm: null },
      { size: "3XL", lengthCm: 84, shoulderCm: 55, bustCm: 142, sleeveCm: null },
      { size: "4XL", lengthCm: 86, shoulderCm: 58, bustCm: 152, sleeveCm: null },
      { size: "5XL", lengthCm: 88, shoulderCm: 61, bustCm: 162, sleeveCm: null },
    ],
  },
  {
    code: "3MOTS",
    sourceUrl: "https://www.yoycol.com/product_detail/3MOTS",
    toleranceCm: 3,
    confirmationNeededSizes: [],
    rows: [
      { size: "XS", lengthCm: 68, shoulderCm: 37, bustCm: 82, sleeveCm: null },
      { size: "S", lengthCm: 71, shoulderCm: 40, bustCm: 92, sleeveCm: null },
      { size: "M", lengthCm: 74, shoulderCm: 43, bustCm: 102, sleeveCm: null },
      { size: "L", lengthCm: 76, shoulderCm: 46, bustCm: 112, sleeveCm: null },
      { size: "XL", lengthCm: 79, shoulderCm: 49, bustCm: 122, sleeveCm: null },
      { size: "2XL", lengthCm: 81, shoulderCm: 52, bustCm: 132, sleeveCm: null },
      { size: "3XL", lengthCm: 84, shoulderCm: 55, bustCm: 142, sleeveCm: null },
      { size: "4XL", lengthCm: 86, shoulderCm: 58, bustCm: 152, sleeveCm: null },
      { size: "5XL", lengthCm: 88, shoulderCm: 61, bustCm: 162, sleeveCm: null },
      { size: "6XL", lengthCm: 90, shoulderCm: 64, bustCm: 172, sleeveCm: null },
    ],
  },
  {
    code: "3MCSPL56",
    sourceUrl: "https://www.yoycol.com/product_detail/3MCSPL56",
    toleranceCm: 3,
    confirmationNeededSizes: ["2XL", "3XL"],
    warning: "poloShoulderNote",
    rows: [
      { size: "S", lengthCm: 71, shoulderCm: 40, bustCm: 92, sleeveCm: 23 },
      { size: "M", lengthCm: 74, shoulderCm: 43, bustCm: 102, sleeveCm: 23.5 },
      { size: "L", lengthCm: 76, shoulderCm: 46, bustCm: 112, sleeveCm: 24 },
      { size: "XL", lengthCm: 79, shoulderCm: 49, bustCm: 122, sleeveCm: 24.5 },
      { size: "2XL", lengthCm: 81, shoulderCm: 55, bustCm: 132, sleeveCm: 25 },
      { size: "3XL", lengthCm: 84, shoulderCm: 55, bustCm: 142, sleeveCm: 25.5 },
      { size: "4XL", lengthCm: 86, shoulderCm: 58, bustCm: 152, sleeveCm: 26 },
      { size: "5XL", lengthCm: 88, shoulderCm: 61, bustCm: 162, sleeveCm: 26.5 },
    ],
  },
  {
    code: "3PH",
    sourceUrl: "https://www.yoycol.com/product_detail/3PH",
    toleranceCm: 3,
    confirmationNeededSizes: ["7XL"],
    warning: "hoodieSleeveNote",
    rows: [
      { size: "XS", lengthCm: 65, shoulderCm: 39, bustCm: 92, sleeveCm: 66 },
      { size: "S", lengthCm: 67, shoulderCm: 42, bustCm: 102, sleeveCm: 67 },
      { size: "M", lengthCm: 70, shoulderCm: 45, bustCm: 112, sleeveCm: 68 },
      { size: "L", lengthCm: 72, shoulderCm: 48, bustCm: 122, sleeveCm: 69 },
      { size: "XL", lengthCm: 75, shoulderCm: 51, bustCm: 132, sleeveCm: 70.5 },
      { size: "2XL", lengthCm: 76.5, shoulderCm: 54, bustCm: 142, sleeveCm: 72 },
      { size: "3XL", lengthCm: 79.5, shoulderCm: 57, bustCm: 152, sleeveCm: 73 },
      { size: "4XL", lengthCm: 82, shoulderCm: 60, bustCm: 162, sleeveCm: 74 },
      { size: "5XL", lengthCm: 85, shoulderCm: 63, bustCm: 172, sleeveCm: 75 },
      { size: "6XL", lengthCm: 88, shoulderCm: 66, bustCm: 182, sleeveCm: 76 },
      { size: "7XL", lengthCm: 91, shoulderCm: 69, bustCm: 192, sleeveCm: 75 },
    ],
  },
  {
    code: "3TPH",
    sourceUrl: "https://www.yoycol.com/product_detail/3TPH",
    toleranceCm: 3,
    confirmationNeededSizes: [],
    rows: [
      { size: "XS", lengthCm: 65, shoulderCm: 39, bustCm: 92, sleeveCm: 66 },
      { size: "S", lengthCm: 67, shoulderCm: 42, bustCm: 102, sleeveCm: 67 },
      { size: "M", lengthCm: 70, shoulderCm: 45, bustCm: 112, sleeveCm: 68 },
      { size: "L", lengthCm: 72, shoulderCm: 48, bustCm: 122, sleeveCm: 69 },
      { size: "XL", lengthCm: 75, shoulderCm: 51, bustCm: 132, sleeveCm: 70 },
      { size: "2XL", lengthCm: 76, shoulderCm: 54, bustCm: 142, sleeveCm: 71 },
      { size: "3XL", lengthCm: 79, shoulderCm: 57, bustCm: 152, sleeveCm: 71 },
      { size: "4XL", lengthCm: 82, shoulderCm: 60, bustCm: 162, sleeveCm: 72 },
      { size: "5XL", lengthCm: 85, shoulderCm: 63, bustCm: 172, sleeveCm: 73 },
      { size: "6XL", lengthCm: 88, shoulderCm: 66, bustCm: 182, sleeveCm: 73.5 },
    ],
  },
  {
    code: "3MHZDM16",
    sourceUrl: "https://www.yoycol.com/product_detail/3MHZDM16",
    toleranceCm: 5,
    confirmationNeededSizes: [],
    warning: "cottonHoodieMaterialNote",
    rows: [
      { size: "XS", lengthCm: 66, shoulderCm: 41, bustCm: 96, sleeveCm: 67 },
      { size: "S", lengthCm: 69, shoulderCm: 43, bustCm: 104, sleeveCm: 68 },
      { size: "M", lengthCm: 71, shoulderCm: 45, bustCm: 112, sleeveCm: 69 },
      { size: "L", lengthCm: 74, shoulderCm: 47, bustCm: 120, sleeveCm: 70 },
      { size: "XL", lengthCm: 76, shoulderCm: 49.5, bustCm: 130, sleeveCm: 71 },
      { size: "2XL", lengthCm: 79, shoulderCm: 52, bustCm: 140, sleeveCm: 72 },
      { size: "3XL", lengthCm: 81, shoulderCm: 54.5, bustCm: 150, sleeveCm: 73 },
      { size: "4XL", lengthCm: 84, shoulderCm: 57, bustCm: 160, sleeveCm: 74 },
      { size: "5XL", lengthCm: 86, shoulderCm: 59.5, bustCm: 170, sleeveCm: 75 },
      { size: "6XL", lengthCm: 89, shoulderCm: 62, bustCm: 180, sleeveCm: 76 },
    ],
  },
];

/** Unit conversion only; never converts supplier Bust or regional letter sizes. */
export function merchMeasurement(cm: number, unit: MerchSizingUnit): number {
  return unit === "in" ? cm / 2.54 : cm;
}
