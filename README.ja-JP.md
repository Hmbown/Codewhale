<!-- source: README.md sha256:bbb8bfb61e57 -->
# Codewhale

Codewhale は、選んだホスト型またはローカルのモデルを使ってプロジェクトを読み、ファイルを編集し、コマンドを実行して、自分の作業結果を確認するオープンソースのエージェントです。まずはターミナルで一つのタスクから始めましょう。大きな仕事では、異なるモデルや役割を持つエージェントに作業の一部を分担させられます。

![ターミナルで動作する Codewhale](web/public/codewhale-tui-d7a9a1c.png)

*v0.10.0 の開発ビルドによるターミナルのプレビュー。*

[English](README.md) · [简体中文](README.zh-CN.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## インストール

macOS または Linux に新規インストールする場合は、公式 GitHub Release を使います。

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

インストーラーは、公開済みの最新リリースを選択します。[変更履歴](CHANGELOG.md)には次のリリースの未公開候補版についても記載されていますが、その変更が公開ダウンロードに含まれるのは、リリースが公開されてからです。

Windows では [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest) から対応するインストーラーまたはアーカイブを入手してください。既存の直接インストールは `codewhale update` で更新できます。確認だけなら `codewhale update --check` を使います。更新対象の実行ファイルのパスが表示され、より新しいビルドは保持されます。npm と Cargo は補助的なパッケージ導入方法です。パッケージ管理からの移行や PATH の設定は[インストールガイド](docs/INSTALL.md)を参照してください。

初回起動ではそのまま入力欄（コンポーザー）が開き、セットアップの案内はありません。モデルからの応答には、ホスト型またはローカルのモデルへの接続が必要です。接続されるまで、起動画面には "no model connected" と表示されます。`/provider` を実行する（または F3 を押す）と、ホスト型サービスのキーを追加したり、ローカルランタイムを選んだりできます。Ollama がチャットモデルとともにすでに動作している場合、Codewhale は自動的にそれに切り替わります。Codewhale は補助的なパッケージ配布方法として npm と Cargo に対応し、Docker、Nix、Scoop、Android/Termux、必要に応じて利用できる CNB ミラーにも対応しています。パッケージマネージャーでインストール済みの場合は、移行手順が案内されます。[インストールと PATH のヘルプ](docs/INSTALL.md)を参照してください。

各シェルの Tab 補完はコマンド一つで設定できます — `codewhale completion bash|zsh|fish|powershell|elvish`。詳しくは[シェル補完](docs/INSTALL.md#8-shell-completions)をご覧ください。

## 使い方

プロジェクトのフォルダーでターミナルを開き、`codewhale` を実行します。`/provider` でプロバイダーを、`/model` でモデルを選び、具体的なタスクを伝えます：

```text
Fix the failing tests and explain what changed.
```

TUI を開かずにタスクを実行することもできます：

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale はリポジトリを読み、ファイルを編集し、コマンドを実行して結果を確認しながら、目標に向かって作業を続けます。ファイルの変更やシェルコマンドの実行をせずに調べるには `/mode plan` を使い、変更を加えてほしいときは `/mode work` を使います。`Shift+Tab` を押すと Ask、Auto-Review、Full Access を選択できます。それぞれで許可される操作は[モードと権限のガイド](docs/MODES.md)を参照してください。

## ターミナル、アプリ、Computer Use

ターミナルとグラフィカルなクライアントは Codewhale Runtime に接続します。Runtime がエージェントとそのツールを実行します：

- **ターミナル：** `codewhale` は対話型インターフェースを開き、`codewhale exec` はスクリプトや CI ジョブからタスクを実行します。
- **ローカルブラウザー：** `codewhale web` は、同じ Runtime を使う同梱の[ローカル Web クライアント](docs/WEB.md)を開きます。
- **Codewhale デスクトップアプリ（GPUI）：** 別リポジトリで開発しているネイティブデスクトップアプリが、サインイン後に使う製品クライアントの方向性です。app.codewhale.net のホステッド Web アプリはこれに合わせて作り直します。マーケティングサイト・サインイン・課金・法務・ダウンロードの各ページは Web に残ります。提供状況は[製品ページ](https://codewhale.net/en/product)をご覧ください。

**Computer Use は、ほかのアプリケーションの状態を確認し、操作するためのツールを追加します。** このプラグインは現在のソースコードに含まれています。使用前に要求されるアクセス権を確認し、有効にしてください。OS の権限やプラットフォームの要件も満たす必要があります。同梱の [Computer Use ガイド](crates/tui/plugins/computer-use/README.md)と[プラグインの設定](docs/PLUGINS.md)を参照してください。

VS Code では、コミュニティが保守する CodeWhale 拡張機能がサイドバーからローカルの Runtime に接続します。[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode) からインストールしてください。ソースコードは [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode) にあります。

## Codewhale を選ぶ理由

- **モデルを選べます。** ホスト型プロバイダーに接続するほか、Ollama、vLLM、SGLang 経由でローカルモデルも利用できます。`/provider` でプロバイダーを切り替え、`/model` でモデルを選択します。
- **主導権を保てます。** 提案された操作と、その結果生じたファイルの変更を確認できます。承認設定によってレビューが必要なタイミングが決まり、Full Access でもポリシーの厳格な制約は守られます。`/undo` と `/restore` はワークスペースの変更を復元する際に役立ちます。
- **長い作業も整理できます。** セッションを保存し、永続的な `/goal` を設定し、ワークフローを実行前に確認できます。さらに、エージェントの内部指示を会話履歴に混ぜることなく、複数のエージェントを連携させられます。
- **今あるエージェントを拡張できます。** MCP サーバーやスキルを接続し、フックを設定し、エージェントの役割をプロジェクトまたは個人設定内の読みやすいファイルとして管理できます。

コマンドとキーボードショートカットは、TUI で `/help` を実行して確認できます。

## 安全性

Codewhale は、あなたが許可した範囲のアクセス権で、あなたのマシン上で動作します。承認モードとリポジトリのルールがエージェントの操作を制限し、対応環境では任意の OS サンドボックスがさらに強固な実行境界を加えます。不明なモデル料金は、無料と表示せず不明のまま扱います。

正確なポリシーの適用順序は[認可の順序](docs/AUTHORIZATION_ORDER.md)、ローカル設定は[設定ガイド](docs/CONFIGURATION.md)をご覧ください。

## ドキュメント

- [プロバイダーとローカルモデル](docs/PROVIDERS.md)
- [エージェントチーム](docs/FLEET.md)
- [MCP](docs/MCP.md)、[フック](docs/HOOKS.md)、[設定](docs/CONFIGURATION.md)
- [ローカル Web クライアント](docs/WEB.md)
- [すべてのドキュメント](docs/README.md)
- [リポジトリ構成とコントリビューションガイド](CONTRIBUTING.md#project-structure)

## コミュニティに参加

**不具合の報告、機能の提案、pull request を歓迎します。** Codewhale を何か月も使っている方も、初めて試す方もお気軽にご参加ください。必要なプロバイダーがない、ワークフローが使いづらい、ターミナル UI が作業を妨げるといった場合は、[issue を作成](https://github.com/Hmbown/CodeWhale/issues/new/choose)するか、[pull request を送信](CONTRIBUTING.md)して、一緒に改善しましょう。初めてのコントリビューションも歓迎し、採用された成果にはコントリビューターのクレジットを残します。

[Discord](https://discord.gg/37gfS3ksug) に参加するか、WeChat で Hunter（`hunterbown`）を追加して Whale Brothers グループへの参加を依頼してください。

## プロジェクトの沿革

Codewhale は `deepseek-tui` として始まり、その設定とセッションとの互換性を現在も維持しています。今ではプロバイダーに依存せず、独立して保守されており、いかなるモデルプロバイダーとも提携していません。

すべてのコントリビューターと、プロジェクトの成長を支えたオープンソースコミュニティに感謝します。[コントリビューターの記録](docs/CONTRIBUTORS.md)もご覧ください。

## ライセンス

[MIT](LICENSE)。他のオープンソースプロジェクトを基にした部分は[サードパーティー通知](docs/THIRD_PARTY_NOTICES.md)に記載しています。
