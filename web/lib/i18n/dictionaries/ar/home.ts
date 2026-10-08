import type { HomeDict } from "../types";

/**
 * Arabic home dictionary — native copy for the whale-road landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale: وكيل برمجة مفتوح المصدر لأي نموذج",
  metaDescription:
    "Codewhale وكيل برمجة مفتوح المصدر لطرفيتك. يقرأ مشروعك ويعدّل الملفات ويشغّل اختباراتك باستخدام النموذج المستضاف أو المحلي الذي تختاره.",
  heroTitle: "وكيل برمجة مفتوح المصدر لأي نموذج",
  heroIntro:
    "يقرأ {brand} مشروعك ويعدّل الملفات ويشغّل اختباراتك من طرفيتك. اربط نموذجًا مستضافًا أو محليًا، واختر الإجراءات التي تحتاج إلى موافقتك.",
  getCodewhale: "ثبّت Codewhale",
  heroInstallAria: "أمر التثبيت",
  exploreProduct: "شاهد كيف يعمل",
  shotPreview: "معاينة الطرفية",
  shotBuild: "إصدار تطوير v{version}",
  screenshotAlt:
    "إصدار تطوير Codewhale v{version}: علامة الحوت، جلسة جديدة، حقل الرسالة، أذونات Ask، وضع Work وحالة النموذج. عرض للمخرجات الفعلية من طرفية معزولة.",
  latestRelease: "أحدث إصدار {tag}",
  releaseUnavailable: "حالة الإصدار غير متاحة",
  currentSource: "المصدر",
  sourceCandidate: "غير منشور",
  publishedRelease: "منشور",
  figcaptionSourceCandidate: "غير منشور",
  chapterTerminal: "طرفيتك",
  chapterTerminalTitle: "تابع كل تعديل وكل أمر أثناء تنفيذه",
  gainHeading:
    "فوّض المهمة واحتفظ بالتحكم",
  gainLede:
    "اطلب نتيجة: إصلاح خطأ، أو شرح وحدة برمجية، أو أتمتة مهمة تكررها. ابدأ بوكيل واحد، وأضف المزيد من الوكلاء عندما يكبر العمل.",
  gain: [
    [
      "غيّر الشيفرة وتحقق منها",
      "يفحص الوكيل مشروعك ويعدّل الملفات ويشغّل اختباراتك. تابع كل تعديل ونتيجة كل أمر أثناء عمله."
    ],
    [
      "أتمت العمل المتكرر",
      "شغّل codewhale exec من السكربتات وCI. استخدم Fleet لتقسيم عمل أكبر بين عدة وكلاء."
    ],
    [
      "ابقَ متحكمًا",
      "اضبط الأذونات قبل بدء العمل، وأجب عن طلبات الموافقة، وأوقف أي مهمة في أي وقت. شغّل /receipts لعرض قائمة بكل ملف وأمر وموافقة في الجلسة."
    ]
  ],
  chapterModels: "نماذجك",
  modelsHeading: "اختر نموذجًا لكل مهمة",
  modelsBody:
    "اختر لكل جلسة مزوّدًا مدمجًا، أو أي نقطة نهاية متوافقة مع OpenAI، أو نموذجًا محليًا. يبقى اتصالك بالنموذج منفصلًا عن أي حساب Codewhale.",
  modelsFacts: [
    ["مستضاف", "مفتاح API الخاص بك، محفوظ عبر codewhale auth set --provider <id>"],
    ["بوابة", "نقطة نهاية واحدة لنماذج كثيرة؛ وما زلت أنت من يختار المزوّد"],
    ["محلي", "vLLM أو SGLang أو Ollama على localhost، وغالبًا دون مفتاح"],
  ],
  modelsLink: "تصفّح النماذج والمزوّدين",
  startHeading: "ثبّت، واربط نموذجًا، وشغّل مهمة",
  startLede:
    "شغّل مهمتك الأولى في ثلاث خطوات من مجلد مشروعك. أضف Fleet لاحقًا إذا احتاج العمل إلى عدة وكلاء.",
  startGuideLink: "اتبع دليل البداية ←",
  startVocabularyLink: "اطّلع على مفردات المنتج ←",
  chapterAvailability: "أين يعمل",
  availabilityHeading: "استخدم Codewhale في طرفيتك اليوم",
  availabilityLede:
    "يمكنك الآن استخدام الطرفية أو عميل المتصفح المحلي أو واجهة CodeWhale GUI التي يصونها المجتمع. تطبيق سطح المكتب وتطبيق الويب المستضاف الذي يُعاد بناؤه قيد التطوير، ويشتركان في نموذج الجلسات نفسه.",
  availability: [
    [
      "الطرفية والمتصفح المحلي",
      "تم الإصدار",
      "ثبّته على Linux أو macOS أو Windows، ثم شغّل codewhale، أو codewhale web لعميل المتصفح المحلي. يعمل npm وCargo أيضًا؛ ودعم Android عبر Termux في مرحلة المعاينة."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "متاح",
      "مشروع مستقل يصونه المجتمع: محادثة وخيوط وتغييرات الملفات في شريط جانبي داخل VS Code متصلة بـ Codewhale Runtime نفسه. ثبّت الواجهة من VS Code Marketplace.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "تطبيق الويب",
      "معاينة قيد التطوير",
      "يُعاد بناؤه ليطابق تطبيق سطح المكتب. يمكنك اليوم تسجيل الدخول، ثم كتابة /rc في جلسة طرفية قيد التشغيل لمتابعتها على الويب؛ وما زال تنفيذ المهام المستضاف قيد التأهيل."
    ],
    [
      "سطح المكتب",
      "نسخة قيد التطوير",
      "التطبيق الأصلي الذي يتحول إلى العميل الرئيسي لـ Codewhale: المجلدات والمحادثات واتصالات النماذج في نافذة واحدة. لا يتوفر تنزيل عام بعد."
    ],
    [
      "أجهزة الكمبيوتر السحابية",
      "قيد التطوير",
      "أجهزة كمبيوتر مستضافة تشغّل مهامك."
    ]
  ],
  availabilityNote:
    "لا تحتاج الطرفية والمتصفح المحلي وواجهة GUI إلى حساب Codewhale. يستخدم الويب المستضاف وسطح المكتب حسابًا لا يحل محل اتصالك بالنموذج؛ ويفوترك مزوّدك على الاستخدام بمفتاحك الخاص.",
  accountLink: "أنشئ حسابًا",
  surfacesHeading: "وسّع ما يمكن للوكيل الوصول إليه",
  surfaces: [
    ["الملفات والأوامر", "اقرأ المشروع، وعدّل الملفات، وشغّل الاختبارات، وافحص المخرجات ضمن الأذونات التي تحددها."],
    ["الإضافات وMCP", "اربط المزيد من الأدوات والخدمات. تبقى كل إضافة معطّلة حتى تراجعها وتفعّلها."],
    ["Computer Use · معاينة", "إضافة تتيح للوكيل رؤية التطبيقات الأخرى وتشغيلها. أنت من يفعّل الإضافة ويمنحها أذونات النظام التي تطلبها."],
    ["الجلسات المحفوظة", "احتفظ بالمحادثة ونتائج الأدوات معًا، واستأنف العمل بدلًا من البدء من جديد. يفتح المتصفح المحلي الجلسة نفسها على جهازك."],
    ["Fleet", "وزّع أجزاء المهمة على وكلاء بنماذج وأدوار مختلفة، ثم تابع تقدمهم."],
  ],
  runtimeLink: "اطّلع على كل التكاملات",
  installBandHeading: "التثبيت على macOS أو Linux",
  copy: "انسخ",
  copied: "نُسخ ✓",
  binaries: "الملفات الثنائية",
  chinaMirrors: "مرايا في الصين",
  installGuideLink: "اقرأ دليل التثبيت ←",
  communityHeading: "ابنِ Codewhale معنا",
  communityBody:
    "أبلغ عن خطأ، أو اقترح ميزة، أو أرسل أول طلب سحب لك على GitHub. نرحّب بالإصلاحات الصغيرة المختبَرة.",
  communityLinksAria: "روابط المجتمع",
  contribute: "إرسال طلب سحب",
};
