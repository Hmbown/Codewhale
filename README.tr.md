<!-- source: README.md sha256:bbb8bfb61e57 -->
# Codewhale

Codewhale, seçtiğiniz barındırılan veya yerel bir modeli kullanarak projenizi okuyan, dosyaları düzenleyen, komutları çalıştıran ve yaptığı işi kontrol eden açık kaynaklı bir ajandır. Terminalde tek bir görevle başlayın. Daha büyük bir işte, işin bölümlerini farklı model ve rollere sahip ajanlara verin.

![Terminalde çalışan Codewhale](web/public/codewhale-tui-5765d80.png)

*v0.10.0 geliştirme derlemesinden terminal önizlemesi.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## Kurulum

macOS veya Linux üzerinde yeni kurulum için resmî GitHub sürümünü kullanın:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

Yükleyici, yayımlanmış en son sürümü seçer. [Değişiklik günlüğü](CHANGELOG.md), bir sonraki sürümün henüz yayımlanmamış adayını da açıklar; bu değişiklikler, sürüm kullanıma sunulana kadar yayımlanmış indirmelere dahil edilmez.

Windows’ta [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest) üzerinden uygun yükleyiciyi veya arşivi indirin. Mevcut doğrudan kurulumu güncellemek için `codewhale update`, yalnızca kontrol etmek için `codewhale update --check` çalıştırın. Güncelleyici çalıştırılabilir dosyanın yolunu gösterir ve daha yeni derlemeleri korur. npm ve Cargo ikincil paketleme seçenekleridir. Paket yöneticisinden geçiş ve PATH ayarları için [kurulum kılavuzuna](docs/INSTALL.md) bakın.

İlk çalıştırma doğrudan mesaj yazma alanını açar; sizi bir kurulum adımından geçirmez. Model yanıtları için barındırılan ya da yerel bir modele bağlantı gerekir: bağlanana kadar açılış ekranında "no model connected" yazar. Barındırılan bir anahtar eklemek veya yerel bir çalışma ortamı seçmek için `/provider` komutunu çalıştırın (ya da F3’e basın). Ollama zaten bir sohbet modeliyle çalışıyorsa Codewhale kendiliğinden ona geçer. Codewhale, ikincil paketleme seçenekleri olarak npm ve Cargo’nun yanı sıra Docker, Nix, Scoop, Android/Termux ve isteğe bağlı CNB aynasını da destekler. Paket yöneticisiyle yönetilen mevcut kurulumlar için geçiş talimatları sağlanır. [Kurulum ve PATH yardımına](docs/INSTALL.md) bakın.

