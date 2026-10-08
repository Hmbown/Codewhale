import type { HomeDict } from "../types";

/** Indonesian home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: berkarya dengan model dan alat pilihanmu",
  metaDescription:
    "Buat aplikasi, otomatisasikan alur kerja, dan gunakan alat terhubung dengan Codewhale. Sumber terbuka, dengan API model milikmu atau inferensi lokal dan mandiri.",
  heroTitle: "Wujudkan apa yang kamu bayangkan.",
  heroIntro:
    "Buat aplikasi, otomatisasikan alur kerja, atau ubah hasil riset menjadi sesuatu yang berguna. {brand} bekerja dengan API model yang sudah kamu gunakan, inferensimu sendiri, dan alat yang kamu hubungkan.",
  getCodewhale: "Instal Codewhale",
  heroInstallAria: "Perintah instalasi",
  exploreProduct: "Jelajahi Codewhale",
  shotPreview: "Pratinjau terminal",
  screenshotAlt:
    "Codewhale v{version}: tangkapan terminal berisi percakapan, kolom pesan, dan kontrol sesi.",
  latestRelease: "Rilis terbaru {tag}",
  releaseUnavailable: "Status rilis tidak tersedia",
  currentSource: "Sumber",
  sourceCandidate: "Belum dirilis",
  publishedRelease: "dirilis",
  gainHeading: "Buat sesuatu yang berguna.",
  gainLede:
    "Mulai dari hasil yang kamu inginkan. Codewhale bekerja dengan berkas, perintah, dan alat terhubung; kamu menentukan akses dan persetujuannya.",
  gain: [
    [
      "Buat aplikasi dan alat",
      "Ubah ide menjadi aplikasi yang berjalan, skrip berguna, atau fitur dalam proyek yang ada. Biarkan agen menulis, menjalankan, dan menguji bersamamu."
    ],
    [
      "Otomatiskan pekerjaan berulang",
      "Ubah tugas berulang menjadi alur kerja dari terminal, skrip, atau CI. Gunakan tim agen Fleet saat pekerjaan bisa berjalan paralel."
    ],
    [
      "Hubungkan alat yang kamu pakai",
      "Hubungkan alat seperti Gmail dan Slack melalui plugin, server MCP, atau API. Gunakan layanan tersebut bersama berkas dan perintahmu."
    ]
  ],
  exampleTasks: [
    "Buat aplikasi untuk membuat janji temu.",
    "Ubah CSV penjualan menjadi laporan mingguan yang bisa saya buat ulang.",
    "Ubah pesan dari email saya yang sudah terhubung menjadi daftar tindakan.",
  ],
  // A static example report built from local sample orders.
  reportTitle: "Laporan penjualan mingguan",
  reportSampleLabel: "Contoh laporan · data sampel",
  reportDescription: "Minta Codewhale mengelompokkan pesanan per minggu dan menyimpan prosesnya untuk CSV berikutnya.",
  reportSourceLabel: "Data masukan:",
  reportColumns: ["Awal minggu","Pesanan","Penjualan (USD)"],
  reportTotalLabel: "Jumlah",
  reportTrend: "Perubahan penjualan dari minggu pertama ke minggu terakhir: {change}.",
  reportDownloadLabel: "Unduh laporan CSV",
  chapterModels: "Model Anda",
  modelsHeading: "Tetap gunakan model pilihanmu.",
  modelsBody:
    "Hubungkan API model yang sudah kamu bayar, gunakan gateway yang kompatibel, atau jalankan inferensi di perangkat kerasmu. Pilih model per sesi dan untuk setiap agen dalam Fleet.",
  modelsFacts: [
    [
      "Akun API milikmu",
      "Hubungkan OpenAI, Anthropic, Google, atau DeepSeek dengan kuncimu sendiri."
    ],
    [
      "Gateway milikmu",
      "Gunakan endpoint yang kompatibel dengan OpenAI dan pilih model yang disediakannya."
    ],
    [
      "Inferensi milikmu",
      "Jalankan model lokal atau yang dihosting sendiri dengan Ollama, vLLM, atau SGLang."
    ]
  ],
  modelsLink: "Lihat model dan penyedia",
  startHeading: "Bawa tugasmu. Mulai bekerja.",
  startLede:
    "Pasang Codewhale, hubungkan model, dan beri pekerjaan yang berguna. Mulai dengan satu agen; tambahkan alat atau tim saat diperlukan.",
  startGuideLink: "Ikuti panduan memulai",
  startVocabularyLink: "Lihat kosakata produk",
  chapterAvailability: "Tempat menjalankan",
  availabilityHeading: "Mulai dari terminal.",
  availabilityLede:
    "Terminal dan klien peramban lokal sudah tersedia. Aplikasi desktop native dan aplikasi web yang dihosting sedang dikembangkan ulang.",
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
    "Terminal, peramban lokal, dan GUI tidak memerlukan akun Codewhale. Web hosted dan aplikasi desktop menggunakan akun. Jika Anda memakai kunci penyedia milik Anda sendiri, penyedia tersebut menagih penggunaan itu.",
  accountLink: "Buat akun",
  surfacesHeading: "Satu tugas. Berkas, aplikasi, dan agenmu.",
  surfaces: [
    [
      "Berkas dan terminal",
      "Buat berkas, jalankan perintah, periksa data, dan uji hasilmu. Kamu menentukan folder kerja dan izin."
    ],
    [
      "Plugin dan aplikasi terhubung",
      "Tambahkan keterampilan dan alat melalui plugin dan MCP. Tinjau dan aktifkan koneksi yang boleh digunakan agen."
    ],
    [
      "Peramban dan komputer · pratinjau",
      "Gunakan alat peramban dan plugin Computer Use untuk bekerja di aplikasi dan situs dengan akses yang kamu berikan."
    ],
    [
      "Sesi yang bisa dilanjutkan",
      "Simpan percakapan, hasil alat, dan riwayat kerja bersama. Lanjutkan tugas di terminal atau klien peramban lokal."
    ],
    [
      "Tim agen",
      "Gunakan Fleet untuk membagi pekerjaan besar ke agen dengan peran dan model berbeda, lalu ikuti kemajuannya di satu tempat."
    ]
  ],
  runtimeLink: "Lihat semua integrasi",
  installBandHeading: "Instal di macOS atau Linux",
  copy: "Salin",
  copied: "Tersalin ✓",
  binaries: "Biner",
  chinaMirrors: "Mirror Tiongkok",
  installGuideLink: "Baca panduan instalasi",
  communityHeading: "Jadikan Codewhale milikmu.",
  communityBody:
    "Codewhale bersumber terbuka. Baca kodenya, buat plugin, bagikan alur kerja, atau bantu memperbaiki rilis berikutnya.",
  communityLinksAria: "Tautan komunitas",
  contribute: "Berkontribusi di GitHub",
};
