import type { DocsConfigurationDict } from "../types";

export const docsConfiguration: DocsConfigurationDict = {
  metaTitle: "配置 · Codewhale 文档",
  metaDescription: "config.toml 的查找顺序、项目级覆盖、凭据优先级和旧版路径迁移。",
  bodyClassName: "text-ink-soft leading-[1.9] tracking-wide",
  overviewTitle: "配置",
  overviewLead:
    "Codewhale 从 ~/.codewhale/config.toml 读取配置（旧版 ~/.deepseek/config.toml 仍作为回退读取）。--config 标志和 CODEWHALE_CONFIG_PATH 环境变量可以指定别的路径，两者同时设置时 --config 优先；文件加载之后再应用环境变量覆盖。",
  auditLead:
    "在 TUI 里运行 {auditCommand} 可以查看哪些文档化的键能在当前会话修改、哪些能持久化、哪些只能改文件或需要重启——改动前以它输出的“Command / reason”列为准。",
  overlayTitle: "项目级覆盖",
  overlayLead:
    "当工作区包含常规文件 <workspace>/.codewhale/config.toml 时，其中声明的安全取值会合并到全局配置之上（旧版 <workspace>/.deepseek/config.toml 在新路径缺失时仍会读取；符号链接的项目配置会被拒绝）。这让仓库可以建议模型或收紧本地安全姿态，而不动用户的全局配置。单次启动可用 --no-project-config 跳过覆盖。",
  overlayLimits:
    "覆盖层有意保持狭窄：支持 model、reasoning_effort、approval_policy 与 sandbox_mode（只能收紧）、notes_path、max_subagents（夹紧到 1..=128）、allow_shell（false 生效，true 被忽略）。凭据、端点、提供商选择、MCP 配置、hooks、skills 和 instructions = [...] 始终属于用户全局配置——仓库里的 config.toml 声明 api_key、base_url 或 provider 会被忽略，克隆的仓库无法借此选择任意本地文件进入提示词。",
  credentialsTitle: "凭据查找",
  credentialsLead:
    "当前提供商的 API 密钥按以下顺序解析，先命中者生效：路由自身的认证约定（OAuth 路由使用已授权的令牌；auth_mode = \"none\" 不发送密钥），然后是显式 {apiKey}，然后是配置文件中的 api_key，然后是 api_key_env 绑定，然后是 codewhale auth set 写入的密钥存储（默认是 ~/.codewhale/secrets/ 下的文件；仅当 CODEWHALE_SECRET_BACKEND=system 时才使用系统 keyring），最后是提供商自己的环境变量（只会发送到该提供商的官方端点）。{authStatus} 可以查看当前提供商的配置文件、密钥存储后端、环境变量、生效来源和末四位标签，而不会打印密钥本身。托管、OpenAI 兼容、自托管或 Anthropic 原生路由用 {providerConfig} 或 {providerFlag} 选择；完整注册表见模型与提供商页和 docs/PROVIDERS.md。",
  legacyTitle: "旧版 .deepseek/ 路径",
  legacyLead:
    "Codewhale 由 DeepSeek-TUI 更名而来。为了不破坏既有安装，运行时从新的 ~/.codewhale/ 位置读取状态，但在只有旧目录存在时回退到 ~/.deepseek/，并且始终写入 ~/.codewhale/——读取带回退、写入新位置。状态目录解析集中在 crates/config/src/lib.rs 的 resolve_state_dir / ensure_state_dir 中，每一处旧路径引用都有审计过的保留决定。",
  sourceNote: "来源文档：docs/CONFIGURATION.md, docs/LEGACY_PATHS.md · 更新时请同步修改 docs-map.ts。",
};
