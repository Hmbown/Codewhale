# Berkontribusi pada Codewhale

Terima kasih atas minat Anda untuk berkontribusi pada Codewhale! Dokumen ini memberikan panduan dan instruksi untuk berkontribusi.

---

## Memulai (Getting Started)

### Prasyarat

- Rust 1.88 atau lebih baru (edisi 2024)
- Pengelola paket Cargo
- Git

### Menyiapkan Lingkungan Pengembangan

1. Fork dan klon repositori:
   ```bash
   git clone https://github.com/USERNAME_ANDA/CodeWhale.git
   cd CodeWhale
   ```

2. Kompilasi proyek:
   ```bash
   cargo build
   ```

3. Jalankan pengujian (tests):
   ```bash
   cargo test --workspace --all-features
   ```

4. Jalankan dalam mode pengembangan:
   ```bash
   cargo run --bin codewhale
   ```

---

## Alur Kerja Pengembangan

### Gaya Kode (Code Style)

- Jalankan `cargo fmt` sebelum melakukan commit untuk memastikan format kode konsisten.
- Jalankan `cargo clippy` dan atasi seluruh peringatan.
- Ikuti konvensi penamaan Rust (`snake_case` untuk fungsi/variabel, `CamelCase` untuk tipe data/struct).
- Tambahkan komentar dokumentasi untuk API publik.

### Pengujian

- Tulis pengujian untuk fungsionalitas baru.
- Pastikan seluruh pengujian yang ada lulus: `cargo test --workspace --all-features`.
- Tempatkan unit test di samping kode yang diuji (`#[cfg(test)]`), serta tambahkan pengujian integrasi di bawah direktori `tests/` pada crate yang bersangkutan.

---

## Pengajuan Pull Request (PR)

1. Buat branch baru dari `main` untuk fitur atau perbaikan Anda.
2. Pastikan `cargo fmt`, `cargo clippy`, dan pengujian lulus sebelum mengajukan PR.
3. Tulis judul dan deskripsi PR yang jelas yang menjelaskan tujuan perubahan.
4. Setiap kontributor akan mendapatkan kredit dalam changelog dan `docs/CONTRIBUTORS.md`.
