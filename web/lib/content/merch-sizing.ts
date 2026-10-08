import { pickText } from "@/lib/i18n/dictionaries";
import type { MerchSizingProductCode } from "@/lib/merch/sizing";

/** Shared Chinese/English sizing copy; other locales use the site's English fallback. */
const text = {
  title: { en: "Compare garment measurements", zh: "对照成衣尺寸，选择尺码" },
  introduction: { en: "Start with a garment you like: lay it flat and measure its length, shoulders and sleeves. Compare only with the chart for the exact supplier garment; fit and letter sizes differ between styles.", zh: "找一件穿着合适的衣服，平铺后量衣长、肩宽和袖长，再对照对应供应商款式的尺寸表。不同款式的版型和字母尺码不通用。" },
  guidance: { en: "These are supplier garment measurements, not body measurements or a sample-verified fit guide. We do not guess sizes from height, weight or where you live.", zh: "这里列的是供应商提供的成衣尺寸，不是人体尺寸，也尚未用实物样品验证版型。不按身高、体重或所在地区猜测尺码。" },
  garment: { en: "Supplier garment", zh: "供应商款式" },
  units: { en: "Measurement units", zh: "尺寸单位" },
  cm: { en: "cm", zh: "厘米" },
  in: { en: "inches", zh: "英寸" },
  conversion: { en: "Inches = centimetres ÷ 2.54, rounded to one decimal place. Letter sizes are unchanged.", zh: "英寸 = 厘米 ÷ 2.54，显示到一位小数；字母尺码不作换算。" },
  caption: { en: "{garment}: supplier garment measurements ({unit})", zh: "{garment}：供应商成衣尺寸（{unit}）" },
  size: { en: "Size", zh: "尺码" },
  length: { en: "Length", zh: "衣长" },
  shoulder: { en: "Shoulder", zh: "肩宽" },
  bust: { en: "Bust (supplier label)", zh: "胸围（供应商 Bust）" },
  sleeve: { en: "Sleeve", zh: "袖长" },
  bustNote: { en: "The supplier's Bust measurement diagram has not been verified. Bust is preserved exactly as published; do not assume it is flat half-chest or convert it to a circumference. Confirm the measurement method before choosing a size.", zh: "供应商 Bust 的量法图尚未核实。此列保留原始数值，不将它当作平铺半胸宽，也不擅自换算成整圈胸围。选择尺码前需确认量法。" },
  tolerance: { en: "The supplier lists measurement variation up to {value} {unit}. No physical sample has been measured against this chart.", zh: "供应商注明尺寸误差最大为 {value} {unit}；尚未用实物样品核对这张表。" },
  missingSleeve: { en: "— means the supplier does not list that measurement; no sleeve length has been inferred.", zh: "— 表示供应商未列出该项尺寸；不推算缺失的袖长。" },
  confirmationRequired: { en: "Supplier confirmation needed", zh: "需供应商确认" },
  poloShoulderNote: { en: "* Published polo shoulders are 55cm for both 2XL and 3XL. These raw values are retained; confirm them before offering those variants.", zh: "* 供应商公布的 Polo 2XL、3XL 肩宽均为 55 厘米。原值保留，开放这两个尺码前需确认。" },
  hoodieSleeveNote: { en: "* Published hoodie sleeve length is 75cm for 7XL, shorter than 6XL's 76cm. The raw value is retained; confirm it before offering 7XL.", zh: "* 供应商公布的卫衣 7XL 袖长为 75 厘米，短于 6XL 的 76 厘米。原值保留，开放 7XL 前需确认。" },
  cottonHoodieMaterialNote: { en: "This hoodie is listed as cotton loopback, but the supplier also gives a polyester warning for the non-custom fabric option. Confirm the chosen fabric before offering it as cotton; this size chart does not establish a fabric quote.", zh: "此款标为棉质毛圈，但供应商对非定制面料选项另有聚酯纤维提示。作为棉质款售卖前需确认所选面料；尺码表不代表已确认的面料报价。" },
  source: { en: "Official supplier chart", zh: "供应商官方尺寸表" },
  dated: { en: "Public chart captured 4 October 2026. Listed sizes do not establish stock, checkout availability or delivery coverage.", zh: "公开尺寸表记录于 2026 年 10 月 4 日。列出的尺码不代表已有库存、可付款订购或已确认配送范围。" },
  tableRegion: { en: "Garment size table; scroll horizontally if needed", zh: "成衣尺码表，可横向滚动查看" },
} as const;

const garments: Record<MerchSizingProductCode, { en: string; zh: string }> = {
  "DJCTX": { en: "DJCTX · Cotton DTF tee", zh: "DJCTX · 纯棉 DTF T 恤" },
  "3MTXVYM32": { en: "3MTXVYM32 · Cotton tee", zh: "3MTXVYM32 · 纯棉 T 恤" },
  "3MOTS": { en: "3MOTS · Stretch jersey tee", zh: "3MOTS · 弹力针织 T 恤" },
  "3MCSPL56": { en: "3MCSPL56 · polo", zh: "3MCSPL56 · Polo 衫" },
  "3PH": { en: "3PH · Lightweight scuba hoodie", zh: "3PH · 轻薄 Scuba 面料卫衣" },
  "3TPH": { en: "3TPH · Polyester fleece hoodie", zh: "3TPH · 聚酯绒面卫衣" },
  "3MHZDM16": { en: "3MHZDM16 · Cotton loopback hoodie", zh: "3MHZDM16 · 棉质毛圈卫衣" },
};

export function getMerchSizingCopy(locale: string) {
  return Object.fromEntries(Object.entries(text).map(([key, value]) => [key, pickText(value, locale)])) as Record<keyof typeof text, string>;
}

export function merchSizingGarmentLabel(code: MerchSizingProductCode, locale: string): string {
  return pickText(garments[code], locale);
}
