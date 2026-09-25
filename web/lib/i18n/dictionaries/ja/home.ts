import type { HomeDict } from "../types";

/**
 * Japanese home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — 選んだモデルで開発し、作業を自動化する",
  metaDescription:
    "オープンソースのエージェントと、自由に選べるホスト型またはローカルの AI モデルを使って、ソフトウェアを開発し、ファイルを扱い、日々の作業を自動化できます。",
  heroTitle: "選んだモデルで開発し、作業を自動化する",
  heroIntro:
    "{brand} のエージェントは、ソフトウェアを開発し、ファイルを扱い、繰り返し行う作業を再利用できるワークフローにまとめられます。達成したいことを伝えて仕事に合うホスト型またはローカルのモデルを選び、必要に応じてプロバイダーを自由に切り替えながら作業を進めてください。",
  getCodewhale: "Codewhale を入手",
  heroInstallAria: "インストールコマンド",
  exploreProduct: "製品を見る",
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
  gainHeading: "Codewhale でできること",
  gainLede: "プロジェクトや質問、自動化したい作業から始めて、ひとつのエージェントと一緒に進めることも、大きな仕事を複数のエージェントに分担させることもできます。",
  gain: [
    [
      "作りたいものを形にする",
      "作りたいものを説明し、コードを読み、ファイルを編集し、コマンドを実行して結果を確認できるエージェントと一緒に取り組めます。"
    ],
    [
      "日々の作業を自動化する",
      "繰り返し行う作業のスクリプトやワークフローを作れば、必要なときにターミナルから何度でも実行できます。"
    ],
    [
      "さまざまなモデルを使う",
      "エージェントにホスト型またはローカルのモデルを使い、モデルや役割に合った仕事をそれぞれに任せられます。"
    ]
  ],
  modelsHeading: "作業に合わせて選べるモデル",
  modelsBody:
    "ホスト型のプロバイダーに直接接続することも、ゲートウェイを通じて複数のプロバイダーを利用することも、モデルをローカルで実行することもでき、作業中にセッションごとに使うモデルを選べます。",
  modelsFacts: [
    ["ホスト型", "自分の API キーを codewhale auth set --provider <id> で保存"],
    ["ゲートウェイ", "ひとつのエンドポイントで多くのモデル、プロバイダーは自分で選ぶ"],
    ["ローカル", "localhost 上の vLLM、SGLang、Ollama。通常キー不要"],
  ],
  modelsLink: "モデルとプロバイダーを見る",
  startHeading: "Codewhale を使い始めるには",
  startLede: "Codewhale をインストールしてモデルを接続したら、ターミナルで最初の作業を伝え、複数のエージェントに分担してほしくなったときに Fleet を追加できます。",
  startGuideLink: "はじめかたガイドを読む",
  startVocabularyLink: "製品用語を見る",
  availabilityHeading: "Codewhale を使える場所",
  availabilityLede: "Codewhale は今すぐターミナルで使え、Web アプリ、デスクトップアプリ、クラウドコンピューターも現在開発しています。",
  availability: [
    [
      "ターミナル",
      "リリース済み",
      "Linux、macOS、Windows 向けのリリースバイナリを GitHub で提供しています。npm と Cargo からもインストールできます。Android の Termux 版はプレビューです。"
    ],
    [
      "ウェブアプリ",
      "開発プレビュー",
      "開発プレビューでアカウントへのアクセスとブラウザのペアリングを利用できます。"
    ],
    [
      "デスクトップ",
      "開発ビルド",
      "macOS アプリは開発中です。一般向けのダウンロードは後日提供予定です。"
    ],
    [
      "クラウドコンピューター",
      "開発中",
      "タスクを実行するためのホスト型コンピューター。"
    ]
  ],
  availabilityNote: "ターミナルは Codewhale のアカウントなしで使え、ホスト型モデルの利用料金はプロバイダーから請求されます。",
  accountLink: "アカウントを作成",
  surfacesHeading: "Codewhale のさまざまな使い方",
  surfaces: [
    ["TUI", "対話型のターミナル作業"],
    ["codewhale exec", "スクリプトと CI"],
    ["ローカル Web クライアント","localhost のインターフェース。ホスト型のブラウザ作業環境は開発中"],
    ["Runtime API + MCP", "ローカル連携"],
    ["Fleet","複数のエージェントでひとつの仕事に取り組む"],
  ],
  runtimeLink: "連携機能を見る",
  installBandHeading: "macOS または Linux に Codewhale をインストールする",
  copy: "コピー",
  copied: "コピー済み ✓",
  binaries: "バイナリ",
  chinaMirrors: "中国ミラー",
  installGuideLink: "インストールガイドを読む",
  communityHeading: "Codewhale を一緒により良くする",
  communityBody: "バグの報告でも、機能のアイデアでも、初めてのプルリクエストでも、皆さんの声を聞き、これからの取り組みを一緒に進めていきたいと考えています。",
  communityLinksAria: "コミュニティリンク",
  contribute: "プルリクエストを送る",
};
