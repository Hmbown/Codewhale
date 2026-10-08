import type { HomeDict } from "../types";

/**
 * Turkish home dictionary — native copy for the whale-road landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale: her model için açık kaynaklı kodlama ajanı",
  metaDescription:
    "Codewhale, terminalin için açık kaynaklı bir kodlama ajanıdır. Projeni okur, dosyaları düzenler ve testlerini seçtiğin barındırılan veya yerel modelle çalıştırır.",
  heroTitle: "Her model için açık kaynaklı kodlama ajanı",
  heroIntro:
    "{brand}, terminalinde projeni okur, dosyaları düzenler ve testlerini çalıştırır. Barındırılan veya yerel bir model bağla ve hangi eylemlerin senin onayını gerektireceğini seç.",
  getCodewhale: "Codewhale'i kur",
  heroInstallAria: "Kurulum komutu",
  exploreProduct: "Nasıl çalıştığını gör",
  shotPreview: "Terminal önizlemesi",
  shotBuild: "v{version} geliştirme derlemesi",
  screenshotAlt:
    "Codewhale v{version} geliştirme derlemesi: balina, yeni oturum, mesaj alanı, Ask izinleri, Work modu ve model durumu. Yalıtılmış bir terminalin gerçek çıktısından oluşturulmuştur.",
  latestRelease: "En yeni sürüm {tag}",
  releaseUnavailable: "Sürüm durumu kullanılamıyor",
  currentSource: "Kaynak",
  sourceCandidate: "Yayımlanmadı",
  publishedRelease: "yayımlandı",
  figcaptionSourceCandidate: "yayımlanmadı",
  chapterTerminal: "Senin terminalin",
  chapterTerminalTitle: "Her düzenlemeyi ve komutu çalışırken takip et",
  gainHeading:
    "Görevi devret ve kontrolü elinde tut",
  gainLede:
    "Bir sonuç iste: bir hatanın düzeltilmesi, bir modülün açıklanması veya tekrarladığın bir görevin otomatikleştirilmesi. Tek bir ajanla başla ve iş büyüdükçe daha fazla ajan ekle.",
  gain: [
    [
      "Kodu değiştir ve kontrol et",
      "Ajan projeni inceler, dosyaları düzenler ve testlerini çalıştırır. Ajan çalışırken her düzenlemeyi ve komut sonucunu takip et."
    ],
    [
      "Tekrarlanan işleri otomatikleştir",
      "Betiklerden ve CI'dan codewhale exec komutunu çalıştır. Daha büyük bir işi birkaç ajan arasında bölmek için bir Fleet kullan."
    ],
    [
      "Kontrolü elinde tut",
      "Çalışma başlamadan izinleri ayarla, onay isteklerini yanıtla ve bir görevi istediğin anda durdur. Bir oturumdaki her dosyayı, komutu ve onayı listelemek için /receipts komutunu çalıştır."
    ]
  ],
  chapterModels: "Senin modellerin",
  modelsHeading: "Her görev için bir model seç",
  modelsBody:
    "Her oturum için yerleşik bir sağlayıcı, OpenAI uyumlu herhangi bir uç nokta veya yerel bir model seç. Model bağlantın, herhangi bir Codewhale hesabından ayrı kalır.",
  modelsFacts: [
    ["Barındırılan", "codewhale auth set --provider <id> ile kaydedilen kendi API anahtarın"],
    ["Gateway", "Birçok model için tek uç nokta; sağlayıcıyı yine sen seçersin"],
    ["Yerel", "localhost üzerinde vLLM, SGLang veya Ollama, genellikle anahtarsız"],
  ],
  modelsLink: "Modellere ve sağlayıcılara göz at",
  startHeading: "Kur, bir model bağla, bir görev çalıştır",
  startLede:
    "Proje klasöründen üç adımda ilk görevini çalıştır. İş birkaç ajan gerektirirse daha sonra bir Fleet ekle.",
  startGuideLink: "Başlangıç kılavuzunu takip et",
  startVocabularyLink: "Ürün sözlüğünü gör",
  chapterAvailability: "Nerede çalışır",
  availabilityHeading: "Codewhale'i bugün terminalinde kullan",
  availabilityLede:
    "Terminali, yerel tarayıcı istemcisini veya topluluk tarafından sürdürülen CodeWhale GUI'yi şimdi kullanabilirsin. Masaüstü uygulaması ve yeniden yapılan barındırılan web uygulaması geliştirme aşamasında ve aynı oturum modelini paylaşıyor.",
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
    "Terminal, yerel tarayıcı ve GUI için Codewhale hesabı gerekmez. Barındırılan web ve masaüstü bir hesap kullanır ve bu hesap model bağlantının yerini almaz; kendi anahtarınla yapılan kullanımı sağlayıcın faturalandırır.",
  accountLink: "Hesap oluştur",
  surfacesHeading: "Ajanın erişebildiği alanı genişlet",
  surfaces: [
    ["Dosyalar ve komutlar", "Belirlediğin izinler dahilinde projeyi oku, dosyaları düzenle, testleri çalıştır ve çıktıyı incele."],
    ["Eklentiler ve MCP", "Daha fazla araç ve hizmet bağla. Her eklenti, sen inceleyip etkinleştirene kadar kapalı kalır."],
    ["Computer Use · önizleme", "Ajanın diğer uygulamaları görmesini ve kullanmasını sağlayan bir eklenti. Eklentiyi sen etkinleştirir ve istediği sistem izinlerini verirsin."],
    ["Kayıtlı oturumlar", "Sohbeti ve araç sonuçlarını bir arada tut ve baştan başlamak yerine kaldığın yerden devam et. Yerel tarayıcı, bilgisayarındaki aynı oturumu açar."],
    ["Fleet", "Bir görevin parçalarını farklı modellere ve rollere sahip ajanlara ata, ardından ilerlemelerini takip et."],
  ],
  runtimeLink: "Tüm entegrasyonları gör",
  installBandHeading: "macOS veya Linux üzerine kur",
  copy: "Kopyala",
  copied: "Kopyalandı ✓",
  binaries: "İkililer",
  chinaMirrors: "Çin yansıları",
  installGuideLink: "Kurulum kılavuzunu oku",
  communityHeading: "Codewhale'i bizimle birlikte geliştir",
  communityBody:
    "GitHub'da bir hata bildir, bir özellik öner veya ilk pull request'ini gönder. Küçük ve test edilmiş düzeltmeler memnuniyetle karşılanır.",
  communityLinksAria: "Topluluk bağlantıları",
  contribute: "Pull request gönder",
};
