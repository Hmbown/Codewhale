import type { ComputerUseDict } from "../types";

/**
 * Japanese dictionary for `app/[locale]/computer-use/page.tsx` and the
 * Computer Use section of the install page. Product names, the menu items
 * Pause, Stop and Check for updates, and the macOS setting names stay as the
 * app shows them; the macOS setting and folder names use the labels of the
 * Japanese system UI (アクセシビリティ / 画面収録 / アプリケーション).
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Mac 版 Computer Use · Codewhale",
  metaDescription: "Mac 版 Codewhale Computer Use をダウンロードして設定します。アプリのバックグラウンド操作、権限の設定、そして人が操作する Pause と Stop のコントロールを備えています。",
  title: "Computer Use",
  lead: "あなたが作業を続けている間に、Codewhale がアプリの中で仕事を進めます。Mac 用ヘルパーは、権限の設定、アプリのバックグラウンド操作、入力の一時停止や停止をメニューバーにまとめます。",
  publisher: "Codewhale 提供",
  download: "Mac 版をダウンロード",
  downloadZip: "ZIP アーカイブ（アプリ内アップデーターが使用）",
  requirements: "macOS 13.5 以降 · Apple silicon と Intel に対応",
  included: "ダウンロードするのはアプリひとつだけ。Node やコンパイラーを別途インストールする必要はありません。",
  pendingTitle: "Mac 版ダウンロードを準備中",
  pendingBody: "Apple の公証とリリース前チェックが完了し次第、公開インストーラーをここで提供します。",
  unavailableTitle: "ダウンロードの提供状況を確認できませんでした",
  unavailableBody: "ページを再読み込みして再試行するか、下の公開済みリリースをご確認ください。",
  releases: "公開済みリリース",
  receipt: "ダウンロードの検証情報",
  setup: "Mac をセットアップする",
  steps: [
    { title: "アプリをインストールする", body: "ディスクイメージを開き、Codewhale Computer Use を「アプリケーション」フォルダにドラッグします。「アプリケーション」から起動し、メニューバーのクジラのアイコンから Computer Use を選択してください。" },
    { title: "権限を確認する", body: "セットアップのボタンから、システム設定の「アクセシビリティ」と「画面収録」を開きます。どの権限を許可するかはあなたが決めます。" },
    { title: "バックグラウンドチェックを実行する", body: "ヘルパーが使い捨ての練習用ウインドウを開き、テキストを入力して、そのウインドウをキャプチャします。実行中にポインタや前面のアプリが変わらなかったかを確認します。" },
    { title: "Codewhale に接続する", body: "Codewhale のプラグインマーケットプレイスで Computer Use を確認し、信頼して有効にします。ローカル操作がヘルパーの Pause と Stop のコントロールを通るように、プラグインは 0.3.1 以降を使用してください。" },
  ],
  controlsTitle: "作業を続けながら、主導権はあなたに。",
  controlsBody: "対応している操作は、選択したアプリのバックグラウンドで実行されます。前面での操作が必要なアプリやジェスチャには、あなたの承認が必要です。メニューには対象アプリと入力モードが表示され、Pause（一時停止）はヘルパーの入力を保留し、Stop（停止）は進行中のセッションを終了します。",
  updateTitle: "アップデートはあなたのタイミングで",
  updateBody: "アプリから Check for updates（アップデートを確認）を選択します。インストール前にダウンロードしたファイル、Codewhale の署名、Apple の公証を検証し、復旧用に以前のアプリを保持します。",
  help: "セットアップとトラブルシューティング",
  notes: "リリースノート",
  demo: "バックグラウンドチェックを見る",
  source: "ソースコードとその他のプラットフォーム",
  platforms: "このダウンロードは Mac 用です。Windows と Linux では現在、ソースプラグインとホスト側のセットアップを使用します。",
};
