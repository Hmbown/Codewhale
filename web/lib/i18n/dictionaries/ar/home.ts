import type { HomeDict } from "../types";

/**
 * Arabic home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — ابنِ وأتمت باستخدام النماذج التي تختارها",
  metaDescription:
    "ابنِ البرمجيات، واعمل على ملفاتك، وأتمت مهامك اليومية باستخدام وكلاء مفتوحي المصدر وما تختاره من نماذج الذكاء الاصطناعي المستضافة أو المحلية.",
  heroTitle: "ابنِ وأتمت باستخدام النماذج التي تختارها",
  heroIntro:
    "يمنحك {brand} وكلاء يمكنهم بناء البرمجيات والعمل على ملفاتك وتحويل المهام المتكررة إلى مسارات عمل قابلة لإعادة الاستخدام. أخبرهم بما تريد إنجازه واختر النماذج المستضافة أو المحلية المناسبة للمهمة، مع حرية تبديل المزوّدين أثناء العمل.",
  getCodewhale: "احصل على Codewhale",
  exploreProduct: "استكشف المنتج",
  shotPreview: "معاينة الطرفية",
  shotBuild: "إصدار تطوير v{version}",
  screenshotAlt:
    "إصدار تطوير Codewhale v{version}: علامة الحوت، جلسة جديدة، حقل الرسالة، أذونات Ask، وضع Work وحالة النموذج. عرض للمخرجات الفعلية من طرفية معزولة.",
  latestRelease: "أحدث إصدار {tag}",
  releaseUnavailable: "حالة الإصدار غير متاحة",
  currentSource: "المصدر",
  sourceCandidate: "غير منشور",
  providerRoutes: "{count} مزوّد",
  publishedRelease: "منشور",
  figcaptionSourceCandidate: "غير منشور",
  chapterTerminal: "طرفيتك",
  chapterTerminalTitle: "ابدأ بشيء تريد صنعه",
  gainHeading:
    "ما يمكنك فعله باستخدام Codewhale",
  gainLede:
    "ابدأ بمشروع أو سؤال أو مهمة تريد أتمتتها، ثم اعمل مع وكيل واحد أو وزّع أجزاء العمل الأكبر على عدة وكلاء.",
  gain: [
    [
      "ابنِ شيئًا",
      "صف ما تريد صنعه واعمل مع وكلاء يمكنهم قراءة شيفرتك وتعديل الملفات وتشغيل الأوامر والتحقق من النتيجة."
    ],
    [
      "أتمت العمل اليومي",
      "أنشئ سكربتات ومسارات عمل للمهام التي تكررها، لتتمكن من تشغيلها مجددًا من الطرفية كلما احتجت إليها."
    ],
    [
      "اعمل مع نماذج مختلفة",
      "استخدم نماذج مستضافة أو محلية لوكلائك، بحيث تتولى النماذج والأدوار المختلفة أجزاء العمل المناسبة لها."
    ]
  ],
  chapterModels: "نماذجك",
  modelsHeading: "خيارات من النماذج لكل مهمة",
  modelsBody:
    "اتصل مباشرة بمزوّد نماذج مستضافة، أو استخدم بوابة للوصول إلى عدة مزوّدين، أو شغّل نموذجًا محليًا، ثم اختر النموذج الذي تستخدمه كل جلسة أثناء عملك.",
  modelsFacts: [
    ["مستضاف", "مفتاح API الخاص بك، محفوظ عبر codewhale auth set --provider <id>"],
    ["بوابة", "نقطة نهاية واحدة لنماذج كثيرة، والمزوّد ما زال من اختيارك"],
    ["محلي", "vLLM وSGLang وOllama على localhost — غالبًا بلا مفتاح"],
  ],
  modelsLink: "استكشف النماذج والمزوّدين",
  startHeading: "ابدأ باستخدام Codewhale",
  startLede:
    "بعد تثبيت Codewhale وربط نموذج، يمكنك وصف مهمتك الأولى في الطرفية وإضافة Fleet عندما تريد أن يتشارك عدة وكلاء العمل.",
  startGuideLink: "اقرأ دليل البداية ←",
  startVocabularyLink: "اطّلع على مفردات المنتج ←",
  chapterAccount: "احصل على Codewhale",
  availabilityHeading: "أين يمكنك استخدام Codewhale",
  availabilityLede:
    "يمكنك استخدام Codewhale في طرفيتك اليوم، بينما نعمل على تطوير تطبيق الويب وتطبيق سطح المكتب وأجهزة الكمبيوتر السحابية.",
  availability: [
    [
      "الطرفية",
      "تم الإصدار",
      "ملفات إصدار GitHub الثنائية لأنظمة Linux وmacOS وWindows؛ ويمكن استخدام npm وCargo كبديلين. دعم Android عبر Termux ما زال في مرحلة المعاينة."
    ],
    [
      "تطبيق الويب",
      "معاينة قيد التطوير",
      "الوصول إلى الحساب وإقران المتصفح ضمن المعاينة قيد التطوير."
    ],
    [
      "سطح المكتب",
      "نسخة قيد التطوير",
      "تطبيق macOS قيد التطوير؛ وسيتاح تنزيله للجميع لاحقًا."
    ],
    [
      "أجهزة الكمبيوتر السحابية",
      "قيد التطوير",
      "أجهزة كمبيوتر مستضافة لتشغيل مهامك."
    ]
  ],
  availabilityNote:
    "يمكنك استخدام الطرفية دون حساب Codewhale، ويتولى مزوّدك فوترة أي استخدام للنماذج المستضافة.",
  accountLink: "أنشئ حسابًا",
  surfacesHeading: "طرق العمل باستخدام Codewhale",
  surfaces: [
    ["TUI", "عمل تفاعلي في الطرفية"],
    ["codewhale exec", "سكربتات وCI"],
    ["عميل الويب المحلي","واجهة على localhost؛ مساحة العمل المستضافة في المتصفح قيد التطوير"],
    ["Runtime API + MCP", "تكاملات محلية"],
    ["Fleet","عدة وكلاء يعملون على مهمة واحدة"],
  ],
  runtimeLink: "استكشف التكاملات",
  installBandHeading: "ثبّت Codewhale على macOS أو Linux",
  copy: "انسخ",
  copied: "نُسخ ✓",
  binaries: "الملفات الثنائية",
  chinaMirrors: "مرايا في الصين",
  installGuideLink: "اقرأ دليل التثبيت ←",
  communityHeading: "ساهم في تحسين Codewhale",
  communityBody:
    "سواء اكتشفت خطأً، أو كانت لديك فكرة لميزة، أو أردت إرسال أول طلب سحب لك، نود أن نسمع منك ونعمل معًا على الخطوات القادمة.",
  communityLinksAria: "روابط المجتمع",
  contribute: "إرسال طلب سحب",
};
