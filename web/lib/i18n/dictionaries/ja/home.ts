import type { HomeDict } from "../types";

/** Japanese home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale：自分のモデルとツールで、作りたいものを形に",
  metaDescription:
    "Codewhale でアプリを作り、仕事を自動化し、接続したツールを活用。オープンソースで、手持ちのモデル API やローカル・自己ホスト型の推論に対応。",
  heroTitle: "アプリを作り、仕事を自動化しましょう。",
  heroIntro:
    "{brand} は、コードを書き、コマンドを実行し、接続したツールを使って作業するオープンソースのエージェントです。既存のモデル API を使うことも、ローカルや自分のサーバーでモデルを動かすこともできます。",
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
  gainHeading: "できること",
  gainLede: "作りたいものや自動化したい作業を伝えてください。Codewhale は、あなたが指定したアクセス権限の範囲で、ファイルを編集し、コマンドを実行して結果を確認できます。",
  gain: [
    [
      "アプリやツールを作る",
      "アプリを作る、機能を追加する、スクリプトを書くといった作業に使えます。Codewhale は、プロジェクトのファイルを扱い、コードを実行して、作成したものをテストできます。"
    ],
    [
      "繰り返す仕事を自動化",
      "ターミナル、スクリプト、CI からワークフローを実行できます。大きなタスクでは、異なるモデルを使うエージェントの Fleet に作業の一部を任せられます。"
    ],
    [
      "いつものツールをつなぐ",
      "プラグインや MCP サーバーからツールを追加したり、自分のスクリプトから API を利用したりできます。サービスごとに設定と認証が必要です。"
    ]
  ],
  chapterModels: "あなたのモデル",
  modelsHeading: "使いたいモデルを選ぶ",
  modelsBody:
    "プロバイダーのアカウント、OpenAI 互換のエンドポイント、ローカルやセルフホストのモデルを接続できます。セッションと Fleet の各エージェントに、それぞれモデルを選べます。",
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
  startHeading: "使い始める",
  startLede: "Codewhale をインストールし、モデルを接続して、プロジェクトフォルダーを開いてください。必要に応じて、プラグインやエージェントを追加できます。",
  startGuideLink: "はじめかたガイドに沿って進める",
  startVocabularyLink: "製品用語を見る",
  chapterAvailability: "動作環境",
  availabilityHeading: "現在利用できる機能と開発中の機能",
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
  surfacesHeading: "ファイルやツールを使って作業する",
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
  runtimeLink: "ツールと連携機能を見る",
  installBandHeading: "macOS または Linux にインストールする",
  copy: "コピー",
  copied: "コピー済み ✓",
  binaries: "バイナリ",
  chinaMirrors: "中国ミラー",
  installGuideLink: "インストールガイドを読む",
  communityHeading: "Codewhale に貢献する",
  communityBody: "GitHub でバグを報告したり、ドキュメントを改善したり、コードを提供したりできます。プラグインを作り、他のユーザーとワークフローを共有することもできます。",
  communityLinksAria: "コミュニティリンク",
  contribute: "GitHub で参加",
};
