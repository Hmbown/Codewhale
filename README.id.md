<!-- source: README.md sha256:ebf1c69de0dc -->
# Codewhale

Codewhale adalah agen sumber terbuka yang membaca proyek, mengedit berkas, menjalankan perintah, dan memeriksa hasil kerjanya dengan model yang dihosting atau model lokal pilihan Anda. Mulailah dengan satu tugas di terminal. Untuk pekerjaan yang lebih besar, bagikan sebagian pekerjaan kepada agen dengan model dan peran yang berbeda.

![Codewhale berjalan di terminal](web/public/codewhale-tui-171acee.png)

*Pratinjau terminal dari build pengembangan v0.9.12.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## Instalasi

Untuk instalasi baru di macOS atau Linux, gunakan rilis resmi GitHub:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

Installer memilih rilis terbaru yang sudah dipublikasikan. [Catatan perubahan](CHANGELOG.md) juga menjelaskan kandidat yang belum dipublikasikan untuk rilis berikutnya; perubahan tersebut baru disertakan dalam unduhan publik setelah rilisnya tersedia.

Di Windows, unduh installer atau arsip yang sesuai dari [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest). Untuk instalasi biner langsung yang sudah ada, jalankan `codewhale update`, atau `codewhale update --check` untuk memeriksa tanpa memasang. Updater menampilkan jalur executable dan mempertahankan build yang lebih baru. npm dan Cargo adalah pilihan sekunder; lihat [panduan instalasi](docs/INSTALL.md) untuk migrasi dari pengelola paket dan pengaturan PATH.

Saat pertama dijalankan, Codewhale membantu Anda menghubungkan penyedia atau mengonfigurasi Codewhale secara luring. Respons model memerlukan koneksi ke model yang dihosting atau model lokal. Codewhale juga mendukung npm dan Cargo sebagai jalur pengemasan sekunder, serta Docker, Nix, Scoop, Android/Termux, dan mirror CNB opsional. Instalasi yang sudah ada melalui pengelola paket akan menerima petunjuk migrasi. Lihat [bantuan instalasi dan PATH](docs/INSTALL.md).

