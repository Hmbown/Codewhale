# 编写你的第一个 Codewhale 插件

> 英文原文：[PLUGIN_AUTHORING.md](../PLUGIN_AUTHORING.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-29。
> 2026-10-06 核对 DSH 静态转换、显式 Native 入口及本地 Node MCP 工作目录边界。
> 2026-10-07 同步 DSH 经审查 Native 组合与 skill 根目录注册的说明。

先从一个 skill 开始：把 Markdown 指令文件放进一个小型插件包。
[hello-codewhale 示例](../examples/plugins/hello-codewhale/plugin.json)
只有两个文件，不声明服务器或 hook，也不要求调用工具。
本指南带你从源文件开始，完成审查、启用和调用。

## 1. 创建插件包

直接使用仓库中的示例，或在已安装插件目录之外创建以下目录：

```text
hello-codewhale/
├── plugin.json
└── skills/
    └── hello/
        └── SKILL.md
```

`plugin.json`：

```json
{
  "$schema": "https://agent-plugins.org/schemas/plugin.json",
  "name": "hello-codewhale",
  "version": "0.1.0",
  "description": "A minimal, explicitly invoked greeting skill."
}
```

`skills/hello/SKILL.md`：

```markdown
---
name: hello
description: Greet the user when they explicitly try the hello-codewhale example.
invocation: explicit-only
---

Respond with one short greeting in the user's language. Include the exact text
`hello-codewhale:hello` so they can identify the example they invoked.

Use only the conversation. Do not call tools, run commands, read or write files,
or contact external services.
```

这里保留与仓库一致的英文示例；指令要求用用户的语言打招呼，并包含
`hello-codewhale:hello`，同时不调用工具、不运行命令、不读写文件、不联系外部服务。
Codewhale 会自动发现 `skills/`。`explicit-only` 表示此示例不进入模型的自动
skill 目录，而是由你按名称加载。调用元数据见
[Skills](./SKILLS.md#调用与别名元数据)，清单的权威格式以
[插件包契约](./PLUGIN_BUNDLES.md#清单)为准。
新原生插件使用 `plugin.json` 即可，无需再写另一份清单。

## 2. 安装、检查并信任

在仓库根目录启动 Codewhale，然后在 **Codewhale 会话内**逐条输入：

```text
/plugin install ./docs/examples/plugins/hello-codewhale
/plugin validate hello-codewhale
/plugin show hello-codewhale
```

如果使用自己的插件包，把安装路径替换为对应目录。安装会将插件复制到
`~/.codewhale/plugins/hello-codewhale/`，初始状态为未启用、未信任。
审查已安装的源文件、skill 清单和权限。此示例应只声明 Skills，
没有 MCP 服务器、hook 或申请访问的网络主机。

安装审查会打印一条包含两个完整哈希的命令：

```text
/plugin trust hello-codewhale <full-content-sha256>.<full-capability-sha256>
```

审查后执行实际打印的完整命令；上面的尖括号内容只是占位符。
需要重新查看审查内容时，输入不带令牌的 `/plugin trust hello-codewhale`。
信任操作记录已审查的内容哈希和能力哈希，并创建运行时快照，尚不会启用插件。

```text
/plugin enable hello-codewhale
/skills hello-codewhale:
/skills inspect
```

skill 的名称是 `hello-codewhale:hello`，即插件名加 skill 名。
`/skills inspect` 会标明它来自已审查的插件快照。

## 3. 调用并停用

```text
/skill hello-codewhale:hello
```

Codewhale 确认激活后，再发送普通消息 `请打个招呼。`。
回复应是一句简短问候，并包含 `hello-codewhale:hello`。
示例只提供指令；回复仍使用你选择的模型及其正常的提供商连接。
本地安装、审查和激活步骤本身不需要调用模型。

```text
/plugin disable hello-codewhale
```

停用会移除插件提供的功能，但保留信任记录。之后执行
`/skill hello-codewhale:hello` 不应再激活该 skill。
只要已审查的哈希仍然匹配，就可以在需要时重新启用。

## 4. 修改并重新审查

已安装的插件是副本。修改示例的原始源文件不会更新该副本。
要尝试修改后的本地源文件，先停用并卸载已安装的示例，再重新安装源目录：

```text
/plugin disable hello-codewhale
/plugin uninstall hello-codewhale
/plugin install ./docs/examples/plugins/hello-codewhale
/plugin validate hello-codewhale
```

卸载只删除已安装的副本，保留原始示例源文件。审查新的令牌，确认信任后再启用。
从远程来源安装的插件使用 `/plugin update <name>`；详情见
[插件安装指南](./PLUGINS.md#更新与卸载)。

直接修改已发现的插件包内的文件后，使用 `/plugin reload` 刷新注册表。
即使版本号没变，内容变化也会使旧信任记录失效。重新加载不会授予信任。
要主动撤销信任，使用 `/plugin revoke <name>`。

## 只添加需要的组件

所有组件都使用同一套插件审查流程和现有 Codewhale 运行时：

| 组件 | 编写方式 |
| --- | --- |
| Skills | `skills/<name>/SKILL.md`；参见[指令与调用契约](./SKILLS.md)。 |
| MCP | 与清单同级的 `mcp.json`；参见[插件传输与凭据规则](./PLUGIN_BUNDLES.md#校验两种格式)。 |
| Commands | Markdown 命令文件；参见[命令元数据](../architecture/command-dispatch.md#user-commands)。 |
| Agent profiles | Fleet TOML 配置文件；参见[Fleet 编写指南](./FLEET.md#编写-agent-配置fleet-setup)。 |
| Hooks | `HooksConfig` TOML 文件；参见[事件与进程行为](./HOOKS.md)。 |
| Native mod（实验性） | 已审查的 ESM 入口，可注册工具、命令、执行前监听器、提示词片段及插件本地 JSON 状态；参见[扩展契约](./EXTENSIONS.md)。 |

Commands、Agents 和 Hooks 的路径声明放在 `plugin.json` 的
`extensions["net.codewhale"]` 中，详见
[插件组件契约](./PLUGIN_BUNDLES.md#生效与未生效的组件面)。
不要把 MCP 服务器字段或任意运行时入口放在清单根级。
LSP 可以列入清单，但目前没有可执行的适配器。`native` 原生扩展默认只列入清单；
开启实验性的 `[features] extension_host` 后，它必须指向一个 `.mjs`、`.js` 或 `.mts`
ES 模块文件，由 TypeScript 扩展宿主运行，`/plugin validate` 会拒绝其他入口。
其工具始终使用 `Required` 审批要求，不采信插件自行声明的只读提示；
Full Access（完全访问）、Bypass，或针对该已审查构建的精确会话授权，都可以满足这一要求而不再弹出审批
（[设计文档](../design/TS_EXTENSION_HOST.md#as-built-phase-1-2026-09-25)）。
可运行的类型化示例、生命周期和按插件归属显示的诊断见
[扩展编写指南](./EXTENSIONS.md)。`.mts` 只支持 Node 可直接擦除的类型语法，无需单独的编译器；需要转换的语法不受支持。

## 编写有明确作用范围的 Native mod

[mod-extension 示例](../examples/plugins/mod-extension/README.md) 是可运行的
ESM 插件包，不需要安装依赖或运行编译器。它注册 `mod_counter` 工具、
`/mod-count` 用户命令、一个提示词片段，以及只处理自身工具的执行前监听器。
工具返回结构化 JSON 计数结果；`ctx.storage` 将计数保存在 Rust 指定的插件目录中。
清理函数可重复调用，会撤销注册并等待排队的工作结束。停用后监听器和提示词片段
被撤回，计数状态仍然保留。

先明确开启实验性的 `extension_host` 功能，再安装示例目录，验证并查看其 Native
能力。由本人审查源代码和精确的内容/能力哈希，执行审查给出的信任命令后再启用。
功能关闭时，Native 代码只列入清单，不执行。源文件变更需要重新审查；
`/plugin reload` 是手动刷新，不是自动监视或热重载。

可用的编写服务包括 `tools`、`commands`、`prompt`、`storage`、`logger` 和 Cordis
生命周期机制。`ctx.on('tools/pre-execute', ...)` 可以返回不干预、拒绝、请求审批、
修订对象输入或补充上下文。Rust 合并这些提议；输入变更后重新准备调用并检查权限。
`allow` 不授予批准，`next()` 只表示不干预。异常、格式错误、超时及已撤销的所有者
都会使当前调用被拒绝。这是执行前提议契约，不提供包裹实际执行的中间件，也不支持
改写执行后的结果。

`ctx.prompt.registerSection({id, text})` 提交有边界、带来源的指令，通过现有 Engine
运行时消息进入上下文。`ctx.storage.get/set/delete` 保存有大小限制的插件本地 JSON，
可跨代保留。两者都不能替换系统提示词、会话存储或凭据服务。工具和命令调用可获得
本次调用的只读 `sessionId`、`agentId`、`originTurnId` 字符串，Rust 未提供时字段缺省；
这些是标识标签，不是运行时操作句柄。目前没有已发布的插件编写 SDK，示例使用文档
规定的宿主适配服务。

`ctx.skills.registerRoot({path: 'profiles/review-skills'})` 把已审查包内的子 `SKILL.md`
技能包加入现有的 Rust 技能目录；其 disposer 会撤销该根目录，禁用、撤销信任和宿主退出
同样会撤销。发现或加载 skill 时 Rust 会重新检查 Native 审查凭据、文件哈希和当前注册；
进程或宿主重启后，先前保存的 Native 选择失效。路径规则与数量/字节上限见
[技能根目录契约](./EXTENSIONS.md#注册技能目录)。该 API 只增加
指令，工具权限仍由共享引擎掌握。

下文的 DSH 导入器既能转换可移植的声明式组件，也能为封闭的包准备一份经审查的 Native
组合。自定义 Ratatui/GPUI 控件、DSH 浏览器 UI 插槽及 DSH 自身的 agent 运行时尚未提供。兼容的 Claude 插件包仍使用原有的声明组件适配器；此 Native API 不加载
Claude 的 agent loop，也不会自动适配 Pi 扩展 API。可执行 mod 需按文档中的宿主契约
移植。扩展工具只有在模型直接调用它、且共享 turn gate 正在处理本次调用时，
才能使用 `exec.core.call`。命令及激活代码没有 core 句柄。嵌套 shell 和网络调用必须
弹出用户审批；无法打开审批的模式会拒绝执行。完整的拒绝列表、取消语义和调用上限
见[请求核心执行工具](./EXTENSIONS.md#请求核心执行工具)。

插件信任**不是操作系统沙箱**。本地 MCP 服务器或 hook 可以启动进程；
启用前需审查其代码和权限。Skills 不授予权限：仓库指令、权限规则、沙箱策略
和工具审批仍然适用。不要在插件包或命令参数中保存凭据。
MCP 使用[插件验证契约](./PLUGIN_BUNDLES.md#校验两种格式)规定的、
经过审查的环境变量引用；添加 hook 前，请阅读独立的
[hook 环境契约](./HOOKS.md#hook-进程环境)。

## 转换现有插件

[`scripts/convert-plugin.py`](../../scripts/convert-plugin.py) 将明确选定的远程 MCP
声明、已打包的本地 Node MCP 服务器和可移植 Skills 转换为原生插件包。
它需要 Python 3.10+ 和 PyYAML 6+；如果缺少，请单独安装。转换器不会安装依赖、扫描现有应用配置或凭据、
发送网络请求，也不会执行源代码。

### OpenCode

将以下纯 JSON 保存为 `opencode-mcp.json`：

```json
{
  "mcp": {
    "docs": {
      "type": "remote",
      "url": "https://example.invalid/mcp",
      "oauth": false,
      "enabled": false
    }
  }
}
```

在 Codewhale 仓库根目录的 shell 中运行：

```sh
python3 scripts/convert-plugin.py --format opencode-v1 \
  --config ./opencode-mcp.json --name migrated-tools --output ./migrated-opencode
```

如果数据采用 `mcp.servers.<name>` 结构，请选择 `--format opencode-v2`；
该格式的服务器开关是 `disabled`，不是 `enabled`。根据数据选择格式，
不要依据文件名或上游分支名推断版本。两种格式的远程服务器都要求明确设置 `oauth: false`。
远程 MCP 输出**仅使用 Streamable HTTP**，不会复现 OpenCode 回退到旧版 SSE 的行为。
对于仅支持 SSE 的端点，请手工编写包含 `type: "sse"` 的原生 `mcp.json`，
并使用相同的审查流程。

选定 MCP 服务器时，配置中若包含 `tools`、`permission`/`permissions`、
`agent`/`agents`、旧版 `mode` 或 `default_agent`，转换将被拒绝。
这些设置可能在服务器开关之外进一步限制工具访问。请先在 Codewhale 中手工保留
这些限制，再提供只含 MCP 声明的输入；直接删除这些设置可能扩大访问权限。

转换器不接受 JSONC 注释或末尾多余逗号；请提供只含待移植声明的纯 JSON 副本。

### DeepSeek Harness（DSH）

将以下静态 Cordis 条目列表保存为 `dsh-mcp.yml`：

```yaml
- name: '@deepseek-ai/dsh-mcp-client'
  disabled: true
  config:
    serverName: docs
    transport: streamable-http
    url: https://example.invalid/mcp
```

```sh
python3 scripts/convert-plugin.py --format dsh \
  --config ./dsh-mcp.yml --name migrated-dsh --output ./migrated-dsh
```

DSH 输入也可以使用 JSON，但必须是普通条目列表，不能是完整 profile 或
patch 组合。每个条目的 `name` 都必须为 `@deepseek-ai/dsh-mcp-client`。

真正的 DeepSeek Harness 插件包（即 `package.json` 中声明了 `dsh.bundle.patch` 的
npm 包）由 Codewhale 原生导入，而不是通过此脚本（`--bundle` 已停用）：

```text
/plugin import dsh ./node_modules/@demo/tools-dsh
/plugin import dsh approve <package-dir> <content-hash>
```

第一条命令把该包转换到临时目录，并显示哪些内容会被转换、哪些会被跳过、插件包将
申请的网络主机和本地进程，以及它的内容哈希；此时不会安装任何东西。`approve` 通过
普通的、经过审查的安装器，精确安装刚才转换出的那个插件包：安装后处于未启用、未信任
状态；如果包在审查之后被修改，则不会安装任何内容。把包目录当作普通的
`/plugin install <dir>` 来安装，也会走同一个导入器。`/plugin update <name>` 会重新转换
已记录的包；输出发生变化时会替换原插件包，并使其信任记录失效。Runtime API 通过
`POST /v1/apps/plugins/import/dsh/preview` 提供同样的审查，其返回的 `install_source`
和 `content_hash` 用于 `POST /v1/apps/plugins/install`。

`dsh.bundle.patch` 可以指定单个 patch 文件，也可以指定有序的文件列表。导入器只读取
包内、非链接的文件（合计最多 64 个文件、1 MiB 的 patch 数据），然后在一个空 profile
上应用 `insert` 和带键的覆盖。与上游的 `applyEntryPatches` 一致，覆盖会替换整个字段：
`config` **不会**做深度合并。这不是完整的 profile 解析器：其他插件包、用户叠加层和
部署配置都不在其中。普通 YAML 标量遵循 YAML 1.2 core schema，与 DSH 自己的解析器一致。

在可移植的声明式转换中，被禁用的组会把禁用状态传递给其后代。被禁用的 MCP 声明保持禁用。被禁用的 skill 会被
省略并给出明确的回执，因为原生 skill 格式没有禁用状态。组或可移植条目上带条件的/
非布尔的 `disabled` 值会使导入被拒绝，而不会假定它已启用。不受支持的条目策略/依赖字段
（包括 `inject`、`intercept` 和 `isolate`）同样会使包含它们的条目或组的导入被拒绝。
请在手工移植时保留它们的激活规则和权限规则。

包含受支持的本地模块、包内 ESM 导出或受支持的 DSH 服务桥的包可以生成 Native 组合：
导入器封存包内源文件和模块哈希，然后生成一个或多个 Native 入口。只要 Native 图中有一行
无法解析或不受支持，整个图都会被拒绝，不会悄悄安装恰好能解析的部分。经审查的 agent 预设包可以生成各自的 Native 入口，
其贡献范围由所选的默认或子预设决定。导入和审查永远
不会执行插件代码。`dsh.client` 界面代码和上游 agent loop 不在此契约之内。无法表示的可移植组件会记录在
插件包的 `CONVERSION.md` 和结构化的 `CONVERSION.json` 中，其中包含源包名/版本、清单和
有序各层的 SHA-256 哈希、转换器版本、逐条目的结果，以及需要手工移植的内容。未应用的
patch 操作也会出现在结构化的手工移植列表中，并附带其来源层和从 1 开始的操作序号。

在可移植转换中，能被转换（lowering）的 `!!js` 表达式只有 `process.execPath`（变成 `node`）和不含转义或插值的
简单带引号/模板字面量。环境变量表达式（包括带回退值的）永远不会按本机环境求值。
已打包的 Node MCP 只有在其相对入口（以及声明的相对工作目录）解析后位于包内时才会被
转换；包外的服务器会被跳过，宿主路径永远不会被复制。`@deepseek-ai/dsh-skill-filesystem`
条目只有在其字面的 `customSkillDirs` 子目录位于包内时，才会贡献这些子目录。默认的用户
和项目 skill 根目录、文件监视器以及外部服务依赖都不会被导入。DSH 模块仅通过实验性的
TypeScript 扩展宿主运行（`[features] extension_host`，默认关闭），可以来自明确编写的 Native 入口，
也可以来自导入器生成并经过审查的组合。生成的 Native 组合在显式启用 `extension_host`、且已安装的包
经过审查、信任并启用之前保持不活动；宿主在加载前校验其源文件闭包，Rust 保留工具审批和会话权限。
工具、命令、提示词片段、技能、钩子和 owner 本地存储沿用 Codewhale 现有的消费者与审批规则。
这是受支持的兼容子集：导入成功并不表示每个上游 DSH 服务都可用。参见[扩展编写指南](./EXTENSIONS.md)与
[宿主设计](../design/TS_EXTENSION_HOST.md)。

### 本地 Node MCP 服务器

对于已打包的 Node MCP 服务器，使用 `--stdio-root SERVER=DIRECTORY` 明确选择
原进程的工作目录。转换器将整个目录复制到 `mcp/<server>`，并把原生服务器的
工作目录设为经审查的副本，保留相对入口导入和只读资源的目录布局。

```json
{
  "mcp": {
    "localdocs": {
      "type": "local",
      "command": ["node", "server.mjs"],
      "environment": {"API_TOKEN": "{env:LOCALDOCS_TOKEN}"},
      "enabled": false
    }
  }
}
```

```sh
python3 scripts/convert-plugin.py --format opencode-v1 \
  --config ./local-mcp.json --stdio-root localdocs=./packaged-localdocs \
  --name local-tools --output ./migrated-local
```

每个本地服务器都必须提供对应的 `--stdio-root`。OpenCode v2 使用 `mcp.servers`
和 `disabled`。静态 DSH 条目使用 `transport: stdio`、`command: node`、
`args: [server.mjs]`；DSH 的 `env` 必须省略或为空，其字面量和表达式不按
OpenCode 环境引用解释。DSH/v2 的 `cwd` 只能省略、为空或为 `.`；选定目录明确
指定原进程的工作目录。只有远程 OpenCode 服务器需要 `oauth: false`。

支持 `node` 加一个相对 `.mjs`、`.js` 或 `.cjs` 入口。原生启动适配器保留包的模块类型
和同级模块导入。TypeScript 需先编译为 JavaScript；转换器不会运行编译器。依赖和只读资源必须
预先打包在该目录内；转换时不运行包管理器、安装脚本、模块加载器或服务器。
符号链接/reparse point、硬链接文件、隐藏文件或目录（包括 `.gitignore`、
`.env*`、`.npmrc` 和 `node_modules/.bin`）、常见凭据文件名及私钥容器会被拒绝。
请准备干净的打包目录；不会根据忽略规则静默遗漏文件。转换前检查每个选定文件
是否嵌入凭据。整个输出仍受 4,096 个文件 / 64 MiB 限制。

Shell 启动器、Node 标志、额外参数、其他解释器、环境变量字面值以及改变模块
加载方式的环境变量名均不支持。写入工作目录、依赖实时工作区或导入包外文件的
服务器需要手工移植。复制文件不等于静态验证 JavaScript 依赖闭包，也不提供
代码沙箱。本地 MCP 以宿主用户权限运行；远程端点的主机声明不会限制本地进程
的网络或文件访问。Codewhale 启动服务器前仍需完成原生安装、能力审查、哈希绑定
信任和启用流程。此功能是已打包 Node MCP 子集，不执行 DSH/Cordis 插件模块。

### 审查转换结果

两个示例都保留服务器的停用状态，并使用占位端点。准备连接时，先替换源文件中的
端点并修改启用开关，再重新转换。输出目录必须尚不存在，且其父目录已存在。
已有输出会被拒绝覆盖；输入被拒绝时不会留下输出插件包。

添加 `--skill ./my-skill` 可选择包含 `SKILL.md` 的目录，也可以用
`--skill ./my-skill.md` 选择单个文件；重复该选项可选择多个 skill。
仅转换 Skills 时可以省略 `--config`。Skills 的 frontmatter 必须包含
`name` 和 `description`。`disable-model-invocation: true` 会转换为原生的
`invocation: explicit-only`。说明性字段 `license`、`compatibility` 和
`metadata` 会保存在伴随数据文件 `SOURCE_SKILL_METADATA.json` 中。
所选 skill 目录中的其他文件会作为数据复制；加载 skill 前需审查这些文件及其指令。

只有完全符合 `{env:MCP_TOKEN}` 形式的 OpenCode 请求头引用会转换为原生
`env_headers`；转换器不会读取变量值。字面量请求头、DSH 请求头表达式以及
URL 中的文件或环境变量替换都会被拒绝。配置的超时值必须是以毫秒表示的整秒数，
范围为 `1000` 到 `3600000`。未指定的超时使用 Codewhale 的默认值。

外部运行时的可执行插件和 hooks、其他 stdio 启动方式、自动 OAuth、配置中的
JavaScript、YAML 别名或标签、
`__jsExpr`，以及不支持的 skill 运行时字段（包括 `user-invocable: false`）
需要手工移植。转换不会复现其他客户端的运行时，也不会绕过 Codewhale 的凭据
和沙箱规则。

阅读生成的 `CONVERSION.md`、`CONVERSION.json`（针对插件包）、`plugin.json`、`mcp.json`（如有）以及
所有选定的 skill 和 MCP 源文件。然后使用 `/plugin install ./migrated-opencode`（或 DSH 输出路径）、
`/plugin validate <name>`，并按前文相同的哈希绑定流程审查、信任和启用。
转换本身既不证明连接可用，也不证明运行时兼容；输出尚未安装、信任或启用。

源码核查日期：2026-09-08。依据 OpenCode `d6855b6b47` 的
[v1 MCP 文档](https://github.com/anomalyco/opencode/blob/d6855b6b47a8433462ac6aeeba882ccf734cb7f1/packages/web/src/content/docs/mcp-servers.mdx)
和 [v2 MCP schema](https://github.com/anomalyco/opencode/blob/d6855b6b47a8433462ac6aeeba882ccf734cb7f1/packages/core/src/config/mcp.ts)，
以及 DSH `c389f96bf3` 的 [MCP 客户端参考](https://github.com/deepseek-ai/deepseek-harness/blob/c389f96bf3a9b6807cb71ed6bdad5849be0df6d8/packages/mcp/mcp-client/README.md)。
Bundle patch 的语义已对照 DSH
[`00102833df`](https://github.com/deepseek-ai/deepseek-harness/tree/00102833dfaee1da9f48a3a8eae9d34005a75218)
（`0.1.7-alpha.2`）重新核对；`scripts/fixtures/dsh-web-app` 保留其真实的五文件包，
仅作为解析器测试数据，不是可导入的原生插件。CI 使用 Python/PyYAML 和合成的 Node
夹具运行离线转换器测试语料。上游支持的功能多于这个有意限定范围的转换器。

## 社区背景

本指南回应了 [giancarlocp 在讨论 #5827 中提出的插件编写指南与 OpenCode
转换需求](https://github.com/codewhale-hq/Codewhale/discussions/5827)。
简体中文版本遵循 [SparkofSpike 在 issue #5482 中提出的中文文档工作方向](https://github.com/codewhale-hq/Codewhale/issues/5482)。
