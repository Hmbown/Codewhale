import type { HomeDict } from "../types";

/** Japanese home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale：自分のモデルとツールで、作りたいものを形に",
  metaDescription:
    "Codewhale でアプリを作り、仕事を自動化し、接続したツールを活用。オープンソースで、手持ちのモデル API やローカル・自己ホスト型の推論に対応。",
  heroTitle: "作りたいものを、形に。",
  heroIntro:
    "アプリを作る。作業を自動化する。集めた調査資料を役立つ成果にまとめる。{brand} は、すでに使っているモデル API、自分の推論環境、接続したツールで動きます。",
  getCodewhale: "Codewhale をインストール",
  heroInstallAria: "インストールコマンド",
  exploreProduct: "Codewhale を見る",
  shotPreview: "ターミナルのプレビュー",
  screenshotAlt:
    "Codewhale v{version} のターミナル収録画面。会話、メッセージ入力欄、セッション操作を表示。",
  latestRelease: "最新リリース {tag}",
  releaseUnavailable: "リリース情報を取得できません",
  currentSource: "ソース",
  sourceCandidate: "未リリース",
  publishedRelease: "リリース済み",
  gainHeading: "役に立つものを作ろう。",
  gainLede: "欲しい結果から始めましょう。Codewhale がファイル、コマンド、接続したツールで作業し、アクセス範囲と承認方法はあなたが決めます。",
  gain: [
    [
      "アプリやツールを作る",
      "アイデアを動くアプリや便利なスクリプト、既存プロジェクトの新機能に。エージェントと一緒に書き、実行し、テストできます。"
    ],
    [
      "繰り返す仕事を自動化",
      "定期的な作業を、ターミナルやスクリプト、CI から実行するワークフローに。並行して進められる仕事には Fleet のチームを使えます。"
    ],
    [
      "いつものツールをつなぐ",
      "プラグイン、MCP サーバー、API を通じて Gmail や Slack などのツールを接続できます。ファイルやコマンドと同じタスクで活用できます。"
    ]
  ],
  exampleTasks: [
    "予約を受け付けるアプリを作って。",
    "売上CSVから、毎週繰り返し作れるレポートを作って。",
    "接続済みのメールから、やることリストを作って。",
  ],
  // A static example report built from local sample orders.
  reportTitle: "週次売上レポート",
  reportSampleLabel: "レポートの例 · サンプルデータ",
  reportDescription: "Codewhale に注文を週ごとに集計させ、次の CSV にも使えるように手順を保存しましょう。",
  reportSourceLabel: "入力データ：",
  reportColumns: ["週の開始日","注文数","売上（米ドル）"],
  reportTotalLabel: "合計",
  reportTrend: "最初の週から最後の週までの売上の変化：{change}。",
  reportDownloadLabel: "レポート CSV をダウンロード",
  chapterModels: "あなたのモデル",
  modelsHeading: "使うモデルは、自分で選ぶ。",
  modelsBody:
    "契約済みのモデル API、互換ゲートウェイ、自分のハードウェアでの推論を使えます。セッションごとにも、Fleet 内のエージェントごとにもモデルを選べます。",
  modelsFacts: [
    [
      "自分の API アカウント",
      "自分のキーで OpenAI、Anthropic、Google、DeepSeek などに接続。"
    ],
    [
      "自分のゲートウェイ",
      "OpenAI 互換エンドポイントに接続し、提供されるモデルを選択。"
    ],
    [
      "自分の推論環境",
      "Ollama、vLLM、SGLang でローカルまたは自己ホスト型モデルを実行。"
    ]
  ],
  modelsLink: "モデルとプロバイダーを見る",
  startHeading: "やりたい仕事から、始めよう。",
  startLede: "Codewhale をインストールし、モデルを接続して、役立つ仕事を任せましょう。まずは一つのエージェントから。必要に応じてツールやチームを加えられます。",
  startGuideLink: "はじめかたガイドに沿って進める",
  startVocabularyLink: "製品用語を見る",
  chapterAvailability: "動作環境",
  availabilityHeading: "ターミナルから始める。",
  availabilityLede: "ターミナルとローカルブラウザクライアントは利用可能です。ネイティブのデスクトップアプリと再構築中のホスト型ウェブアプリは開発中です。",
  availability: [
    [
      "ターミナルとローカルブラウザー",
      "リリース済み",
      "Linux、macOS、Windows にインストールし、codewhale を実行します。ローカルブラウザークライアントを使うには codewhale web を実行します。npm と Cargo でもインストールできます。Android の Termux 版はプレビューです。"
    ],
    [
      "CodeWhale GUI（VS Code）",
      "利用可能",
      "コミュニティが保守する独立したプロジェクトです。同じ Codewhale Runtime 上で、VS Code のサイドバーからチャット、スレッド、ファイル変更を扱えます。VS Code Marketplace からインストールできます。",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "ホスト型 Web アプリ",
      "開発プレビュー",
      "デスクトップアプリに合わせて作り直しています。現在はサインインしたうえで、実行中のターミナルセッションで /rc と入力すると、そのセッションを Web で続けられます。ホスト型のタスク実行は引き続き検証中です。"
    ],
    [
      "デスクトップ",
      "開発ビルド",
      "Codewhale の主要なクライアントになりつつあるネイティブアプリです。フォルダー、会話、モデル接続をひとつのウィンドウにまとめます。一般向けのダウンロードはまだありません。"
    ],
    [
      "クラウドコンピューター",
      "開発中",
      "タスクを実行するホスト型コンピューター。"
    ]
  ],
  availabilityNote: "ターミナル、ローカルブラウザー、GUI は Codewhale のアカウントなしで使えます。ホスト型 Web とデスクトップはアカウントを使います。自分のプロバイダーキーを使う場合、その利用料金はプロバイダーから請求されます。",
  accountLink: "アカウントを作成",
  surfacesHeading: "一つのタスクに、ファイルもアプリもエージェントも。",
  surfaces: [
    [
      "ファイルとターミナル",
      "ファイル作成、コマンド実行、データ分析、成果物のテストまで。作業フォルダと権限は自分で設定します。"
    ],
    [
      "プラグインと接続アプリ",
      "プラグインと MCP でスキルやツールを追加。エージェントに使わせたい接続を確認して有効にします。"
    ],
    [
      "ブラウザとコンピューター操作 · プレビュー",
      "許可したアクセス範囲で、ブラウザツールと Computer Use プラグインを使ってアプリやサイトを操作できます。"
    ],
    [
      "続きから始められるセッション",
      "会話、ツールの結果、作業履歴をまとめて保存。ターミナルやローカルブラウザからタスクを再開できます。"
    ],
    [
      "エージェントのチーム",
      "Fleet で大きな仕事を、役割やモデルの異なるエージェントに分担。進捗を一か所で確認できます。"
    ]
  ],
  runtimeLink: "すべての連携機能を見る",
  installBandHeading: "macOS または Linux にインストールする",
  copy: "コピー",
  copied: "コピー済み ✓",
  binaries: "バイナリ",
  chinaMirrors: "中国ミラー",
  installGuideLink: "インストールガイドを読む",
  communityHeading: "Codewhale を、自分の道具に。",
  communityBody: "Codewhale はオープンソースです。コードを読む、プラグインを作る、ワークフローを共有する。次のリリースを一緒に良くしましょう。",
  communityLinksAria: "コミュニティリンク",
  contribute: "GitHub で参加",
};
