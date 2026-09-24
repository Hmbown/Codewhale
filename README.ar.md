<!-- source: README.md sha256:bbb8bfb61e57 -->
# Codewhale

Codewhale وكيل مفتوح المصدر يقرأ مشروعك ويعدّل الملفات ويشغّل الأوامر ويتحقق من عمله باستخدام نموذج مستضاف أو محلي تختاره. ابدأ بمهمة واحدة في الطرفية. وللأعمال الأكبر، وزّع أجزاء العمل على وكلاء بنماذج وأدوار مختلفة.

![Codewhale يعمل في طرفية](web/public/codewhale-tui-d7a9a1c.png)

*معاينة للطرفية من بنية تطوير للإصدار v0.10.0.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## التثبيت

للتثبيت الجديد على macOS أو Linux، استخدم الإصدار الرسمي من GitHub:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

يختار المثبّت أحدث إصدار منشور. ويصف [سجل التغييرات](CHANGELOG.md) أيضًا النسخة المرشحة غير المنشورة للإصدار التالي؛ ولا تُضمّن هذه التغييرات في التنزيلات المنشورة حتى يصبح الإصدار متاحًا.

على Windows، نزّل المثبّت أو الأرشيف المناسب من [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest). لتحديث تثبيت مباشر موجود، شغّل `codewhale update`، أو `codewhale update --check` للفحص فقط. يعرض المحدّث مسار الملف التنفيذي ويحتفظ بالبنيات الأحدث. npm وCargo خياران ثانويان؛ راجع [دليل التثبيت](docs/INSTALL.md) للانتقال من تثبيت يديره مدير حزم وإعداد PATH.

يفتح التشغيل الأول مباشرةً على محرر الرسائل، ولا يرشدك خلال خطوات إعداد. تتطلب ردود النموذج الاتصال بنموذج مستضاف أو محلي: وإلى أن يتم ذلك، تعرض شاشة البدء "no model connected". شغّل `/provider` (أو اضغط F3) لإضافة مفتاح لخدمة مستضافة أو اختيار بيئة تشغيل محلية. وإذا كان Ollama يعمل بالفعل مع نموذج محادثة، ينتقل Codewhale إليه تلقائيًا. ويدعم Codewhale أيضًا npm وCargo كخياري تحزيم ثانويين، إلى جانب Docker وNix وScoop وAndroid/Termux ومرآة CNB اختيارية. تتوفر تعليمات انتقال للتثبيتات الحالية التي يديرها مدير حزم. راجع [المساعدة بشأن التثبيت وPATH](docs/INSTALL.md).

