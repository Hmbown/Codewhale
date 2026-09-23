import type { ComputerUseDict } from "../types";

/**
 * Turkish dictionary for `app/[locale]/computer-use/page.tsx` and the
 * Computer Use section of the install page. Product names, the menu items
 * Pause, Stop and Check for updates, and the macOS setting names stay as
 * the app shows them.
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Mac için Computer Use · Codewhale",
  metaDescription: "Mac için Codewhale Computer Use'u indir ve kur. Arka planda uygulama denetimi, izin kurulumu ve senin elindeki Pause ve Stop denetimleri.",
  title: "Computer Use",
  lead: "Sen çalışmaya devam ederken Codewhale uygulamalarında iş yapsın. Mac yardımcısı izinleri, arka planda uygulama denetimini ve girişi duraklatma ya da durdurma yolunu menü çubuğuna taşır.",
  publisher: "Codewhale tarafından",
  download: "Mac için indir",
  downloadZip: "ZIP arşivi (uygulama içi güncelleyici kullanır)",
  requirements: "macOS 13.5 veya üzeri · Apple silicon ve Intel",
  included: "Tek bir uygulama indirmesi. Ayrıca Node kurulumu veya derleyici gerekmez.",
  pendingTitle: "Mac indirmesi hazırlanıyor",
  pendingBody: "Apple noter onayı ve sürüm kontrolleri tamamlandığında herkese açık yükleyici burada görünecek.",
  unavailableTitle: "İndirme durumu kontrol edilemedi",
  unavailableBody: "Yeniden denemek için sayfayı yenile ya da aşağıdaki yayımlanmış sürümlere bak.",
  releases: "Yayımlanmış sürümler",
  receipt: "İndirme doğrulama bilgileri",
  setup: "Mac'ini kur",
  steps: [
    { title: "Uygulamayı kur", body: "Disk görüntüsünü aç ve Codewhale Computer Use'u Uygulamalar klasörüne sürükle. Uygulamalar'dan aç, ardından menü çubuğundaki balina simgesinden Computer Use'u seç." },
    { title: "İzinleri gözden geçir", body: "Kurulum düğmeleriyle Sistem Ayarları'nda Erişilebilirlik ve Ekran Kaydı bölümlerini aç. Hangi izinleri vereceğine sen karar verirsin." },
    { title: "Arka plan kontrolünü çalıştır", body: "Yardımcı geçici bir deneme penceresi açar, metin girer ve o pencerenin görüntüsünü alır. Çalışma sırasında imlecin veya etkin uygulamanın değişip değişmediğini kontrol eder." },
    { title: "Codewhale'e bağla", body: "Codewhale eklenti mağazasında Computer Use'u incele, güvenilir olarak işaretle ve etkinleştir. Yerel eylemlerin yardımcının Pause ve Stop denetimlerinden geçmesi için eklentinin 0.3.1 veya üzeri sürümünü kullan." },
  ],
  controlsTitle: "Çalışmaya devam et. Denetim sende kalsın.",
  controlsBody: "Desteklenen eylemler seçili uygulamada arka planda çalışır. Ön planda denetim gerektiren uygulamalar ve hareketler için senin onayın gerekir. Menü hedefi ve giriş modunu gösterir; Pause (Duraklat) yardımcının girişini askıya alır, Stop (Durdur) ise mevcut oturumlarını sonlandırır.",
  updateTitle: "Güncellemeler sen istediğinde",
  updateBody: "Uygulamadan Check for updates'i (Güncellemeleri denetle) seç. Bir güncellemeyi kurmadan önce indirmeyi, Codewhale imzasını ve Apple noter onayını doğrular; kurtarma için önceki uygulamayı saklar.",
  help: "Kurulum ve sorun giderme",
  notes: "Sürüm notları",
  demo: "Arka plan kontrolünü izle",
  source: "Kaynak kodu ve diğer platformlar",
  platforms: "Bu indirme Mac içindir. Windows ve Linux şu anda kaynak eklentisini ve ana makine tarafındaki kurulumu kullanır.",
  installTitle: "Mac için Computer Use",
  installLead: "Computer Use yardımcısıyla arka planda uygulama denetimi ekle. Mac izinlerini ayarla, bir arka plan kontrolü çalıştır ve yardımcının girişini menü çubuğundan duraklat ya da durdur.",
  installLink: "Computer Use indirme ve kurulum",
};
