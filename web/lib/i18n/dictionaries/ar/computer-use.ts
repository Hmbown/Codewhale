import type { ComputerUseDict } from "../types";

/**
 * Arabic (Modern Standard) dictionary for `app/[locale]/computer-use/page.tsx`
 * and the Computer Use section of the install page. Product names, the menu
 * items Pause, Stop and Check for updates, and the macOS setting names stay
 * as the app shows them; the macOS names use the Arabic system UI labels.
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Computer Use لنظام Mac · Codewhale",
  metaDescription: "نزّل Codewhale Computer Use لنظام Mac وجهّزه. تحكّم في التطبيقات في الخلفية، وإعداد الأذونات، وأزرار Pause وStop بيدك.",
  title: "Computer Use",
  lead: "دع Codewhale يعمل داخل تطبيقاتك بينما تواصل عملك. يجمع مساعد Mac في شريط القوائم الأذونات والتحكم في التطبيقات في الخلفية وإمكانية إيقاف الإدخال مؤقتًا أو إنهائه.",
  publisher: "من Codewhale",
  download: "نزّل لنظام Mac",
  downloadZip: "أرشيف ZIP (يستخدمه محدّث التطبيق الداخلي)",
  requirements: "macOS 13.5 أو أحدث · Apple silicon وIntel",
  included: "تنزيل تطبيق واحد فقط. لا حاجة إلى تثبيت Node أو مترجم بشكل منفصل.",
  pendingTitle: "تنزيل Mac قيد التحضير",
  pendingBody: "سيظهر المثبّت العام هنا بعد اكتمال توثيق Apple وفحوصات الإصدار.",
  unavailableTitle: "تعذّر التحقق من توفر التنزيل",
  unavailableBody: "حدّث هذه الصفحة للمحاولة مجددًا، أو راجع الإصدارات المنشورة أدناه.",
  releases: "الإصدارات المنشورة",
  receipt: "تفاصيل التحقق من التنزيل",
  setup: "جهّز جهاز Mac",
  steps: [
    { title: "ثبّت التطبيق", body: "افتح صورة القرص واسحب Codewhale Computer Use إلى مجلد «التطبيقات». افتحه من «التطبيقات»، ثم اختر Computer Use من أيقونة الحوت في شريط القوائم." },
    { title: "راجع الأذونات", body: "استخدم أزرار الإعداد لفتح «تسهيلات الاستخدام» و«تسجيل الشاشة» في إعدادات النظام. أنت من يقرر الأذونات التي تمنحها." },
    { title: "شغّل فحص الخلفية", body: "يفتح المساعد نافذة تدريب مؤقتة، ويدخل نصًا فيها، ثم يلتقط تلك النافذة. ويتحقق مما إذا كان المؤشر أو التطبيق النشط قد تغيّر أثناء التشغيل." },
    { title: "اربطه بـ Codewhale", body: "راجع Computer Use في متجر إضافات Codewhale ثم وثّقه وفعّله. استخدم الإضافة بإصدار 0.3.1 أو أحدث لتمر الإجراءات المحلية عبر زرّي Pause وStop في المساعد." },
  ],
  controlsTitle: "واصل عملك، وابقَ متحكمًا.",
  controlsBody: "تعمل الإجراءات المدعومة على التطبيق المحدد في الخلفية. أما التطبيقات والإيماءات التي تحتاج إلى التحكم في المقدمة فتتطلب إذنك. تعرض القائمة التطبيق المستهدف ووضع الإدخال؛ يعلّق Pause (إيقاف مؤقت) إدخال المساعد، وينهي Stop (إيقاف) جلساته الحالية.",
  updateTitle: "تحديثات في الوقت الذي تختاره",
  updateBody: "اختر Check for updates (التحقق من التحديثات) من التطبيق. قبل تثبيت أي تحديث، يتحقق من ملف التنزيل وتوقيع Codewhale وتوثيق Apple، ويحتفظ بالنسخة السابقة من التطبيق للاسترداد.",
  help: "الإعداد واستكشاف الأخطاء وإصلاحها",
  notes: "ملاحظات الإصدار",
  demo: "شاهد فحص الخلفية",
  source: "المصدر والمنصات الأخرى",
  platforms: "هذا التنزيل مخصص لنظام Mac. يعتمد Windows وLinux حاليًا على إضافة المصدر والإعداد من جهة المضيف.",
};
