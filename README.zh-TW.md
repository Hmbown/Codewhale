<!-- source: README.md sha256:bbb8bfb61e57 -->
# Codewhale

Codewhale 是一款開源代理，可使用你選擇的託管模型或本機模型讀取專案、編輯檔案、執行指令，並檢查自己的工作。從終端機中的一項任務開始。對於較大的工作，可以將部分任務交給使用不同模型、擔任不同角色的代理。

![Codewhale 在終端機中執行](web/public/codewhale-tui-d7a9a1c.png)

*終端機預覽截圖來自 v0.10.0 的開發建置版本。*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [한국어](README.ko-KR.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## 安裝

在 macOS 或 Linux 上首次安裝時，請使用官方 GitHub Release：

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

安裝程式會選擇最新的已發布版本。[更新日誌](CHANGELOG.md)也描述了下一版本尚未發布的候選建置；只有在該版本正式發布後，已發布的下載檔才會包含這些變更。

Windows 請從 [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest) 下載對應的安裝程式或封存檔。已有的直接安裝使用 `codewhale update`；若只想檢查，使用 `codewhale update --check`。更新器會顯示執行檔路徑，並保留較新的建置版本。npm 和 Cargo 是次要套件安裝方式；套件管理器安裝的遷移與 PATH 設定請參閱[安裝指南](docs/INSTALL.md)。

第一次執行會直接開啟輸入框，不會引導你完成設定流程。要取得模型回覆，必須連線至託管模型或本機模型：在連線之前，啟動畫面會顯示 "no model connected"。執行 `/provider`（或按 F3）即可新增託管服務金鑰或選擇本機執行環境。如果 Ollama 已在執行且帶有聊天模型，Codewhale 會自動切換到它。Codewhale 也支援 npm 和 Cargo 作為次要套件安裝方式，以及 Docker、Nix、Scoop、Android/Termux 與選用的 CNB 鏡像。對於既有的套件管理器安裝，系統會提供遷移說明。請參閱[安裝與 PATH 說明](docs/INSTALL.md)。

每種 shell 只需一個指令即可啟用 Tab 自動完成——`codewhale completion bash|zsh|fish|powershell|elvish`。請參閱 [shell 自動完成](docs/INSTALL.md#8-shell-completions)。

## 使用

在專案資料夾中開啟終端機並執行 `codewhale`。使用 `/provider` 選擇供應商，使用 `/model` 選擇模型，接著描述一項具體任務：

```text
Fix the failing tests and explain what changed.
```

你也可以不開啟 TUI，直接執行任務：

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale 可以讀取你的程式碼儲存庫、編輯檔案、執行指令、檢查結果，並持續朝目標推進。使用 `/mode plan` 可以在不修改檔案、不執行 shell 指令的情況下探索；希望它進行修改時，使用 `/mode work`。按 `Shift+Tab` 可選擇 Ask、Auto-Review 或 Full Access；[模式與權限指南](docs/MODES.md)說明了各自允許的操作。

## 終端機、應用程式與 Computer Use

終端機和圖形用戶端連線至 Codewhale Runtime，由它執行代理及其工具：

- **終端機：** `codewhale` 開啟互動介面；`codewhale exec` 可從指令碼或 CI 工作中執行任務。
- **本機瀏覽器：** `codewhale web` 開啟隨附的[本機網頁用戶端](docs/WEB.md)，使用同一個 Runtime。
- **Codewhale 桌面應用程式（GPUI）：** 在獨立儲存庫中開發的原生桌面應用程式是登入後產品客戶端的方向。app.codewhale.net 的託管網頁應用程式將依它的樣子重建；行銷網站、登入、計費、法律與下載頁面保留在網頁上。其可用情況見[產品頁面](https://codewhale.net/en/product)。

**Computer Use 提供觀察其他應用程式並與之互動的工具。** 目前的原始碼已包含此外掛程式。使用前請檢視它要求的存取權限並啟用它；仍須符合作業系統權限與平台要求。請參閱隨附的 [Computer Use 指南](crates/tui/plugins/computer-use/README.md)與[外掛程式設定](docs/PLUGINS.md)。

在 VS Code 中，社群維護的 CodeWhale 擴充功能透過側邊欄連線至本機 Runtime。可從 [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode) 安裝；原始碼見 [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode)。

## 為何選擇 Codewhale

- **選擇你的模型。** 連線至託管供應商，或透過 Ollama、vLLM、SGLang 使用本機模型。使用 `/provider` 切換供應商，使用 `/model` 選擇模型。
- **掌控權始終在你手中。** 檢查擬執行的操作及其造成的檔案變更。核准設定決定何時需要審查；Full Access 仍須遵守不可逾越的政策邊界。`/undo` 和 `/restore` 可協助復原工作區變更。
- **讓長時間工作井然有序。** 儲存工作階段、設定持久的 `/goal`、在工作流程執行前加以審查，並協調多個代理，同時避免其內部指示混入你的對話記錄。
- **擴充你已有的代理。** 連接 MCP 伺服器與技能、設定掛鉤，並將代理角色以可讀檔案保存在專案或個人設定中。

在 TUI 中執行 `/help` 可查看指令與鍵盤快速鍵。

## 安全性

Codewhale 在你的電腦上執行，且只擁有你授予的存取權限。核准模式與儲存庫規則會限制代理可以執行的操作；在支援的平台上，選用的作業系統沙箱可提供更強的執行邊界。未知的模型價格會維持顯示為未知，而不會被誤報為免費。

閱讀[授權順序](docs/AUTHORIZATION_ORDER.md)以了解確切的政策層級，並閱讀[設定](docs/CONFIGURATION.md)以了解本機設定。

## 文件

- [供應商與本機模型](docs/PROVIDERS.md)
- [代理團隊](docs/FLEET.md)
- [MCP](docs/MCP.md)、[掛鉤](docs/HOOKS.md)與[設定](docs/CONFIGURATION.md)
- [本機網頁用戶端](docs/WEB.md)
- [所有文件](docs/README.md)
- [儲存庫結構與貢獻指南](CONTRIBUTING.md#project-structure)

## 加入社群

**歡迎回報錯誤、提出功能建議及提交 pull request**，無論你已使用 Codewhale 數月，還是第一次嘗試。如果缺少某個供應商、工作流程操作不便，或終端機介面妨礙了你，請[提出 issue](https://github.com/Hmbown/CodeWhale/issues/new/choose) 或[提交 pull request](CONTRIBUTING.md)，一起改善。我們歡迎首次貢獻，貢獻者也會保留已合併工作的署名。

加入 [Discord](https://discord.gg/37gfS3ksug)，或在微信加入 Hunter（`hunterbown`）並申請加入 Whale Brothers 群組。

## 專案歷史

Codewhale 最初名為 `deepseek-tui`，至今仍保留與其設定及工作階段的相容性。現在它不偏向任何供應商，由社群獨立維護，也不隸屬於任何模型供應商。

感謝每一位貢獻者，以及協助專案成長的開源社群。請參閱[貢獻者記錄](docs/CONTRIBUTORS.md)。

## 授權條款

[MIT](LICENSE)。從其他開放原始碼專案改編的部分記錄於[第三方聲明](docs/THIRD_PARTY_NOTICES.md)。
