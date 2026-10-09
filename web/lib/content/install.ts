/**
 * Installation commands shared by the install page and homepage hero.
 * Code-owned shell, never translated.
 */
export const INSTALL_COMMANDS = {
  shell: "curl -fsSL https://codewhale.net/install.sh | sh",
  npm: "npm install -g codewhale",
  windows: "winget install --id HunterBown.CodeWhale --exact --source winget",
  cargo: "cargo install codewhale-cli --locked",
} as const;

export const INSTALL_COPY = {
  windowsNote: { en: "Windows x64. Winget updates can lag behind GitHub releases; npm is another option.", zh: "适用于 Windows x64。Winget 更新可能晚于 GitHub 发布；也可以通过 npm 安装。" },
  cargoNote: { en: "Builds from source. Requires current stable Rust and build tools. The Cargo package can lag behind GitHub releases.", zh: "从源码构建，需要当前稳定版 Rust 和构建工具。Cargo 包可能晚于 GitHub 发布。" },
  metaTitle: { en: "Install · Codewhale", zh: "安装 · Codewhale" },
  metaDescription: { en: "Install Codewhale on macOS, Linux, or Windows, connect a model, and run your first task. Covers package managers and source builds.", zh: "在 macOS、Linux 或 Windows 上安装 Codewhale，连接模型并运行第一项任务。也介绍包管理器和源码构建。" },
  source: { en: "Read the verified guide on GitHub", zh: "查看 GitHub 上已验证的指南" },
  translationNotice: { en: "Installation receipts retain the version actually tested; latest resolves to the published release.", zh: "本页采用经审阅的简体中文指南。安装回执保留实际测试版本；latest 以已发布版本为准。" },
  configuration: { en: "Settings and configuration", zh: "设置与配置" },
} as const;
