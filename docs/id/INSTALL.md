# Menginstal Codewhale

Halaman ini mencakup setiap jalur instalasi yang didukung dan penanganan masalah umum saat instalasi gagal, termasuk **Linux ARM64** dan platform lainnya.

Jika Anda hanya menginginkan versi singkat, lihat [README utama](../../README.md#install) atau [README Bahasa Indonesia](../../README.id.md#instalasi).

Perintah `latest` memilih rilis yang sudah diterbitkan, bukan build kandidat dari
kode sumber. Pada pemeriksaan 2026-09-04, rilis stabil terbaru adalah
[v0.9.11](https://github.com/codewhale-hq/CodeWhale/releases/tag/v0.9.11).

---

## 1. Skrip Instalasi Web (macOS & Linux)

Untuk instalasi baru di macOS dan Linux, gunakan installer resmi GitHub:

```bash
curl -fsSL https://codewhale.net/install.sh | sh
```

Skrip ini akan mengunduh biner rilis `codewhale` dan `codew` yang cocok, memverifikasinya terhadap `codewhale-artifacts-sha256.txt`, dan menginstalnya ke `~/.local/bin` secara bawaan. Nama aset `codewhale-tui-*` hanya dipertahankan untuk kompatibilitas updater lama dan bukan perintah ketiga.

Di Windows, pilih installer atau arsip yang sesuai dari
[GitHub Releases](https://github.com/codewhale-hq/CodeWhale/releases/latest).
Untuk instalasi biner langsung yang sudah ada:

```bash
codewhale update --check
codewhale update
```

Updater menampilkan jalur executable yang diperbarui dan mencoba GitHub lebih
dahulu. Build yang lebih baru tetap dipertahankan; kandidat v0.9.12 tidak diturunkan
ke rilis publik v0.9.11. npm dan Cargo tetap tersedia sebagai pilihan sekunder.

### Direktori sudah terisi atau instalasi dikelola paket

Installer tidak memakai `sudo` otomatis dan menolak berkas berbeda atau symlink
yang sudah ada. Updater juga mempertahankan berkas milik pengelola paket dan
menolak perintah lain di direktori yang sama bila isinya berbeda. Untuk migrasi,
buat direktori pengguna baru tanpa menghapus instalasi lama:

```bash
mkdir -p "$HOME/.local"
codewhale_install_dir="$(mktemp -d "$HOME/.local/codewhale-release.XXXXXX")"
curl -fsSL https://codewhale.net/install.sh | CODEWHALE_INSTALL_DIR="$codewhale_install_dir" sh
"$codewhale_install_dir/codewhale" --version
export PATH="$codewhale_install_dir:$PATH"
hash -r
command -v codewhale codew
```

Setelah memverifikasi versi dan jalurnya, simpan direktori tersebut di awal PATH
dalam konfigurasi shell. Pembaruan berikutnya memakai
`"$codewhale_install_dir/codewhale" update`. Gunakan pengelola paket untuk memperbarui
salinan yang tetap dikelola npm atau Cargo. Lihat
[panduan migrasi dan PATH](../INSTALL.md#migrating-from-npm-cargo-or-another-installation)
untuk rincian, termasuk Windows.

---

## 2. Platform yang Didukung

Rilis v0.9.11 menyediakan aset GitHub berikut. Adanya aset bukan bukti pengujian
pada setiap perangkat; Android/Termux tetap berstatus pratinjau. Jalur npm dan
Cargo bergantung pada paket yang diterbitkan dan dukungan platformnya.

| Platform | Arsitektur | Aset Rilis GitHub | `npm install` | `cargo install` |
| --- | --- | --- | :---: | :---: |
| Linux | x64 (x86_64) | `codewhale-linux-x64`, `codew-linux-x64` | ✅ | ✅ |
| Linux | arm64 | `codewhale-linux-arm64`, `codew-linux-arm64` | ✅ | ✅ |
| Android / Termux | arm64 (aarch64) | `codewhale-android-arm64.tar.gz` (pratinjau) | ⚠️ Pratinjau | ⚠️ Pratinjau |
| macOS | x64 | `codewhale-macos-x64`, `codew-macos-x64` | ✅ | ✅ |
| macOS | arm64 (M-series) | `codewhale-macos-arm64`, `codew-macos-arm64` | ✅ | ✅ |
| Windows | x64 | `codewhale-windows-x64.exe`, `codew-windows-x64.exe` | ✅ | ✅ |
| Windows | arm64 | `codewhale-windows-arm64.exe`, `codew-windows-arm64.exe` | ✅ | ✅ |

Untuk platform tanpa prebuilt yang kompatibel, periksa dukungan toolchain dan
dependensinya pada [panduan build dari sumber](../INSTALL.md#7-build-from-source).

---

## 3. Instalasi via npm

npm adalah pilihan instalasi sekunder yang memakai paket yang sudah diterbitkan:

```bash
npm install -g codewhale
```

Bagi pengguna Linux/macOS, pastikan direktori biner global npm berada di dalam `$PATH` Anda.

---

## 4. Instalasi via Cargo (Kompilasi dari Sumber Kode)

Jika Anda ingin mengompilasi biner langsung dari sumber kode menggunakan Rust:

```bash
cargo install codewhale-cli --locked
```

Persyaratan sistem:
- Rust toolchain (versi stable terbaru)
- Dependensi `libdbus-1-dev` atau `pkg-config` pada Linux untuk integrasi keyring OS.

---

## 5. Android / Termux

Termux berjalan di atas Bionic libc Android dan menggunakan `$PREFIX` sebagai
awalan Unix-nya. Dukungan perangkat tetap **pratinjau**. Gunakan arsip Android
`codewhale-android-arm64.tar.gz` dari rilis GitHub yang menyediakannya (termasuk
v0.9.11), verifikasi dengan `codewhale-bundles-sha256.txt` dari rilis yang sama,
lalu jalankan installer arsip dengan `PREFIX="$PREFIX"`.
Ikuti [langkah Android / Termux](../INSTALL.md#android--termux-arm64).
Installer web macOS/Linux dan aset `codewhale-linux-arm64` bukan jalur Android.

Jika tidak ada arsip Android yang kompatibel atau Anda sedang memvalidasi build
dari sumber, Cargo tetap menjadi pilihan pratinjau di dalam Termux:

```bash
pkg install -y rust clang pkg-config make git
cargo install codewhale-cli --locked
```

---

## 6. Migrasi dari `deepseek-tui`

Jika Anda sebelumnya menggunakan `deepseek-tui` atau menemui
`MISSING_COMPANION_BINARY`, gunakan migrasi GitHub ke direktori baru di atas dan
periksa jalur `codewhale` serta `codew`. Runtime saat ini berada dalam satu biner;
tidak perlu mengunduh runtime pendamping lain.

Instalasi baru memakai `~/.codewhale/`; konfigurasi dan sesi lama di
`~/.deepseek/` tetap dapat dibaca sebagai fallback dan tidak dihapus otomatis.
Lihat [jalur lama](../LEGACY_PATHS.md) dan [catatan perubahan nama](REBRAND.md).
