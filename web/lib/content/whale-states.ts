/**
 * The words under the homepage whale. Every state reads without motion or
 * colour: a mark and a word (codewhale-design DIRECTION.md, Craft). The
 * performance is an illustration of a session's phases, not live telemetry.
 *
 * These are short status phrases shown in every routed locale, so they carry
 * all eighteen languages here instead of the en/zh pair other content files
 * use; a locale missing from a row falls back to English.
 */
type StateText = { en: string } & Partial<Record<string, string>>;

export const WHALE_STATE_TEXT = {
  rest: {
    en: "Ready", zh: "就绪", ja: "準備完了", ko: "준비됨", vi: "Sẵn sàng", id: "Siap",
    es: "Listo", "pt-BR": "Pronto", ru: "Готово к работе", uk: "Готово до роботи", fr: "Prêt",
    de: "Bereit", hi: "तैयार", tr: "Hazır", it: "Pronto", pl: "Gotowy", ar: "جاهز", ca: "A punt",
  },
  listen: {
    en: "Reading your request", zh: "正在阅读你的请求", ja: "依頼を読んでいます", ko: "요청을 읽는 중",
    vi: "Đang đọc yêu cầu", id: "Membaca permintaan Anda", es: "Leyendo tu solicitud",
    "pt-BR": "Lendo seu pedido", ru: "Читает ваш запрос", uk: "Читає ваш запит",
    fr: "Lecture de votre demande", de: "Liest deine Anfrage", hi: "आपका अनुरोध पढ़ रहा है",
    tr: "İsteğini okuyor", it: "Legge la tua richiesta", pl: "Czyta Twoje polecenie",
    ar: "يقرأ طلبك", ca: "Llegint la teva petició",
  },
  think: {
    en: "Planning", zh: "正在规划", ja: "計画中", ko: "계획하는 중", vi: "Đang lập kế hoạch",
    id: "Merencanakan", es: "Planificando", "pt-BR": "Planejando", ru: "Планирует", uk: "Планує",
    fr: "Planification", de: "Plant", hi: "योजना बना रहा है", tr: "Planlıyor", it: "Pianifica",
    pl: "Planuje", ar: "يخطط", ca: "Planificant",
  },
  read: {
    en: "Reading files", zh: "正在读取文件", ja: "ファイルを読んでいます", ko: "파일을 읽는 중",
    vi: "Đang đọc tệp", id: "Membaca berkas", es: "Leyendo archivos", "pt-BR": "Lendo arquivos",
    ru: "Читает файлы", uk: "Читає файли", fr: "Lecture des fichiers", de: "Liest Dateien",
    hi: "फ़ाइलें पढ़ रहा है", tr: "Dosyaları okuyor", it: "Legge i file", pl: "Czyta pliki",
    ar: "يقرأ الملفات", ca: "Llegint fitxers",
  },
  write: {
    en: "Editing", zh: "正在编辑", ja: "編集中", ko: "편집하는 중", vi: "Đang chỉnh sửa",
    id: "Mengedit", es: "Editando", "pt-BR": "Editando", ru: "Редактирует", uk: "Редагує",
    fr: "Modification", de: "Bearbeitet", hi: "संपादित कर रहा है", tr: "Düzenliyor",
    it: "Modifica", pl: "Edytuje", ar: "يعدّل", ca: "Editant",
  },
  run: {
    en: "Running tests", zh: "正在运行测试", ja: "テストを実行中", ko: "테스트 실행 중",
    vi: "Đang chạy kiểm thử", id: "Menjalankan pengujian", es: "Ejecutando pruebas",
    "pt-BR": "Executando testes", ru: "Запускает тесты", uk: "Запускає тести",
    fr: "Exécution des tests", de: "Führt Tests aus", hi: "टेस्ट चला रहा है",
    tr: "Testleri çalıştırıyor", it: "Esegue i test", pl: "Uruchamia testy",
    ar: "يشغّل الاختبارات", ca: "Executant proves",
  },
  done: {
    en: "Done", zh: "已完成", ja: "完了", ko: "완료", vi: "Hoàn tất", id: "Selesai", es: "Listo",
    "pt-BR": "Concluído", ru: "Готово", uk: "Готово", fr: "Terminé", de: "Fertig", hi: "पूरा हुआ",
    tr: "Tamamlandı", it: "Fatto", pl: "Gotowe", ar: "تم", ca: "Fet",
  },
  pod: {
    en: "Working with three agents", zh: "正在与三个智能体协作", ja: "3 つのエージェントと作業中",
    ko: "에이전트 세 개와 작업 중", vi: "Đang làm việc cùng ba tác tử", id: "Bekerja dengan tiga agen",
    es: "Trabajando con tres agentes", "pt-BR": "Trabalhando com três agentes",
    ru: "Работает с тремя агентами", uk: "Працює з трьома агентами", fr: "Travaille avec trois agents",
    de: "Arbeitet mit drei Agenten", hi: "तीन एजेंटों के साथ काम कर रहा है",
    tr: "Üç ajanla çalışıyor", it: "Lavora con tre agenti", pl: "Pracuje z trzema agentami",
    ar: "يعمل مع ثلاثة وكلاء", ca: "Treballant amb tres agents",
  },
  connect: {
    en: "Connecting a provider", zh: "正在连接提供商", ja: "プロバイダーに接続中",
    ko: "제공업체 연결 중", vi: "Đang kết nối nhà cung cấp", id: "Menghubungkan penyedia",
    es: "Conectando un proveedor", "pt-BR": "Conectando um provedor", ru: "Подключает провайдера",
    uk: "Підключає провайдера", fr: "Connexion d’un fournisseur", de: "Verbindet einen Anbieter",
    hi: "प्रदाता से जोड़ रहा है", tr: "Sağlayıcıya bağlanıyor", it: "Collega un provider",
    pl: "Łączy dostawcę", ar: "يتصل بمزوّد", ca: "Connectant un proveïdor",
  },
  busy: {
    en: "Working", zh: "正在工作", ja: "作業中", ko: "작업 중", vi: "Đang làm việc", id: "Bekerja",
    es: "Trabajando", "pt-BR": "Trabalhando", ru: "Работает", uk: "Працює", fr: "Au travail",
    de: "Arbeitet", hi: "काम कर रहा है", tr: "Çalışıyor", it: "Al lavoro", pl: "Pracuje",
    ar: "يعمل", ca: "Treballant",
  },
} satisfies Record<string, StateText>;

export type WhaleStateKey = keyof typeof WHALE_STATE_TEXT;

export function whaleStateLabels(locale: string): Record<WhaleStateKey, string> {
  return Object.fromEntries(
    Object.entries(WHALE_STATE_TEXT).map(([key, text]) => [key, (text as StateText)[locale] ?? text.en]),
  ) as Record<WhaleStateKey, string>;
}
