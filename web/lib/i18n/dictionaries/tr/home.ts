import type { HomeDict } from "../types";

/**
 * Turkish home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — Seçtiğin modellerle geliştir ve işleri otomatikleştir",
  metaDescription:
    "Açık kaynaklı ajanlar ve seçtiğin barındırılan veya yerel AI modelleriyle yazılım geliştir, dosyaların üzerinde çalış ve günlük görevleri otomatikleştir.",
  heroTitle: "Seçtiğin modellerle geliştir ve işleri otomatikleştir",
  heroIntro:
    "{brand}, yazılım geliştirebilen, dosyaların üzerinde çalışabilen ve tekrarlanan görevleri yeniden kullanılabilir iş akışlarına dönüştürebilen ajanlar sunar. Onlara ne yapmak istediğini anlat ve işe uygun barındırılan veya yerel modelleri seç; çalışırken sağlayıcı değiştirmekte özgürsün.",
  getCodewhale: "Codewhale'i edin",
  exploreProduct: "Ürünü keşfet",
  shotPreview: "Terminal önizlemesi",
  shotBuild: "v{version} geliştirme derlemesi",
  screenshotAlt:
    "Codewhale v{version} geliştirme derlemesi: balina, yeni oturum, mesaj alanı, Ask izinleri, Work modu ve model durumu. Yalıtılmış bir terminalin gerçek çıktısından oluşturulmuştur.",
  latestRelease: "En yeni sürüm {tag}",
  releaseUnavailable: "Sürüm durumu kullanılamıyor",
  currentSource: "Kaynak",
  sourceCandidate: "Yayımlanmadı",
  providerRoutes: "{count} sağlayıcı",
  publishedRelease: "yayımlandı",
  figcaptionSourceCandidate: "yayımlanmadı",
  chapterTerminal: "Senin terminalin",
  chapterTerminalTitle: "Yapmak istediğin bir şeyle başla",
  gainHeading:
    "Codewhale ile neler yapabilirsin",
  gainLede:
    "Bir projeyle, bir soruyla veya otomatikleştirmek istediğin bir görevle başla; ardından tek bir ajanla çalış ya da daha büyük bir işin parçalarını birkaç ajana ver.",
  gain: [
    [
      "Bir şey geliştir",
      "Yapmak istediğini anlat ve kodunu okuyabilen, dosyaları düzenleyebilen, komutları çalıştırabilen ve sonucu kontrol edebilen ajanlarla çalış."
    ],
    [
      "Günlük işleri otomatikleştir",
      "Tekrarladığın görevler için betikler ve iş akışları oluştur; böylece ihtiyaç duyduğunda bunları terminalden yeniden çalıştırabilirsin."
    ],
    [
      "Farklı modellerle çalış",
      "Ajanların için barındırılan veya yerel modeller kullan; farklı modeller ve roller, işin kendilerine uygun kısımlarını üstlensin."
    ]
  ],
  chapterModels: "Senin modellerin",
  modelsHeading: "Her görev için model seçenekleri",
  modelsBody:
    "Doğrudan model barındıran bir sağlayıcıya bağlan, birden fazla sağlayıcıya erişmek için bir ağ geçidi kullan veya bir modeli yerel olarak çalıştır; ardından çalışırken her oturumun hangi modeli kullanacağını seç.",
  modelsFacts: [
    ["Barındırılan", "codewhale auth set --provider <id> ile kaydedilen kendi API anahtarın"],
    ["Gateway", "Birçok model için tek uç nokta, sağlayıcıyı yine sen seçersin"],
    ["Yerel", "localhost üzerinde vLLM, SGLang, Ollama — genellikle anahtarsız"],
  ],
  modelsLink: "Modelleri ve sağlayıcıları keşfet",
  startHeading: "Codewhale ile işe başla",
  startLede:
    "Codewhale'i kurup bir model bağladıktan sonra ilk görevini terminalde anlatabilir, birkaç ajanın işi paylaşmasını istediğinde bir Fleet ekleyebilirsin.",
  startGuideLink: "Başlangıç kılavuzunu oku",
  startVocabularyLink: "Ürün sözlüğünü gör",
  chapterAccount: "Codewhale'i edin",
  availabilityHeading: "Codewhale'i nerelerde kullanabilirsin",
  availabilityLede:
    "Biz web uygulamasını, masaüstü uygulamasını ve bulut bilgisayarlarını geliştirirken sen Codewhale'i bugün terminalinde kullanabilirsin.",
  availability: [
    [
      "Terminal",
      "Yayınlandı",
      "Linux, macOS ve Windows için GitHub sürüm ikili dosyaları; npm ve Cargo alternatiflerdir. Termux üzerinde Android desteği önizleme aşamasında."
    ],
    [
      "Web uygulaması",
      "Geliştirme önizlemesi",
      "Geliştirme önizlemesinde hesap erişimi ve tarayıcı eşleştirme."
    ],
    [
      "Masaüstü",
      "Geliştirme sürümü",
      "macOS uygulaması geliştirme aşamasında; herkese açık indirme daha sonra sunulacak."
    ],
    [
      "Bulut bilgisayarları",
      "Geliştirme aşamasında",
      "Görevlerini çalıştırmak için barındırılan bilgisayarlar."
    ]
  ],
  availabilityNote:
    "Terminali Codewhale hesabı olmadan kullanabilirsin; barındırılan model kullanımını ise sağlayıcın faturalandırır.",
  accountLink: "Hesap oluştur",
  surfacesHeading: "Codewhale ile çalışma yolları",
  surfaces: [
    ["TUI", "Terminalde etkileşimli iş"],
    ["codewhale exec", "Betikler ve CI"],
    ["Yerel web istemcisi","localhost arayüzü; barındırılan tarayıcı çalışma alanı geliştirme aşamasında"],
    ["Runtime API + MCP", "Yerel entegrasyonlar"],
    ["Fleet","Tek bir işte birden çok ajan"],
  ],
  runtimeLink: "Entegrasyonları keşfet",
  installBandHeading: "Codewhale'i macOS veya Linux üzerine kur",
  copy: "Kopyala",
  copied: "Kopyalandı ✓",
  binaries: "İkililer",
  chinaMirrors: "Çin yansıları",
  installGuideLink: "Kurulum kılavuzunu oku",
  communityHeading: "Codewhale'i daha iyi hâle getirmeye yardımcı ol",
  communityBody:
    "Bir hata bulduysan, bir özellik fikrin varsa ya da ilk pull request'ini göndermek istiyorsan seni dinlemek ve sonraki adımlar üzerinde birlikte çalışmak isteriz.",
  communityLinksAria: "Topluluk bağlantıları",
  contribute: "Pull request gönder",
};
