import type { HomeDict } from "../types";

/** Turkish home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: kendi modellerin ve araçlarınla geliştir",
  metaDescription:
    "Codewhale ile uygulamalar geliştir, iş akışlarını otomatikleştir ve bağlı araçlarla çalış. Açık kaynak; kendi model API’lerin, yerel veya kendi barındırdığın çıkarımla.",
  heroTitle: "Bilgisayarın için açık kaynaklı bir ajan.",
  heroIntro:
    "Uygulamalar geliştir, iş akışlarını otomatikleştir ve Slack, Gmail ve diğer bağlı araçlarla çalış. {brand}, mevcut model API’lerini veya yerel ve kendi barındırdığın çıkarım altyapısını kullanır.",
  getCodewhale: "Codewhale'i kur",
  heroInstallAria: "Kurulum komutu",
  exploreProduct: "Codewhale’i keşfet",
  shotPreview: "Terminal önizlemesi",
  screenshotAlt:
    "Codewhale v{version}: sohbeti, mesaj alanını ve oturum kontrollerini gösteren terminal kaydı.",
  latestRelease: "En yeni sürüm {tag}",
  releaseUnavailable: "Sürüm durumu kullanılamıyor",
  currentSource: "Kaynak",
  sourceCandidate: "Yayımlanmadı",
  publishedRelease: "yayımlandı",
  gainHeading:
    "Neler yapabilirsin",
  gainLede:
    "İstediğin sonuçla başla. Codewhale dosyalar, komutlar ve bağlı araçlarla çalışır; erişimi ve onayları sen belirlersin.",
  gain: [
    [
      "Uygulamalar ve araçlar geliştir",
      "Bir fikri çalışan uygulamaya, faydalı bir betiğe veya mevcut projede yeni bir özelliğe dönüştür. Ajan seninle birlikte yazsın, çalıştırsın ve test etsin."
    ],
    [
      "Tekrarlanan işleri otomatikleştir",
      "Tekrarlanan bir görevi terminalden, betiklerden veya CI’dan çalıştırılan iş akışına dönüştür. İş paralel yürüyebiliyorsa Fleet ile ajan ekibi kullan."
    ],
    [
      "Kullandığın araçları bağla",
      "Gmail ve Slack gibi araçları eklentiler, MCP sunucuları veya API’lerle bağla. Bu hizmetlerle dosya ve komutlarını aynı görevde kullan."
    ]
  ],
  chapterModels: "Senin modellerin",
  modelsHeading: "Kendi modellerini kullanmaya devam et.",
  modelsBody:
    "Zaten ödeme yaptığın model API’lerini bağla, uyumlu bir ağ geçidi kullan veya kendi donanımında çıkarım çalıştır. Her oturum ve Fleet içindeki her ajan için model seç.",
  modelsFacts: [
    [
      "API hesapların",
      "OpenAI, Anthropic, Google veya DeepSeek’i kendi anahtarlarınla bağla."
    ],
    [
      "Ağ geçidin",
      "OpenAI uyumlu bir uç nokta kullan ve sunduğu modelleri seç."
    ],
    [
      "Kendi çıkarımın",
      "Ollama, vLLM veya SGLang ile yerel ya da kendi barındırdığın modelleri çalıştır."
    ]
  ],
  modelsLink: "Modellere ve sağlayıcılara göz at",
  startHeading: "Bir görev getir. Başla.",
  startLede:
    "Codewhale’i kur, bir model bağla ve ona yapmaya değer bir iş ver. Bir ajanla başla; gerektiğinde araçlar veya bir ekip ekle.",
  startGuideLink: "Başlangıç kılavuzunu takip et",
  startVocabularyLink: "Ürün sözlüğünü gör",
  chapterAvailability: "Nerede çalışır",
  availabilityHeading: "Terminalden başla.",
  availabilityLede:
    "Terminal ve yerel tarayıcı istemcisi hazır. Yerel masaüstü uygulaması ve yeniden kurulan barındırılmış web uygulaması geliştiriliyor.",
  availability: [
    [
      "Terminal ve yerel tarayıcı",
      "Yayınlandı",
      "Linux, macOS veya Windows üzerine kur, ardından codewhale komutunu ya da yerel tarayıcı istemcisi için codewhale web komutunu çalıştır. npm ve Cargo da çalışır; Termux üzerinde Android önizleme aşamasında."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "Kullanılabilir",
      "Topluluk tarafından sürdürülen ayrı bir proje: aynı Codewhale Runtime üzerinde VS Code kenar çubuğunda sohbet, konular ve dosya değişiklikleri. VS Code Marketplace'ten kur.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "Web uygulaması",
      "Geliştirme önizlemesi",
      "Masaüstü uygulamasına uyacak şekilde yeniden yapılıyor. Bugün oturum açabilir, ardından çalışan bir terminal oturumuna /rc yazarak o oturumu web üzerinde sürdürebilirsin; barındırılan görev yürütme hâlâ doğrulanıyor."
    ],
    [
      "Masaüstü",
      "Geliştirme sürümü",
      "Codewhale'in ana istemcisi haline gelen yerel uygulama: klasörler, sohbetler ve model bağlantıları tek bir pencerede. Henüz herkese açık indirme yok."
    ],
    [
      "Bulut bilgisayarları",
      "Geliştirme aşamasında",
      "Görevlerini çalıştıran barındırılan bilgisayarlar."
    ]
  ],
  availabilityNote:
    "Terminal, yerel tarayıcı ve GUI için Codewhale hesabı gerekmez. Barındırılan web ve masaüstü uygulaması bir hesap kullanır. Kendi sağlayıcı anahtarını kullanırsan, bu kullanımı sağlayıcın faturalandırır.",
  accountLink: "Hesap oluştur",
  surfacesHeading: "Tek görev. Dosyaların, uygulamaların ve ajanların.",
  surfaces: [
    [
      "Dosyalar ve terminal",
      "Dosya oluştur, komut çalıştır, verileri incele ve geliştirdiklerini test et. Çalışma klasörünü ve izinleri sen belirlersin."
    ],
    [
      "Eklentiler ve bağlı uygulamalar",
      "Eklentiler ve MCP ile beceriler ve araçlar ekle. Ajanın kullanmasını istediğin bağlantıları incele ve etkinleştir."
    ],
    [
      "Tarayıcı ve bilgisayar · önizleme",
      "Verdiğin erişim kapsamında uygulama ve sitelerde tarayıcı araçları ve Computer Use eklentisiyle çalış."
    ],
    [
      "Devam edebileceğin oturumlar",
      "Sohbeti, araç sonuçlarını ve çalışma geçmişini bir arada tut. Görevi terminalde veya yerel tarayıcı istemcisinde sürdür."
    ],
    [
      "Ajan ekipleri",
      "Büyük bir işi Fleet ile farklı rol ve modellerdeki ajanlara böl, ilerlemelerini tek yerde izle."
    ]
  ],
  runtimeLink: "Tüm entegrasyonları gör",
  installBandHeading: "macOS veya Linux üzerine kur",
  copy: "Kopyala",
  copied: "Kopyalandı ✓",
  binaries: "İkililer",
  chinaMirrors: "Çin yansıları",
  installGuideLink: "Kurulum kılavuzunu oku",
  communityHeading: "Codewhale’i kendine göre şekillendir.",
  communityBody:
    "Codewhale açık kaynak. Kodu oku, bir eklenti geliştir, iş akışı paylaş veya sonraki sürümü iyileştirmeye yardım et.",
  communityLinksAria: "Topluluk bağlantıları",
  contribute: "GitHub’da katkıda bulun",
};
