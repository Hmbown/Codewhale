import type { HomeDict } from "../types";

/**
 * Japanese home dictionary — native copy for the whale-road landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale：あらゆるモデルで使えるオープンソースのコーディングエージェント",
  metaDescription:
    "Codewhale はターミナルで使うオープンソースのコーディングエージェントです。選んだホスト型またはローカルのモデルで、プロジェクトを読み、ファイルを編集し、テストを実行します。",
  heroTitle: "あらゆるモデルで使えるオープンソースのコーディングエージェント",
  heroIntro:
    "{brand} はターミナルからプロジェクトを読み、ファイルを編集し、テストを実行します。ホスト型またはローカルのモデルを接続し、どの操作に承認が必要かを選べます。",
  getCodewhale: "Codewhale をインストール",
  heroInstallAria: "インストールコマンド",
  exploreProduct: "仕組みを見る",
  shotPreview: "ターミナルのプレビュー",
  shotBuild: "v{version} 開発ビルド",
  screenshotAlt:
    "Codewhale v{version} 開発ビルド。クジラのマーク、新しいセッション、入力欄、Ask 権限、Work モード、モデルの状態。独立したターミナルの実際の出力を描画。",
  latestRelease: "最新リリース {tag}",
  releaseUnavailable: "リリース情報を取得できません",
  currentSource: "ソース",
  sourceCandidate: "未リリース",
  publishedRelease: "リリース済み",
  figcaptionSourceCandidate: "未リリース",
  chapterTerminal: "あなたのターミナル",
  chapterTerminalTitle: "編集とコマンドを実行中にひとつずつ確認する",
  gainHeading: "タスクを任せながら制御を保つ",
  gainLede: "バグの修正、モジュールの説明、繰り返す作業の自動化など、求める結果を伝えてください。ひとつのエージェントから始め、仕事が大きくなったらエージェントを追加できます。",
  gain: [
    [
      "コードを変更して確認する",
      "エージェントがプロジェクトを調べ、ファイルを編集し、テストを実行します。作業中の編集とコマンドの結果をひとつずつ確認できます。"
    ],
    [
      "繰り返しの作業を自動化する",
      "スクリプトや CI から codewhale exec を実行します。Fleet を使うと、大きな仕事を複数のエージェントに分担できます。"
    ],
    [
      "制御を保つ",
      "作業を始める前に権限を設定し、承認リクエストに応え、いつでもタスクを停止できます。/receipts を実行すると、セッション内のすべてのファイル、コマンド、承認が一覧表示されます。"
    ]
  ],
  chapterModels: "あなたのモデル",
  modelsHeading: "タスクごとにモデルを選ぶ",
  modelsBody:
    "セッションごとに、組み込みのプロバイダー、任意の OpenAI 互換エンドポイント、またはローカルモデルを選べます。モデルの接続は Codewhale のアカウントとは別に扱われます。",
  modelsFacts: [
    ["ホスト型", "自分の API キーを codewhale auth set --provider <id> で保存"],
    ["ゲートウェイ", "ひとつのエンドポイントで多くのモデルを利用。プロバイダーは引き続き自分で選ぶ"],
    ["ローカル", "localhost 上の vLLM、SGLang、Ollama。通常はキー不要"],
  ],
  modelsLink: "モデルとプロバイダーを見る",
  startHeading: "インストールし、モデルを接続し、タスクを実行する",
  startLede: "プロジェクトフォルダーから 3 つの手順で最初のタスクを実行できます。複数のエージェントが必要な作業になったら、あとから Fleet を追加できます。",
  startGuideLink: "はじめかたガイドに沿って進める",
  startVocabularyLink: "製品用語を見る",
  chapterAvailability: "動作環境",
  availabilityHeading: "今すぐターミナルで使う",
  availabilityLede: "ターミナル、ローカルブラウザークライアント、コミュニティの CodeWhale GUI は今すぐ使えます。デスクトップアプリと、作り直しているホスト型 Web アプリは開発中で、同じセッションモデルを共有します。",
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
  availabilityNote: "ターミナル、ローカルブラウザー、GUI は Codewhale のアカウントなしで使えます。ホスト型 Web とデスクトップはアカウントを使いますが、アカウントはモデル接続の代わりにはなりません。自分のキーでの利用料金はプロバイダーから請求されます。",
  accountLink: "アカウントを作成",
  surfacesHeading: "エージェントが扱える範囲を広げる",
  surfaces: [
    ["ファイルとコマンド", "設定した権限の範囲で、プロジェクトを読み、ファイルを編集し、テストを実行し、出力を確認します。"],
    ["プラグインと MCP", "ツールやサービスを追加で接続します。各プラグインは、内容を確認して有効にするまでオフのままです。"],
    ["Computer Use · プレビュー", "エージェントがほかのアプリを見て操作できるようにするプラグインです。自分で有効にし、求められるシステム権限を付与します。"],
    ["保存済みセッション", "会話とツールの結果をまとめて保存し、最初からやり直さずに再開できます。ローカルブラウザーは、あなたのコンピューター上の同じセッションを開きます。"],
    ["Fleet", "異なるモデルと役割を持つエージェントにタスクの各部分を割り当て、それぞれの進捗を確認します。"],
  ],
  runtimeLink: "すべての連携機能を見る",
  installBandHeading: "macOS または Linux にインストールする",
  copy: "コピー",
  copied: "コピー済み ✓",
  binaries: "バイナリ",
  chinaMirrors: "中国ミラー",
  installGuideLink: "インストールガイドを読む",
  communityHeading: "Codewhale を一緒に作る",
  communityBody: "GitHub でバグを報告したり、機能を提案したり、初めてのプルリクエストを送ったりしてください。小さく、テスト済みの修正を歓迎します。",
  communityLinksAria: "コミュニティリンク",
  contribute: "プルリクエストを送る",
};