Penyelesaian Tab cukup diaktifkan dengan satu perintah per shell — `codewhale completion bash|zsh|fish|powershell|elvish`. Lihat [penyelesaian shell](docs/INSTALL.md#8-shell-completions).

## Penggunaan

Buka terminal di folder proyek Anda dan jalankan `codewhale`. Pilih penyedia dengan `/provider` dan model dengan `/model`. Lalu jelaskan tugas yang konkret:

```text
Fix the failing tests and explain what changed.
```

Atau jalankan tugas tanpa membuka TUI:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale dapat membaca repositori Anda, mengedit berkas, menjalankan perintah, memeriksa hasil, dan terus bekerja menuju tujuan. Gunakan `/mode plan` untuk menelusuri tanpa mengubah berkas atau menjalankan perintah shell, dan `/mode work` saat Anda ingin agen melakukan perubahan. Tekan `Shift+Tab` untuk memilih Ask, Auto-Review, atau Full Access; [panduan mode dan izin](docs/MODES.md) menjelaskan tindakan yang diizinkan oleh masing-masing pilihan.

## Terminal, aplikasi, dan Computer Use

Terminal dan klien grafis terhubung ke Codewhale Runtime, yang menjalankan agen beserta alatnya:

- **Terminal:** `codewhale` membuka antarmuka interaktif; `codewhale exec` menjalankan tugas dari skrip atau job CI.
- **Browser lokal:** `codewhale web` membuka [klien web lokal](docs/WEB.md) bawaan untuk Runtime yang sama.
- **Aplikasi desktop Codewhale (GPUI):** aplikasi desktop native GPUI adalah arah klien produk (diputuskan 2026-09-14; peta tahap ada di docs/TRANSITION.md pada repo privat codehwhale-gpui). Aplikasi web yang dihosting di app.codewhale.net dihentikan bertahap; situs pemasaran, masuk, penagihan, halaman legal, dan unduhan tetap di web secara permanen. Ketersediaannya tercantum di [halaman produk](https://codewhale.net/en/product).

**Computer Use menambahkan alat untuk mengamati dan berinteraksi dengan aplikasi lain.** Plugin ini disertakan dalam kode sumber saat ini. Tinjau akses yang diminta dan aktifkan plugin sebelum digunakan; izin OS dan persyaratan platform tetap berlaku. Lihat [panduan Computer Use](crates/tui/plugins/computer-use/README.md) yang disertakan dan [pengaturan plugin](docs/PLUGINS.md).

Di VS Code, ekstensi CodeWhale yang dikelola komunitas terhubung ke Runtime lokal melalui sidebar. Pasang dari [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode); kode sumber ada di [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode).

## Mengapa Codewhale

- **Pilih model Anda.** Hubungkan penyedia terkelola atau model lokal melalui Ollama, vLLM, atau SGLang. Gunakan `/provider` untuk mengganti penyedia dan `/model` untuk memilih model.
- **Tetap memegang kendali.** Periksa tindakan yang diusulkan dan perubahan berkas yang dihasilkannya. Pengaturan persetujuan menentukan kapan peninjauan diperlukan; Full Access tetap mematuhi batas kebijakan yang wajib dipenuhi. `/undo` dan `/restore` membantu memulihkan perubahan ruang kerja.
- **Jaga agar pekerjaan panjang tetap teratur.** Simpan sesi, tetapkan `/goal` yang bertahan lama, tinjau alur kerja sebelum dijalankan, dan koordinasikan agen tanpa memasukkan instruksi internal mereka ke transkrip Anda.
- **Perluas agen yang sudah Anda miliki.** Hubungkan server MCP dan keterampilan, konfigurasikan hook, dan simpan peran agen sebagai berkas yang mudah dibaca di proyek atau pengaturan pribadi Anda.

Jalankan `/help` di TUI untuk melihat perintah dan pintasan papan ketik.

## Keamanan

Codewhale berjalan di mesin Anda dengan akses yang Anda berikan. Mode persetujuan dan aturan repositori membatasi tindakan agen; sandbox OS opsional menambahkan batas eksekusi yang lebih kuat jika didukung. Harga model yang belum diketahui tetap ditampilkan sebagai tidak diketahui, bukan dilaporkan gratis.

Baca [urutan otorisasi](docs/AUTHORIZATION_ORDER.md) untuk susunan kebijakan yang tepat dan [konfigurasi](docs/CONFIGURATION.md) untuk pengaturan lokal.

## Dokumentasi

- [Penyedia dan model lokal](docs/PROVIDERS.md)
- [Tim agen](docs/FLEET.md)
- [MCP](docs/MCP.md), [hook](docs/HOOKS.md), dan [konfigurasi](docs/CONFIGURATION.md)
- [Klien web lokal](docs/WEB.md)
- [Semua dokumentasi](docs)
- [Struktur repositori dan panduan kontribusi](CONTRIBUTING.md#project-structure)

## Bergabung dengan komunitas

**Laporan bug, ide fitur, dan pull request selalu diterima**, baik Anda telah memakai Codewhale selama berbulan-bulan maupun baru mencobanya. Jika penyedia belum tersedia, alur kerja terasa janggal, atau UI terminal menghambat Anda, [buat issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) atau [kirim pull request](CONTRIBUTING.md) agar kita dapat memperbaikinya bersama. Kontribusi pertama sangat disambut, dan kontributor tetap menerima kredit untuk pekerjaan yang digabungkan.

Bergabunglah di [Discord](https://discord.gg/37gfS3ksug), atau tambahkan Hunter di WeChat (`hunterbown`) dan mintalah untuk bergabung dengan grup Whale Brothers.

## Riwayat proyek

Codewhale bermula sebagai `deepseek-tui` dan tetap mempertahankan kompatibilitas konfigurasi serta sesinya. Kini Codewhale netral terhadap penyedia, dikelola secara independen, dan tidak berafiliasi dengan penyedia model mana pun.

Terima kasih kepada setiap kontributor dan komunitas sumber terbuka yang membantu proyek ini tumbuh. Lihat [catatan kontributor](docs/CONTRIBUTORS.md).

## Lisensi

[MIT](LICENSE). Bagian yang diadaptasi dari proyek sumber terbuka lain dicatat dalam [pemberitahuan pihak ketiga](docs/THIRD_PARTY_NOTICES.md).
