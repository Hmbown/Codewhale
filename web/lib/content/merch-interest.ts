import { pickText } from "@/lib/i18n/dictionaries";
import type { MerchInterestItemId } from "@/lib/merch/interest-options";

const text = {
  title: { en: "Help shape the first drop.", zh: "一起决定首批周边。" },
  body: { en: "Pick what you’d wear or keep on your desk. Join the waitlist and tell us what price would feel comfortable.", zh: "选出你想穿的、或想放在桌上的周边。加入意向名单，也告诉我们你能接受的价格。" },
  join: { en: "Join the merch waitlist", zh: "加入周边意向名单" },
  priceNote: { en: "Prices below are research choices for one item. Shipping and applicable tax would be extra. These are not confirmed sale prices or a preorder.", zh: "以下为单件商品的价格调研选项，运费及适用税费另计。这些不是已确定的售价，也不是预售。" },
  products: { en: "What are you interested in?", zh: "你对哪些周边感兴趣？" },
  pickNote: { en: "Choose one or more. Price feedback is optional.", zh: "可多选，价格反馈为选填。" },
  cotton: { en: "Cotton everyday tees", zh: "日常纯棉 T 恤" },
  cottonNote: { en: "Cotton for a familiar, everyday feel. Prints and fit still need samples.", zh: "纯棉，适合日常穿着。印花与版型仍需用实物样品确认。" },
  stretch: { en: "Stretch fabrics and layers", zh: "弹力面料与外搭" },
  stretchNote: { en: "Polyester blends for ombré and all-over prints; fleece for warmth, lighter scuba for the wave hoodie.", zh: "聚酯混纺用于渐变与全幅印花；抓绒款更保暖，海浪卫衣采用较轻的空气层面料。" },
  desk: { en: "For your desk", zh: "桌面周边" },
  plush: { en: "The mascot companion", zh: "吉祥物毛绒伙伴" },
  email: { en: "Email for merch updates", zh: "接收周边消息的邮箱" },
  country: { en: "Country or region", zh: "国家或地区" },
  countryHint: { en: "For planning delivery. No street address needed.", zh: "用于规划配送，无需填写街道地址。" },
  currency: { en: "Price currency", zh: "价格币种" },
  currencyNote: { en: "CNY and USD choices are separate research points, not exchange-rate equivalents.", zh: "人民币与美元选项分别用于调研，不代表汇率换算后的等值价格。" },
  price: { en: "Comfortable item price (optional)", zh: "可接受的单件价格（选填）" },
  unsure: { en: "Not sure yet", zh: "还不确定" },
  none: { en: "None of these prices", zh: "这些价格都不合适" },
  consent: { en: "Email me about Codewhale merch samples and launch news.", zh: "我愿意接收 Codewhale 周边样品与开售消息。" },
  privacy: { en: "Your contact and choices stay private and are kept for up to 180 days. No payment is taken. You can opt out of any future update.", zh: "联系方式与选择会保密，最多保存 180 天。不收取任何费用，之后的消息可随时退订。" },
  privacyLink: { en: "Privacy policy", zh: "隐私政策" },
  checking: { en: "Checking signup availability…", zh: "正在确认是否可以提交……" },
  unavailable: { en: "Signup isn’t open on this preview yet. You can explore the choices; please come back when submissions open.", zh: "此预览暂未开放提交。你可以先看看款式与价格选项，开放后再来登记。" },
  saving: { en: "Saving your interest…", zh: "正在保存你的意向……" },
  saved: { en: "You’re on the interest list. Thanks for helping shape the drop.", zh: "你的意向已保存。感谢你一起决定首批周边。" },
  savedNote: { en: "This is not an order or a reservation. We’ll share sample results and confirmed prices before sales open.", zh: "这不是订单，也不代表预留商品。开售前会公布样品反馈与确认后的价格。" },
  edit: { en: "Update my choices", zh: "修改我的选择" },
  needPick: { en: "Choose at least one item before joining.", zh: "请先选择至少一款周边。" },
  error: { en: "Your signup wasn’t saved. Check the form and try again.", zh: "意向未能保存，请检查填写内容后重试。" },
  retry: { en: "Please wait a minute, then try again. Your signup has not been saved.", zh: "请等一分钟后重试，你的意向尚未保存。" },
} as const;

const items: Record<MerchInterestItemId, { name: { en: string; zh: string }; detail: { en: string; zh: string } }> = {
  "signature-tee": { name: { en: "Signature Tee", zh: "日常款 T 恤" }, detail: { en: "100% cotton · small chest signature", zh: "纯棉 · 胸前小标志" } },
  "classic-tee": { name: { en: "Classic Tee", zh: "经典款 T 恤" }, detail: { en: "100% cotton · centered whale and name", zh: "纯棉 · 正中的小鲸与名字" } },
  "whale-bro-tee": { name: { en: "Whale Bro Tee", zh: "鲸鱼兄弟 T 恤" }, detail: { en: "100% cotton · 鲸鱼兄弟", zh: "纯棉 · 鲸鱼兄弟" } },
  "mascot-tee": { name: { en: "Mascot Tee", zh: "吉祥物 T 恤" }, detail: { en: "Cotton · saluting character badge", zh: "棉 T 恤 · 敬礼角色徽章" } },
  "tide-tee": { name: { en: "Tide Ombré Tee", zh: "海潮渐变 T 恤" }, detail: { en: "95% polyester / 5% spandex · light-to-dark color", zh: "95% 聚酯 / 5% 氨纶 · 浅至深渐变" } },
  "signature-polo": { name: { en: "Signature Polo", zh: "日常款 Polo 衫" }, detail: { en: "88% polyester / 12% spandex · small chest signature", zh: "88% 聚酯 / 12% 氨纶 · 胸前小标志" } },
  "everyday-hoodie": { name: { en: "Everyday Hoodie", zh: "日常抓绒卫衣" }, detail: { en: "95% polyester / 5% spandex · heavier fleece", zh: "95% 聚酯 / 5% 氨纶 · 较厚抓绒" } },
  "mascot-hoodie": { name: { en: "Ocean Wave Mascot Hoodie", zh: "海浪吉祥物卫衣" }, detail: { en: "95% polyester / 5% spandex · lightweight scuba", zh: "95% 聚酯 / 5% 氨纶 · 轻薄空气层" } },
  mousepad: { name: { en: "Cache Mousepad", zh: "Cache 鼠标垫" }, detail: { en: "18 × 22cm · rubber-backed cloth", zh: "18 × 22cm · 橡胶底布面" } },
  deskmat: { name: { en: "Developer Deskmat", zh: "开发者桌垫" }, detail: { en: "80 × 30cm candidate · detailed ocean art", zh: "80 × 30cm 候选 · 精细海洋图案" } },
  stickers: { name: { en: "Sticker Pack", zh: "贴纸包" }, detail: { en: "Developer jokes · pack contents still being planned", zh: "开发者梗 · 内容与数量筹备中" } },
  plush: { name: { en: "20cm Mascot Doll", zh: "20cm 吉祥物娃娃" }, detail: { en: "Seated blue-haired plush · prototype planned", zh: "蓝发坐姿毛绒娃娃 · 实物样品筹备中" } },
};

export function getInterestCopy(locale: string) {
  return Object.fromEntries(Object.entries(text).map(([key, value]) => [key, pickText(value, locale)])) as Record<keyof typeof text, string>;
}
export function getInterestItemCopy(id: MerchInterestItemId, locale: string) {
  return { name: pickText(items[id].name, locale), detail: pickText(items[id].detail, locale) };
}