يمكن تفعيل الإكمال بمفتاح Tab بأمر واحد لكل واجهة أوامر — `codewhale completion bash|zsh|fish|powershell|elvish`. راجع [إكمال واجهة الأوامر](docs/INSTALL.md#8-shell-completions).

## الاستخدام

افتح طرفية في مجلد مشروعك وشغّل `codewhale`. اختر موفّرك باستخدام `/provider` ونموذجك باستخدام `/model`. ثم صِف مهمة محددة:

```text
Fix the failing tests and explain what changed.
```

أو شغّل مهمة من دون فتح واجهة TUI:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

يستطيع Codewhale قراءة مستودعك وتعديل الملفات وتشغيل الأوامر وفحص النتائج ومواصلة العمل نحو هدف. استخدم `/mode plan` للاستكشاف دون تغيير الملفات أو تنفيذ أوامر واجهة الأوامر، و`/mode work` عندما تريد إجراء تغييرات. اضغط `Shift+Tab` لاختيار Ask أو Auto-Review أو Full Access؛ يوضح [دليل الأوضاع والصلاحيات](docs/MODES.md) ما يسمح به كل خيار.

## الطرفية والتطبيقات وComputer Use

تتصل الطرفية والعملاء الرسوميون ببيئة Codewhale Runtime، التي تشغّل الوكيل وأدواته:

- **الطرفية:** يفتح `codewhale` الواجهة التفاعلية؛ ويشغّل `codewhale exec` مهمة من برنامج نصي أو مهمة CI.
- **المتصفح المحلي:** يفتح `codewhale web` [عميل الويب المحلي](docs/WEB.md) المرفق، والمتصل ببيئة التشغيل نفسها.
- **تطبيق Codewhale لسطح المكتب (GPUI):** تطبيق سطح مكتب أصلي، يُطوَّر في مستودع منفصل، هو اتجاه عميل المنتج للمستخدمين المسجّلين. سيُعاد بناء تطبيق الويب المستضاف على app.codewhale.net ليطابقه؛ ويبقى موقع التسويق وتسجيل الدخول والفوترة والصفحات القانونية وصفحات التنزيل على الويب. تُدرج معلومات توفره في [صفحة المنتج](https://codewhale.net/en/product).

**يضيف Computer Use أدوات لمراقبة التطبيقات الأخرى والتفاعل معها.** الإضافة مضمنة في الشيفرة المصدرية الحالية. راجع صلاحيات الوصول التي تطلبها وفعّلها قبل الاستخدام؛ وتظل أذونات نظام التشغيل ومتطلبات المنصة سارية. راجع [دليل Computer Use](crates/tui/plugins/computer-use/README.md) المرفق و[إعداد الإضافات](docs/PLUGINS.md).

يتصل امتداد CodeWhale لبرنامج VS Code، الذي يصونه المجتمع، ببيئة Runtime المحلية من الشريط الجانبي. ثبّته من [سوق VS Code](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode)؛ والشيفرة المصدرية على [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode).

## لماذا Codewhale

- **اختر نماذجك.** اتصل بموفّرين مستضافين أو بنماذج محلية عبر Ollama أو vLLM أو SGLang. استخدم `/provider` لتغيير الموفّر و`/model` لاختيار نموذج.
- **ابقَ مسيطرًا.** افحص الإجراءات المقترحة والتغييرات الناتجة في الملفات. تحدد إعدادات الموافقة متى تلزم المراجعة؛ ويظل Full Access ملتزمًا بالحدود الصارمة للسياسات. يساعدك `/undo` و`/restore` على استعادة مساحة العمل بعد التغييرات.
- **حافظ على تنظيم الأعمال الطويلة.** احفظ الجلسات، وحدد `/goal` دائمًا، وراجع مسارات العمل قبل تشغيلها، ونسّق بين الوكلاء من دون تحويل تعليماتهم الداخلية إلى جزء من محادثتك.
- **وسّع الوكيل الذي لديك بالفعل.** صِل خوادم MCP والمهارات، واضبط الخطافات، واحتفظ بأدوار الوكلاء كملفات مقروءة في مشروعك أو إعداداتك الشخصية.

شغّل `/help` في واجهة TUI لعرض الأوامر واختصارات لوحة المفاتيح.

## الأمان

يعمل Codewhale على جهازك بصلاحيات الوصول التي تمنحها له. تحد أوضاع الموافقة وقواعد المستودع مما يمكن للوكيل فعله؛ ويضيف عزل نظام التشغيل الاختياري حدًا أقوى للتنفيذ حيثما كان مدعومًا. تظل أسعار النماذج غير المعروفة مسجلة على أنها غير معروفة بدلًا من الإبلاغ عنها كمجانية.

اقرأ [ترتيب التفويض](docs/AUTHORIZATION_ORDER.md) لمعرفة التسلسل الدقيق للسياسات، و[الإعدادات](docs/CONFIGURATION.md) لمعرفة الضبط المحلي.

## الوثائق

- [الموفّرون والنماذج المحلية](docs/PROVIDERS.md)
- [فرق الوكلاء](docs/FLEET.md)
- [MCP](docs/MCP.md) و[الخطافات](docs/HOOKS.md) و[الإعدادات](docs/CONFIGURATION.md)
- [عميل الويب المحلي](docs/WEB.md)
- [جميع الوثائق](docs/README.md)
- [بنية المستودع ودليل المساهمة](CONTRIBUTING.md#project-structure)

## انضم إلى المجتمع

**نرحب بتقارير الأخطاء وأفكار الميزات وطلبات السحب**، سواء كنت تستخدم Codewhale منذ أشهر أو تجربه للمرة الأولى. إذا كان أحد الموفّرين غير متاح، أو كان مسار العمل غير مريح، أو كانت واجهة الطرفية تعيقك، [فافتح issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) أو [أرسل pull request](CONTRIBUTING.md) لنحسّنه معًا. نرحب بالمساهمات الأولى، ويظل كل مساهم منسوبًا إلى العمل الذي يُدمج في المشروع.

انضم إلى [Discord](https://discord.gg/37gfS3ksug)، أو أضف Hunter على WeChat (`hunterbown`) واطلب الانضمام إلى مجموعة Whale Brothers.

## تاريخ المشروع

بدأ Codewhale باسم `deepseek-tui`، ولا يزال يحافظ على التوافق مع إعداداته وجلساته. وهو الآن محايد تجاه الموفّرين ويُصان بصورة مستقلة ولا ينتمي إلى أي موفّر نماذج.

شكرًا لكل مساهم ولمجتمعات المصادر المفتوحة التي ساعدت المشروع على النمو. راجع [سجل المساهمين](docs/CONTRIBUTORS.md).

## الترخيص

[MIT](LICENSE). الأجزاء المقتبسة والمعدّلة من مشاريع أخرى مفتوحة المصدر مسجّلة في [إشعارات الجهات الخارجية](docs/THIRD_PARTY_NOTICES.md).