Her kabukta Tab tamamlama tek bir komutla etkinleştirilir — `codewhale completion bash|zsh|fish|powershell|elvish`. [Kabuk tamamlamalarına](docs/INSTALL.md#8-shell-completions) bakın.

## Kullanım

Proje klasörünüzde bir terminal açın ve `codewhale` komutunu çalıştırın. `/provider` ile sağlayıcınızı, `/model` ile modelinizi seçin. Ardından somut bir görev tarif edin:

```text
Fix the failing tests and explain what changed.
```

TUI’yi açmadan da bir görev çalıştırabilirsiniz:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale deponuzu okuyabilir, dosyaları düzenleyebilir, komutları çalıştırabilir, sonuçları inceleyebilir ve bir hedefe doğru çalışmayı sürdürebilir. Dosyaları değiştirmeden veya kabuk komutlarını çalıştırmadan inceleme yapmak için `/mode plan`, değişiklik yapmak istediğinizde ise `/mode work` kullanın. Ask, Auto-Review veya Full Access seçeneklerinden birini seçmek için `Shift+Tab` tuşlarına basın; [modlar ve izinler kılavuzu](docs/MODES.md) her birinin nelere izin verdiğini açıklar.

## Terminal, uygulamalar ve Computer Use

Terminal ve grafik istemciler, ajanı ve araçlarını çalıştıran Codewhale Runtime’a bağlanır:

- **Terminal:** `codewhale` etkileşimli arayüzü açar; `codewhale exec` bir betikten veya CI işinden görev çalıştırır.
- **Yerel tarayıcı:** `codewhale web`, aynı çalışma zamanı için paketle birlikte gelen [yerel web istemcisini](docs/WEB.md) açar.
- **Codewhale masaüstü uygulaması (GPUI):** ayrı bir depoda geliştirilen yerel masaüstü uygulaması, oturum açmış kullanıcılar için ürün istemcisinin yönüdür. app.codewhale.net'teki barındırılan web uygulaması ona göre yeniden oluşturulacak; pazarlama sitesi, oturum açma, faturalandırma, yasal ve indirme sayfaları web'de kalır. Kullanılabilirlik [ürün sayfasında](https://codewhale.net/en/product) belirtilir.

**Computer Use, diğer uygulamaları gözlemlemek ve onlarla etkileşime girmek için araçlar ekler.** Eklenti mevcut kaynak koduna dahildir. Kullanmadan önce istediği erişimi gözden geçirin ve eklentiyi etkinleştirin; işletim sistemi izinleri ve platform gereksinimleri geçerliliğini korur. Birlikte gelen [Computer Use kılavuzuna](crates/tui/plugins/computer-use/README.md) ve [eklenti kurulumuna](docs/PLUGINS.md) bakın.

Topluluk tarafından bakımı yapılan VS Code için CodeWhale eklentisi, kenar çubuğundan yerel Runtime’a bağlanır. [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode) üzerinden kurun; kaynak kodu [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode) adresindedir.

## Neden Codewhale

- **Modellerinizi seçin.** Barındırılan sağlayıcılara veya Ollama, vLLM ya da SGLang üzerinden yerel modellere bağlanın. Sağlayıcı değiştirmek için `/provider`, model seçmek için `/model` kullanın.
- **Kontrolü elinizde tutun.** Önerilen eylemleri ve bunların sonucunda dosyalarda oluşan değişiklikleri inceleyin. Onay ayarları ne zaman inceleme gerektiğini belirler; Full Access de politikanın kesin sınırlarına uyar. `/undo` ve `/restore`, değişikliklerden sonra çalışma alanını geri yüklemenize yardımcı olur.
- **Uzun süren işleri düzenli tutun.** Oturumları kaydedin, kalıcı bir `/goal` belirleyin, iş akışlarını çalışmadan önce gözden geçirin ve ajanların iç talimatlarını konuşmanıza taşımadan onları koordine edin.
- **Elinizdeki ajanı genişletin.** MCP sunucularını ve becerileri bağlayın, hook’ları yapılandırın ve ajan rollerini projenizde veya kişisel ayarlarınızda okunabilir dosyalar olarak saklayın.

Komutları ve klavye kısayollarını görmek için TUI’de `/help` komutunu çalıştırın.

## Güvenlik

Codewhale, verdiğiniz erişimle kendi makinenizde çalışır. Onay modları ve depo kuralları ajanın yapabileceklerini sınırlar; desteklenen ortamlarda isteğe bağlı işletim sistemi sandbox’ı daha güçlü bir yürütme sınırı ekler. Bilinmeyen model fiyatları ücretsiz olarak bildirilmek yerine bilinmeyen olarak kalır.

Politikaların kesin sıralaması için [yetkilendirme sırasını](docs/AUTHORIZATION_ORDER.md), yerel ayarlar için [yapılandırmayı](docs/CONFIGURATION.md) okuyun.

## Belgeler

- [Sağlayıcılar ve yerel modeller](docs/PROVIDERS.md)
- [Ajan ekipleri](docs/FLEET.md)
- [MCP](docs/MCP.md), [hook’lar](docs/HOOKS.md) ve [yapılandırma](docs/CONFIGURATION.md)
- [Yerel web istemcisi](docs/WEB.md)
- [Tüm belgeler](docs/README.md)
- [Depo yapısı ve katkıda bulunma rehberi](CONTRIBUTING.md#project-structure)

## Topluluğa katılın

**Hata bildirimleri, özellik fikirleri ve pull request’ler memnuniyetle karşılanır**; Codewhale’i aylardır kullanıyor olmanız ya da ilk kez denemeniz fark etmez. Bir sağlayıcı eksikse, bir iş akışı kullanışsızsa veya terminal arayüzü işinizi zorlaştırıyorsa birlikte iyileştirebilmemiz için [bir issue açın](https://github.com/Hmbown/CodeWhale/issues/new/choose) veya [bir pull request gönderin](CONTRIBUTING.md). İlk katkılar memnuniyetle karşılanır ve katkıda bulunanların projeye alınan çalışmaları üzerindeki emeği kayda geçer.

[Discord’a](https://discord.gg/37gfS3ksug) katılın veya WeChat’te Hunter’ı (`hunterbown`) ekleyip Whale Brothers grubuna katılmak istediğinizi belirtin.

## Proje geçmişi

Codewhale, `deepseek-tui` olarak başladı ve onun yapılandırması ile oturumlarıyla uyumluluğunu hâlâ koruyor. Artık sağlayıcılardan bağımsızdır, bağımsız olarak sürdürülür ve herhangi bir model sağlayıcısıyla bağlantılı değildir.

Projeyi büyütmeye yardımcı olan tüm katkıcılara ve açık kaynak topluluklarına teşekkürler. [Katkıcı kaydına](docs/CONTRIBUTORS.md) bakın.

## Lisans

[MIT](LICENSE). Diğer açık kaynak projelerinden uyarlanan bölümler [üçüncü taraf bildirimlerinde](docs/THIRD_PARTY_NOTICES.md) kayıtlıdır.
