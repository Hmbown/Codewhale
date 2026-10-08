import type { HomeDict } from "../types";

/**
 * Indonesian home dictionary — native copy for the whale-road landing page.
 * Translates the English reference in en/home.ts: an open-source coding
 * agent for any model, control you can check, and availability stated per
 * surface as it is today. Product vocabulary stays literal (Plan / Work /
 * Operate, Ask / Auto-Review / Full Access, Codewhale, codewhale exec,
 * Fleet, MCP, Runtime, /receipts).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale: agen pemrograman sumber terbuka untuk model apa pun",
  metaDescription:
    "Codewhale adalah agen pemrograman sumber terbuka untuk terminal Anda. Codewhale membaca proyek Anda, mengedit berkas, dan menjalankan pengujian Anda dengan model yang dihosting atau lokal pilihan Anda.",
  heroTitle: "Agen pemrograman sumber terbuka untuk model apa pun",
  heroIntro:
    "{brand} membaca proyek Anda, mengedit berkas, dan menjalankan pengujian Anda dari terminal. Hubungkan model yang dihosting atau lokal, lalu pilih tindakan mana yang memerlukan persetujuan Anda.",
  getCodewhale: "Instal Codewhale",
  heroInstallAria: "Perintah instalasi",
  exploreProduct: "Lihat cara kerjanya",
  shotPreview: "Pratinjau terminal",
  shotBuild: "build pengembangan v{version}",
  screenshotAlt:
    "Build pengembangan Codewhale v{version}: tanda paus, sesi baru, kolom pesan, izin Ask, mode Work, dan status model. Dirender dari keluaran nyata terminal terisolasi.",
  latestRelease: "Rilis terbaru {tag}",
  releaseUnavailable: "Status rilis tidak tersedia",
  currentSource: "Sumber",
  sourceCandidate: "Belum dirilis",
  publishedRelease: "dirilis",
  figcaptionSourceCandidate: "belum dirilis",
  chapterTerminal: "Terminal Anda",
  chapterTerminalTitle: "Ikuti setiap editan dan perintah saat dijalankan",
  gainHeading: "Delegasikan tugas dan tetap memegang kendali",
  gainLede:
    "Minta sebuah hasil: memperbaiki bug, menjelaskan modul, atau mengotomatisasikan tugas yang sering Anda ulangi. Mulai dengan satu agen, lalu tambahkan agen lain saat pekerjaan bertambah besar.",
  gain: [
    [
      "Ubah kode dan periksa hasilnya",
      "Agen memeriksa proyek Anda, mengedit berkas, dan menjalankan pengujian Anda. Ikuti setiap editan dan hasil perintah selama agen bekerja."
    ],
    [
      "Otomatisasikan pekerjaan berulang",
      "Jalankan codewhale exec dari skrip dan CI. Gunakan Fleet untuk membagi pekerjaan yang lebih besar ke beberapa agen."
    ],
    [
      "Tetap memegang kendali",
      "Atur izin sebelum pekerjaan dimulai, jawab permintaan persetujuan, dan hentikan tugas kapan saja. Jalankan /receipts untuk menampilkan setiap berkas, perintah, dan persetujuan dalam satu sesi."
    ]
  ],
  chapterModels: "Model Anda",
  modelsHeading: "Pilih model untuk setiap tugas",
  modelsBody:
    "Untuk setiap sesi, pilih penyedia bawaan, endpoint apa pun yang kompatibel dengan OpenAI, atau model lokal. Koneksi model Anda tetap terpisah dari akun Codewhale mana pun.",
  modelsFacts: [
    ["Hosted", "Kunci API Anda sendiri, disimpan dengan codewhale auth set --provider <id>"],
    ["Gateway", "Satu endpoint untuk banyak model; Anda tetap memilih penyedianya"],
    ["Lokal", "vLLM, SGLang, atau Ollama di localhost, biasanya tanpa kunci"],
  ],
  modelsLink: "Lihat model dan penyedia",
  startHeading: "Instal, hubungkan model, jalankan tugas",
  startLede:
    "Jalankan tugas pertama Anda dalam tiga langkah dari folder proyek Anda. Tambahkan Fleet nanti jika pekerjaan memerlukan beberapa agen.",
  startGuideLink: "Ikuti panduan memulai",
  startVocabularyLink: "Lihat kosakata produk",
  chapterAvailability: "Tempat menjalankan",
  availabilityHeading: "Gunakan di terminal Anda hari ini",
  availabilityLede:
    "Gunakan terminal, klien peramban lokal, atau CodeWhale GUI dari komunitas sekarang. Aplikasi desktop dan aplikasi web hosted yang dibangun ulang sedang dikembangkan dan berbagi model sesi yang sama.",
  availability: [
    [
      "Terminal dan peramban lokal",
      "Dirilis",
      "Instal di Linux, macOS, atau Windows, lalu jalankan codewhale, atau codewhale web untuk klien peramban lokal. npm dan Cargo juga dapat digunakan; Android di Termux masih berupa pratinjau."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "Tersedia",
      "Proyek terpisah yang dipelihara oleh komunitas: obrolan, utas, dan perubahan berkas di sidebar VS Code di atas Codewhale Runtime yang sama. Pasang dari VS Code Marketplace.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "Aplikasi web hosted",
      "Pratinjau pengembangan",
      "Sedang dibangun ulang agar sesuai dengan aplikasi desktop. Saat ini Anda dapat masuk, lalu mengetik /rc di sesi terminal yang sedang berjalan untuk melanjutkannya di web; eksekusi tugas hosted masih dalam kualifikasi."
    ],
    [
      "Desktop",
      "Build pengembangan",
      "Aplikasi native yang sedang menjadi klien utama Codewhale: folder, percakapan, dan koneksi model dalam satu jendela. Belum ada unduhan publik."
    ],
    [
      "Komputer cloud",
      "Dalam pengembangan",
      "Komputer yang dihosting yang menjalankan tugas Anda."
    ]
  ],
  availabilityNote:
    "Terminal, peramban lokal, dan GUI tidak memerlukan akun Codewhale. Web hosted dan desktop menggunakan akun, yang tidak menggantikan koneksi model Anda; penyedia Anda menagih penggunaan dengan kunci Anda sendiri.",
  accountLink: "Buat akun",
  surfacesHeading: "Perluas jangkauan agen",
  surfaces: [
    ["Berkas dan perintah", "Baca proyek, edit berkas, jalankan pengujian, dan periksa keluaran sesuai izin yang Anda tetapkan."],
    ["Plugin dan MCP", "Hubungkan lebih banyak alat dan layanan. Setiap plugin tetap nonaktif sampai Anda meninjau dan mengaktifkannya."],
    ["Computer Use · pratinjau", "Plugin yang memungkinkan agen melihat dan mengoperasikan aplikasi lain. Anda yang mengaktifkannya dan memberikan izin sistem yang dimintanya."],
    ["Sesi tersimpan", "Simpan percakapan dan hasil alat bersama-sama, lalu lanjutkan pekerjaan tanpa mulai dari awal. Peramban lokal membuka sesi yang sama di komputer Anda."],
    ["Fleet", "Tugaskan bagian-bagian tugas ke agen dengan model dan peran yang berbeda, lalu ikuti kemajuannya."],
  ],
  runtimeLink: "Lihat semua integrasi",
  installBandHeading: "Instal di macOS atau Linux",
  copy: "Salin",
  copied: "Tersalin ✓",
  binaries: "Biner",
  chinaMirrors: "Mirror Tiongkok",
  installGuideLink: "Baca panduan instalasi",
  communityHeading: "Bangun Codewhale bersama kami",
  communityBody:
    "Laporkan bug, usulkan fitur, atau kirim pull request pertama Anda di GitHub. Kami menyambut perbaikan kecil yang sudah diuji.",
  communityLinksAria: "Tautan komunitas",
  contribute: "Kirim pull request",
};
