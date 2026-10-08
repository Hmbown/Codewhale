# 用户记忆

> 英文原文：[MEMORY.md](../MEMORY.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-29。

用户记忆给模型提供一小块本地持久存储，用来存放应当跨会话保留的偏好和约定——
“我更喜欢 pytest 而不是 unittest”、“这个代码库用 4 空格缩进”——这样就不用每次
对话都重复一遍。

从 v0.9.4 起，**原生记忆存储**是唯一的记忆系统。它由 Markdown 文件组成，
用 SQLite FTS5 建索引，完全离线，作用范围按仓库 git origin 的哈希划分。
旧的单文件（`~/.deepseek/memory.md`）推送/注入路径，以及计划中的 Moraine MCP
后端，都已移除。仓库里从未发布过 Moraine 服务器，而原生存储已经提供了同样的架构：
持久的 Markdown 数据源，加上可重建的搜索索引。

记忆需要**主动启用**。默认禁用状态下，不会加载任何内容，不会拦截任何输入，
`remember` 工具也不会出现在模型面前。

## 启用记忆

方式一，设置环境变量：

```bash
export DEEPSEEK_MEMORY=on
```

可接受的“真值”有 `1`、`on`、`true`、`yes`、`y` 和 `enabled`。

或者，在 `~/.codewhale/config.toml` 里添加：

```toml
[memory]
enabled = true
```

切换后需要重启 TUI。关闭记忆就是反过来做同样的设置。

## 目录结构

存储放在旧版 `memory_path` 锚点旁边，是一个 `memory/` 目录。默认的
`memory_path = "~/.codewhale/memory.md"` 会重新落到 `~/.codewhale/memory/`：

```text
~/.codewhale/memory/
├── global/MEMORY.md        # user-scoped notes (follow you everywhere)
├── workspace/<id>/MEMORY.md   # repo-scoped notes (hash of git origin)
└── index.sqlite3           # rebuildable SQLite FTS5 cache
```

作用域目录是 `workspace`（单数）——见 `MemoryScope::directory`，
`crates/runtime/src/native_memory.rs:31-36`。索引文件名是 `index.sqlite3`
（`native_memory.rs:175`）。

Markdown 是持久的数据源；`index.sqlite3` 是可丢弃的全文缓存
（用 `/memory native reindex` 重建）。配置的 `memory_path` **只是一个锚点**：
文件名会被丢掉，父目录里会多出一套 `memory/global/MEMORY.md` 目录树。
不要把 `memory_path` 设成原生布局本身的路径——那样目录树会多套一层。
随包提供的示例保留 `~/.codewhale/memory.md`，所以存储最终落在
`~/.codewhale/memory/global/MEMORY.md`。

## 注入什么内容

启用记忆后，系统提示词里会带上一段记忆条目：有大小上限，带来源标记，
最多 32 条 / 12,000 字符，涵盖全局作用域和当前工作区作用域。这段内容外面包了一层标记，
标明它是**不可信的用户数据**，而不是第二层指令。注入部分之外还需要更多内容时，
模型可以对 FTS5 索引调用 `memory_search` / `memory_get` 工具。

初始记忆快照属于会话开始时冻结的提示词前缀。会话过程中发现的变更会作为历史追加，
具体见 [CACHE.md](./CACHE.md)；原生笔记更新和外部召回，都不得在每一回合
重写系统提示词或工具目录。

## 三种添加记忆的方式

### 1. 输入框里的 `# ` 前缀（#492）

在输入框里输入以 `#` 开头（但不是 `##` 或 `#!`）的单行内容：

```
# remember to use 4-space indentation in this repo
```

TUI 会拦截这行输入，并通过模型工具所用的同一条 `NativeMemoryStore::remember`
路径，把笔记追加到**全局**原生存储。**不会触发回合**——这行输入会被直接收走，
状态行会显示写入了哪个文件，你可以接着输入真正想问的问题。

多个 `#` 组成的前缀会刻意当成普通回合提交，这样粘贴 Markdown 标题时
不会有意外。

### 2. `/memory` 斜杠命令

用来查看和维护原生存储：

`/memory` 分成两部分。不带 `native` 的子命令，针对的是 `config.memory_path()`
指向的那个单文件；原生存储的功能全在 `/memory native …` 之下
（`crates/tui/src/commands/groups/memory/memory.rs:236-268`）。

| 子命令 | 效果 |
|-----------------|-----------------------------------------------------------|
| `/memory`       | 打印 `memory_path` 文件的路径和内容 |
| `/memory show`  | 等同于裸 `/memory` |
| `/memory path`  | 打印 `memory_path` 文件的位置 |
| `/memory clear` | 清空该文件 |
| `/memory edit`  | 打印针对它的 `$EDITOR` 调用命令 |
| `/memory help`  | 显示该命令的帮助 |

其他任何输入都会返回 `unknown subcommand`。原生存储通过 `native` 前缀访问
（`memory.rs:221`）：

| 子命令 | 效果 |
|-----------------------------------------|-------------------------------------|
| `/memory native status`                 | 存储根目录、当前生效的数据源、索引 |
| `/memory native path`                   | 原生存储根目录 |
| `/memory native remember [global\|workspace] <note>` | 追加一条笔记 |
| `/memory native search <query>`         | FTS5 搜索 |
| `/memory native get <id>`               | 读取一条条目 |
| `/memory native reindex`                | 重建 FTS5 索引 |
| `/memory native import`                 | 导入旧的单文件存储 |
| `/memory native export`                 | 导出条目 |
| `/memory native delete [all\|global\|workspace]` | 删除条目 |

没有 `/memory add`，也没有不带 `native` 的 `/memory reindex`；请用
`/memory native remember` 和 `/memory native reindex`。

### 3. `remember` 工具（自动捕获，#489）

启用记忆后，模型会得到一个 `remember` 工具：

```json
{
  "name": "remember",
  "input_schema": {
    "type": "object",
    "properties": {
      "note":  { "type": "string" },
      "scope": { "type": "string", "enum": ["global", "workspace"] }
    },
    "required": ["note"]
  }
}
```

模型一旦发现值得跨会话保留的东西——偏好、约定或事实——就会用它记下来。
这个工具会自动批准：写入范围限定在用户自己的记忆文件里，要是还得走标准的
写入审批流程，自动记忆捕获就失去意义了。工作区作用域要求有一个带 `origin`
remote 的 git 仓库（作用域 id 就是它的哈希）。

## 什么不该写进记忆

记忆只存放**持久**的信号。下面这些不应该放进去：

- **机密信息**——不要放 API 密钥、令牌、密码。这些文件是磁盘上的明文，
  条目还会被注入系统提示词。
- **临时任务状态**——“我现在正在改解析器”每次会话都会变，不属于跨会话记忆。
- **对话片段**——引文式的笔记应该写进笔记工具（`note`），不是记忆。
- **长篇指令**——超过几句话的内容应该放在 `AGENTS.md`（项目级）或技能（skill）里。

## 隐私与作用域

原生存储保存在你自己的机器上，不会自动同步到云端记忆服务。启用记忆后，
召回的记忆条目会作为提示词上下文发送给所选的提供商（provider）。不要把机密写进去。
工作区作用域的记忆以仓库 git origin 的哈希为键，因此一个仓库的笔记永远不会
泄漏到另一个仓库的提示词里。

## 外部记忆服务

持久记忆靠原生存储就已经能用了。一等后端选择目前只接受 `native` 和 `off`；
没有受支持的 `external`、`mem0` 或 `memcode` 后端设置。

第三方记忆服务可以通过现有的 [MCP](./MCP.md) 或[插件](./PLUGINS.md)集成来提供
工具。那些工具是服务自己的，不会替代 `remember`、`memory_search`、
`/memory native`，也不会替代 `#` 快捷添加路径。启用之前，先看看插件要哪些权限，
以及服务会把数据送到哪里。外部服务不可用时必须如实报告失败，而不是悄悄把笔记发到别的
后端。

未来的一等后端，必须在每个记忆入口覆盖捕获、搜索、纠正、删除，
以及作用域和错误报告。它还必须在上面的缓存契约下，把易变的召回内容放进只追加的历史里。
这次完整迁移记录在 [#6050](https://github.com/codewhale-hq/CodeWhale/issues/6050) 里。
0.9.13 没有声称完成这次迁移，也没有声称提供商业记忆集成。

## 配置参考

```toml
# ~/.codewhale/config.toml
[memory]
enabled = true                    # default false; or set DEEPSEEK_MEMORY=on
# Optional explicit backend selection:
# backend = "native"              # "native" or "off" (default: off)
```

| 设置项 | 默认值 | 覆盖方式 |
|-----------------------|-------------------------------|---------------------------------------|
| 记忆开关 | `false` | `[memory] enabled = true` 或 `DEEPSEEK_MEMORY=on` |
| 后端 | `off` | `[memory] backend = "native"` |
| 存储根目录 | `~/.codewhale/memory/` | 由 `memory_path` 推导 |

## 相关文档

- `docs/SUBAGENTS.md`——子智能体（sub-agent）会继承记忆，也可以使用 `remember` 工具。
- `docs/CONFIGURATION.md`——完整的配置参考。
- Issue [#489](https://github.com/codewhale-hq/CodeWhale/issues/489)
  ——跟踪这项工作的第一阶段 EPIC。
