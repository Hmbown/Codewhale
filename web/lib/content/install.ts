/**
 * The two one-line installs, shared by the install page and the homepage
 * hero. Code-owned shell, never translated.
 */
export const INSTALL_COMMANDS = {
  shell: "curl -fsSL https://codewhale.net/install.sh | sh",
  npm: "npm install -g codewhale",
} as const;

export const INSTALL_COPY = {
  metaTitle: { en: "Install · Codewhale", zh: "安装 · Codewhale" },
  metaDescription: { en: "Install Codewhale, connect your model, and start your first task. Guides for macOS, Linux, Windows, package managers, and source builds.", zh: "安装 Codewhale、连接模型并开始第一项任务。提供 macOS、Linux、Windows、包管理器与源码编译指南。" },
  source: { en: "Read the verified guide on GitHub", zh: "查看 GitHub 上已验证的指南" },
  translationNotice: { en: "This verified guide is in English. The Chinese translation has not yet been updated for this revision.", zh: "本页显示已验证的英文指南。中文译文尚未更新至此修订版。" },
  configuration: { en: "Settings and configuration", zh: "设置与配置" },
} as const;
