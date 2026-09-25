import type { HomeDict } from "../types";

/**
 * Indonesian home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — Bangun dan otomatisasikan dengan model pilihan Anda",
  metaDescription:
    "Bangun perangkat lunak, kelola berkas Anda, dan otomatisasikan tugas sehari-hari dengan agen sumber terbuka serta model AI yang dihosting atau dijalankan secara lokal sesuai pilihan Anda.",
  heroTitle: "Bangun dan otomatisasikan dengan model pilihan Anda",
  heroIntro:
    "{brand} menyediakan agen yang dapat membangun perangkat lunak, mengelola berkas Anda, dan mengubah tugas berulang menjadi alur kerja yang dapat digunakan kembali. Sampaikan apa yang ingin Anda capai dan pilih model yang dihosting atau dijalankan secara lokal sesuai kebutuhan tugas, dengan kebebasan untuk berganti penyedia selama bekerja.",
  getCodewhale: "Dapatkan Codewhale",
  heroInstallAria: "Perintah instalasi",
  exploreProduct: "Jelajahi produk",
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
  gainHeading:
    "Yang dapat Anda lakukan dengan Codewhale",
  gainLede:
    "Mulailah dengan proyek, pertanyaan, atau tugas yang ingin Anda otomatisasikan, lalu bekerja dengan satu agen atau bagi pekerjaan yang lebih besar ke beberapa agen.",
  gain: [
    [
      "Bangun sesuatu",
      "Jelaskan apa yang ingin Anda buat dan bekerja dengan agen yang dapat membaca kode Anda, mengedit berkas, menjalankan perintah, dan memeriksa hasilnya."
    ],
    [
      "Otomatisasikan pekerjaan sehari-hari",
      "Buat skrip dan alur kerja untuk tugas yang sering Anda ulangi, sehingga Anda dapat menjalankannya lagi dari terminal kapan pun dibutuhkan."
    ],
    [
      "Bekerja dengan berbagai model",
      "Gunakan model yang dihosting atau dijalankan secara lokal untuk agen Anda, dengan model dan peran yang berbeda menangani bagian pekerjaan yang sesuai."
    ]
  ],
  modelsHeading: "Pilihan model untuk setiap tugas",
  modelsBody:
    "Hubungkan langsung ke penyedia model yang dihosting, gunakan gateway untuk mengakses beberapa penyedia, atau jalankan model secara lokal, lalu pilih model yang digunakan setiap sesi selama Anda bekerja.",
  modelsFacts: [
    ["Hosted", "Kunci API Anda sendiri, disimpan dengan codewhale auth set --provider <id>"],
    ["Gateway", "Satu endpoint untuk banyak model, penyedia tetap Anda yang pilih"],
    ["Lokal", "vLLM, SGLang, Ollama di localhost — biasanya tanpa kunci"],
  ],
  modelsLink: "Jelajahi model dan penyedia",
  startHeading: "Mulai menggunakan Codewhale",
  startLede:
    "Setelah menginstal Codewhale dan menghubungkan model, Anda dapat menjelaskan tugas pertama di terminal dan menambahkan Fleet saat ingin beberapa agen berbagi pekerjaan.",
  startGuideLink: "Baca panduan memulai",
  startVocabularyLink: "Lihat kosakata produk",
  availabilityHeading:
    "Tempat Anda dapat menggunakan Codewhale",
  availabilityLede:
    "Anda dapat menggunakan Codewhale di terminal sekarang, sementara kami mengembangkan aplikasi web, aplikasi desktop, dan komputer cloud.",
  availability: [
    [
      "Terminal",
      "Dirilis",
      "Biner rilis GitHub untuk Linux, macOS, dan Windows; npm dan Cargo tersedia sebagai alternatif. Android di Termux masih dalam tahap pratinjau."
    ],
    [
      "Aplikasi web",
      "Pratinjau pengembangan",
      "Akses akun dan penautan peramban dalam pratinjau pengembangan."
    ],
    [
      "Desktop",
      "Build pengembangan",
      "Aplikasi macOS masih dalam pengembangan; unduhan untuk publik akan tersedia nanti."
    ],
    [
      "Komputer cloud",
      "Dalam pengembangan",
      "Komputer yang dihosting untuk menjalankan tugas Anda."
    ]
  ],
  availabilityNote:
    "Anda dapat menggunakan terminal tanpa akun Codewhale, dan penggunaan model yang dihosting ditagih oleh penyedia Anda.",
  accountLink: "Buat akun",
  surfacesHeading: "Cara bekerja dengan Codewhale",
  surfaces: [
    ["TUI", "Kerja terminal interaktif"],
    ["codewhale exec", "Skrip dan CI"],
    ["Klien web lokal","Antarmuka localhost; ruang kerja peramban yang dihosting masih dalam pengembangan"],
    ["Runtime API + MCP", "Integrasi lokal"],
    ["Fleet","Beberapa agen mengerjakan satu tugas"],
  ],
  runtimeLink: "Jelajahi integrasi",
  installBandHeading: "Instal Codewhale di macOS atau Linux",
  copy: "Salin",
  copied: "Tersalin ✓",
  binaries: "Biner",
  chinaMirrors: "Mirror Tiongkok",
  installGuideLink: "Baca panduan instalasi",
  communityHeading: "Bantu membuat Codewhale lebih baik",
  communityBody:
    "Baik Anda menemukan bug, memiliki ide untuk fitur, maupun ingin mengirim pull request pertama, kami ingin mendengar dari Anda dan bekerja sama menentukan langkah berikutnya.",
  communityLinksAria: "Tautan komunitas",
  contribute: "Kirim pull request",
};
