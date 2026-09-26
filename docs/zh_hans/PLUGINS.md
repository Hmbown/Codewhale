# 安装插件

> 英文原文：[PLUGINS.md](../PLUGINS.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-26。

想做一个插件包，先从[编写你的第一个 Codewhale 插件](./PLUGIN_AUTHORING.md)
和里面可运行的 Skills 示例入手。

本文是 `/plugin install` 这条入口链路的分步说明（v0.9.4，#5182）。
插件包的格式、发现、校验，以及信任/启用生命周期，仍以
[PLUGIN_BUNDLES.md](./PLUGIN_BUNDLES.md) 为准；插件包格式即 `plugin.json`、
兼容的 `kimi.plugin.json`、`.claude-plugin/plugin.json`，或旧版 `plugin.toml`。
本文讲的是文件最初怎么落盘。

`/plugin suggest <task>` 是一个本地、只读的配套命令：它会按名称、关键词、
描述、所含 skill 名、声明的主机，给已安装的插件包排序，也给你用
`/plugin marketplace add` 添加的市场目录排序。它还会说明匹配的原因，
并给出下一步：审查、启用，或从目录安装。但它自己从不安装、信任或启用插件包。

## Codewhale 如何推荐插件

Codewhale 在插件上只帮忙，不推销。规则如下：

- **只有一处会主动提示。** 当你发送的任务匹配到某个已安装但闲置的插件，
  或某个你还没有的目录候选项时，可以弹出一条安静的 toast，
  例如 `/plugin trust supabase` 或
  `/plugin marketplace install <catalog> supabase`。你打字时什么都不会出现，
  插件推广内容也不会被塞进你的消息里。
- **只有一个开关。** 关掉 `contextual_tips` 之后，任何地方都不会再出现插件引导。
  必须出现的通知和你自己执行的 `/plugin` 命令照常可用。
- **只有一份配额。** 插件推荐与其他提示共用每会话的引导配额。
  在交互式 TUI 里，模型每个会话可以调用一次 `request_plugin_install`，
  请你审查任务需要的某个插件；第二次调用会失败。
  Exec、ACP 和 runtime-API 会话拿不到这个工具。
- **内置插件从不作为推荐出现。** Computer Use 这类内置插件，
  只会出现在 `/plugin list` 和 Extensions 里。
- **只认具体词。** 泛化词（accessibility、browser、chrome、docs、screenshot、
  web、wiki 等）永远不触发推荐。匹配器和市场的 `check-marketplace.mjs`
  共用同一份停用词表。
- **只推荐本机跑得起来的。** 插件的 `when.os` 若排除了当前操作系统，就不会被推荐。
- **给出真正的下一步。** 模型请求的那一行会按插件的实际需要写成 Install、
  Review trust 或 Enable。只有那个按钮可点，它打开的是 `/plugin show <name>`；
  它从不安装、信任或启用。
- **关闭可以撤销。** 按 Esc 时，如果草稿非空，先清空草稿，然后才隐藏这一行，
  而且只对当前会话生效。“Don't suggest again”是你显式做出的选择，
  会被持久保存。`/plugin dismissals` 会列出这两种；
  `/plugin dismissals reset [<name>]` 可以让被关闭的插件重新出现在推荐里。
- **新插件要靠自己找。** 想找新插件，就看这些文档、`/plugin marketplace list`、
  Extensions，以及下面的浏览器指南。

Codewhale 不会凭空编造远程插件 URL；缺失的插件只会从你添加过的目录里推荐。
磁盘上的插件包有变化时，发送消息时和回合之间仍会弹出 `/plugin reload` 的 toast。

## 浏览器：选一个

操控浏览器有好几种方式。区别在于用谁的浏览器，以及它能看见什么。

| 选项 | 用的是谁的浏览器 | 适合 |
| --- | --- | --- |
| `chrome-devtools` MCP（`/mcp recommendations`） | 它自己驱动的一个 Chrome，可以包含已登录的页面 | DevTools 级别的检查和性能分析 |
| Playwright MCP（`/mcp recommendations`） | 带 `--isolated` 的全新隔离 profile | 不带你身份的脚本化流程和测试 |
| Computer Use 的 `browser_*` 工具（内置，审查前关闭） | 它自己启动的浏览器，用独立的 profile | 更宽泛的桌面任务里的浏览器步骤 |
| Chromewhale（开发者预览版，`Hmbown/codewhale-plugin-marketplace`） | 你自己的、已经打开的 Chrome profile；以 unpacked 方式加载 | 读取或操作你当前正看的标签页，每次授权一个站点 |

这些都不会主动推荐给你。按任务需要自己添加。

## 来源

`/plugin install <spec>` 接受三种来源：

```text
/plugin install ./path/to/bundle            # local directory (copied)
/plugin install github:owner/repo           # GitHub archive of the default branch
/plugin install https://example.com/x.tar.gz  # direct tarball URL
```

v1 没有注册表索引，也不做 `git clone`，只拉 tarball。
安装器的大小上限和“不允许符号链接”这两条保证，正靠这一点守住。
下载由逐域名的网络策略把关：未知主机会返回一条“需要审批”的错误，并点出主机名
（先 `/network allow <host>`，再重试）；被拒绝的主机直接中止，不碰磁盘。

拉下来的目录树里必须**有且只有一个**插件包根目录。所谓根目录，
就是放着 `plugin.json`、兼容的 `kimi.plugin.json`、`.claude-plugin/plugin.json`
或旧版 `plugin.toml` 清单的那个目录。Kimi 插件包使用 Codewhale 兼容的 skills、commands、agents 和 MCP 声明时
会被接受；不支持的 Kimi 运行时字段会失败关闭，而不是被静默忽略。
插件包落到用户插件根目录 `~/.codewhale/plugins/<name>/`，
其中的 `<name>` 来自清单里的插件名。

Claude 插件包的元数据放在 `.claude-plugin/plugin.json`，组件放在插件包根目录。
导入器支持 skills、commands、agents，以及内联声明或写在根目录 `.mcp.json` 里的
MCP 服务器（服务器映射可以是平铺的，也可以包在 `mcpServers` 下）。
Claude 的 `http` 传输映射为 Streamable HTTP。
`.claude-plugin/marketplace.json` 目录里的相对来源，会从市场仓库的根目录解析。
整个插件包仍要接受与原生插件相同的审查哈希和路径检查。

远程 MCP 请求头可以只写出凭据的名字，不把凭据本身写进去：authorization 值只要与
`Bearer ${ENV_NAME}` 完全一致，就会转成 `bearer_token_env_var`；请求头值只要与
`${ENV_NAME}` 完全一致，就会转成 `env_headers`。导入过程不读取任何凭据值。
字面量凭据和复合模板会被拒绝。

这是一个兼容子集：hooks、LSP 声明、自定义 MCP 文件路径和
`${CLAUDE_PLUGIN_ROOT}` 展开都会被拒绝，并给出原因；不会装出半个插件。
安装远程 MCP 声明不等于完成它的身份验证。
插件贡献的远程服务器仍保留现有的显式凭据要求；这个导入器不启用插件 OAuth。

## 引导流程

安装从不激活任何东西。命令把文件放好，然后直接把你带进标准的能力审查：

```text
/plugin install github:someone/neat-plugin
→ Installed plugin 'neat-plugin' to ~/.codewhale/plugins/neat-plugin.
  It is disabled and untrusted. Review its requested authority below…
  <full inventory, permissions, MCP authority render>
  /plugin trust neat-plugin <content-hash>.<capability-hash>

/plugin trust neat-plugin <paste the token>   # records the hash-bound receipt
/plugin enable neat-plugin                    # activates for this workspace
```

这里渲染的审查内容和确认 token 与 `/plugin trust <name>` 完全相同。
信任走的是严格的、绑定哈希的回执流程，不是一个仅供参考的标记。
插件包的内容或声明的能力一旦变化，回执就不再匹配，插件随之失效，直到你重新审查。

## 更新与卸载

```text
/plugin update <name>      # re-download, byte-compare, atomic swap if changed
/plugin disable <name>     # required before uninstall
/plugin uninstall <name>   # deletes the bundle and prunes its state entry
```

- `update` 会重新下载记录在案的来源。字节完全相同就什么都不做；
  有变化的插件包会被原子替换，其信任回执自动失效（哈希不再匹配），
  所以想让插件再次激活，必须重新审查。从本地路径安装的插件无法重新下载。
  要替换已安装的副本，先停用再卸载，然后运行 `/plugin install <path>`
  并审查新的插件包；原来源目录保持不动。参见
  [本地编写循环](./PLUGIN_AUTHORING.md#4-修改并重新审查)。
- `uninstall` 拒绝卸载已启用的插件（先停用），删除插件包目录，
  并移除它持久化的信任/启用记录。

## 安全规则

- 每次安装都会带一个 `.installed-from` 来源标记。缺少这个标记的插件包，
  安装器**拒绝覆盖或删除**——手工放到 `~/.codewhale/plugins/` 下的插件包，
  永远不会被覆盖。
- tarball 有大小上限，而且会先解压到一个私有暂存目录。路径穿越（`..`、绝对路径）
  和插件包里的符号链接/硬链接都会被拒绝；等所有检查都通过，
  目标目录才会出现，这一步走的是原子重命名。
- 安装前会拿名称与内置插件包、工作区插件包比对，
  防止优先级更高的插件包被这次安装悄悄遮蔽，也防止它反过来遮蔽这次安装。
- 新装的文件一律**停用且未受信任**；启用只能走上面那套显式的信任审查。
