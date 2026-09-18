import type { ComputerUseDict } from "../types";

/**
 * Indonesian dictionary for `app/[locale]/computer-use/page.tsx` and the
 * Computer Use section of the install page. Product names, the menu items
 * Pause, Stop and Check for updates, and the macOS setting names stay as
 * the app shows them; formal "Anda" address matches the rest of the site.
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Computer Use untuk Mac · Codewhale",
  metaDescription: "Unduh dan siapkan Codewhale Computer Use untuk Mac. Kendali aplikasi di latar belakang, pengaturan izin, serta kontrol Pause dan Stop di tangan Anda.",
  title: "Computer Use",
  lead: "Biarkan Codewhale bekerja di aplikasi Anda sementara Anda tetap bekerja. Pembantu Mac ini menghadirkan pengaturan izin, kendali aplikasi di latar belakang, dan cara menjeda atau menghentikan input langsung dari bar menu.",
  publisher: "Oleh Codewhale",
  download: "Unduh untuk Mac",
  downloadZip: "Arsip ZIP (digunakan oleh pembaru dalam aplikasi)",
  requirements: "macOS 13.5 atau lebih baru · Apple silicon dan Intel",
  included: "Cukup satu unduhan aplikasi. Tidak perlu memasang Node atau kompiler secara terpisah.",
  pendingTitle: "Unduhan Mac sedang disiapkan",
  pendingBody: "Pemasang publik akan muncul di sini setelah notarisasi Apple dan pemeriksaan rilis selesai.",
  unavailableTitle: "Ketersediaan unduhan tidak dapat diperiksa",
  unavailableBody: "Muat ulang halaman ini untuk mencoba lagi, atau lihat rilis yang sudah diterbitkan di bawah.",
  releases: "Rilis yang diterbitkan",
  receipt: "Detail verifikasi unduhan",
  setup: "Siapkan Mac Anda",
  steps: [
    { title: "Instal aplikasi", body: "Buka disk image, lalu seret Codewhale Computer Use ke folder “Aplikasi”. Buka aplikasinya dari “Aplikasi”, kemudian pilih Computer Use dari ikon paus di bar menu Anda." },
    { title: "Tinjau izin", body: "Gunakan tombol penyiapan untuk membuka “Aksesibilitas” dan “Perekaman Layar” di Pengaturan Sistem. Anda yang menentukan izin mana yang diberikan." },
    { title: "Jalankan pemeriksaan latar belakang", body: "Pembantu membuka jendela latihan sementara, mengetikkan teks, dan menangkap tampilan jendela tersebut. Ia memeriksa apakah penunjuk atau aplikasi aktif berubah selama proses berlangsung." },
    { title: "Hubungkan ke Codewhale", body: "Tinjau, percayai, dan aktifkan Computer Use di pasar plugin Codewhale. Gunakan plugin versi 0.3.1 atau lebih baru agar tindakan lokal berjalan melalui kontrol Pause dan Stop milik pembantu." },
  ],
  controlsTitle: "Tetap bekerja. Tetap memegang kendali.",
  controlsBody: "Tindakan yang didukung berjalan pada aplikasi terpilih di latar belakang. Aplikasi dan gestur yang membutuhkan kendali latar depan memerlukan persetujuan Anda. Menu menampilkan aplikasi target dan mode input; Pause menangguhkan input pembantu, dan Stop mengakhiri sesi pembantu yang sedang berjalan.",
  updateTitle: "Perbarui saat Anda mau",
  updateBody: "Pilih Check for updates di aplikasi. Sebelum memasang pembaruan, aplikasi memeriksa berkas unduhan, tanda tangan Codewhale, dan notarisasi Apple, serta menyimpan versi sebelumnya untuk pemulihan.",
  help: "Penyiapan dan pemecahan masalah",
  notes: "Catatan rilis",
  demo: "Lihat pemeriksaan latar belakang",
  source: "Kode sumber dan platform lain",
  platforms: "Unduhan ini untuk Mac. Windows dan Linux saat ini menggunakan plugin dari kode sumber dan penyiapan di sisi host.",
  installTitle: "Computer Use untuk Mac",
  installLead: "Tambahkan kendali aplikasi di latar belakang dengan pembantu Computer Use. Siapkan izin Mac, jalankan pemeriksaan latar belakang, dan jeda atau hentikan input pembantu dari bar menu Anda.",
  installLink: "Unduh dan siapkan Computer Use",
};
