# 目录刷新

> 英文原文：[CATALOG_REFRESH.md](../CATALOG_REFRESH.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-26。

Codewhale 怎么让模型元数据保持最新：哪些部分已经自动更新，哪些要人工维护，
以及定时目录任务该做什么、不该做什么。

相关文档：[`PROVIDERS.md`](./PROVIDERS.md)、
RFC [`rfcs/UNIFIED_PROVIDER_LOGIN.md`](../rfcs/UNIFIED_PROVIDER_LOGIN.md)。

---

## 简短回答

| 问题 | 回答 |
|---|---|
| 用户想刷新模型，需要一个专门的模型吗？ | **不需要。** |
| Codewhale 会自动更新公开的模型目录吗？ | **会，在运行时更新**，数据来自 [Models.dev](https://models.dev/catalog.json)，TTL 约 24 小时。 |
| 离线内置种子会在 CI 里自动提交吗？ | **还不会。** 实时缓存覆盖正在运行的安装；仓库内的种子仍由人工或 PR 更新。 |
| 应该让 LLM 重写目录 JSON 吗？ | **不要。** 数据摄取是确定性的公开 JSON。LLM 能*审阅* PR，不能当事实来源。 |

---

## 分层（优先级由低到高）

共享的目录编译器按从低到高的顺序应用这些层：

```
0 bundled Models.dev
5 bundled Codewhale facts
10 live Models.dev
15 verified cloud facts (optional, off by default)
20 exact provider-owned live roster
25 Codewhale account roster
30 config.toml
40 user overrides
policy DENY (final)
```

云事实沿用现有的编译器和 provider lake，详见
[`CLOUD_FACTS.md`](../CLOUD_FACTS.md)。能力来源与价格来源相互独立：能力补丁
不能重新标注继承来的价格。云价格补丁会替换整个价格块；未指定的 token 类别
保持未知。

路由解析还会绑定提供商（provider）类型、已配置的身份和端点。提供商自己给出的新名单，
在其确切范围内就是权威。显式选定的模型依旧保持显式。Codex 的账户观测/原生
缓存和 Ollama 端点标签各自保留专属的可用性规则；公开目录里的某一行，
不能证明某个账户可以调用该模型。已安装的 Codex `account/read` 与 `model/list`
路径记录在 [`PROVIDERS.md`](./PROVIDERS.md) 中。

没有可用目录时，旧版补全列表仍是最后的兜底。内置种子和静态的
传输/计费规则仍由发布流程负责；刷新目录元数据既不会引入新的线协议方言，
也不会改变凭据/计费的归属。

关键代码：

| 组成部分 | 路径 | 作用 |
|---|---|---|
| 实时抓取 + 缓存 | `crates/tui/src/models_dev_live.rs` | 后台刷新、TTL、原子写入、新鲜度状态 |
| Schema / 解析 | `crates/config/src/models_dev.rs` | 不联网的 Models.dev JSON 结构 |
| 编译 + 来源 | `crates/config/src/catalog.rs` | 有序来源、独立的价格来源、policy deny、id 归一化 |
| provider lake 合并 | `crates/tui/src/provider_lake.rs` | 共享目录投影，提供商权威严格限定在路由范围内 |
| 离线种子资产 | `crates/config/assets/models_dev.bundled.json` | 仅作紧凑的离线兜底（`_meta.role` 已注明） |
| 校验脚本 | `scripts/catalog_models_dev.py` | 不含密钥的抓取/校验试运行（#4117） |
| 脚本测试 | `scripts/catalog_models_dev_test.py` | 离线结构/脱敏检查 |

---

## 已经自动更新的部分（运行时）

TUI/运行时启动时（且未被禁用）：

1. 若**磁盘缓存**存在，先用它预填模型选择器（哪怕缓存已过期）。
2. 缓存缺失或超过 **24 小时**时，在**后台抓取** Models.dev
   （15 秒超时、显式的 Codewhale user-agent、**不带任何凭据**）。
3. 成功时：原子写入 `~/.codewhale/catalog/models-dev-catalog.json`，
   并把结果行以 `CatalogSource::ModelsDevLive` 发布到 ProviderLake——第 10 层，
   不带端点指纹。Models.dev 是描述模型的公开目录，所以刷新出来的行，和它
   取代的第 0 层种子一视同仁，仍然可以被第 15 层修正。`CatalogSource::Live`
   保留给提供商自己、按凭据范围返回的 `/models` 应答，位于第 20 层。
4. 失败时：保留原有缓存，或退回**内置**种子。Models.dev 宕机
   绝不会让模型选择直接失败。

### 手动强制刷新

在 TUI 中：

```text
/model refresh
```

该命令派发 `AppAction::RefreshModelsDevCatalog`（异步执行，不会阻塞输入框）。
如果启用了受准入控制的云事实设置，它还会请求一次云刷新；硬禁用和信任密钥检查
依旧生效。实现位于 `crates/tui/src/commands/groups/core/core.rs` 和
`crates/tui/src/models_dev_live.rs`。

### 环境变量开关（测试 / dogfood / 离线）

| 变量 | 作用 |
|---|---|
| `CODEWHALE_MODELS_DEV_URL` | 覆盖基础 URL 或完整的 `*.json` 目录 URL |
| `CODEWHALE_MODELS_DEV_PATH` | 从本地文件加载目录；跳过网络 |
| `CODEWHALE_DISABLE_MODELS_DEV_FETCH` | 真值 → 永不访问网络（`1` / `true` / `yes` / `on`） |

默认值：

- 目录 URL：`https://models.dev/catalog.json`
- TTL：`24 * 60 * 60` 秒（`DEFAULT_MODELS_DEV_TTL_SECS`）
- 缓存文件名：Codewhale `catalog` 状态目录下的 `models-dev-catalog.json`

暴露给界面 / 状态标签的新鲜度取值：`bundled` | `live` | `stale` | `failed`。

---

## 不会自动更新的部分（仓库 / 发布）

在定时 PR 落地之前，以下内容仍需人工维护，或走发布流程：

| 表面 | 为什么会漂移 |
|---|---|
| `models_dev.bundled.json` | 离线种子；刻意比完整的 Models.dev 小 |
| `model_catalog.bundled.json` | 紧凑的 TUI 种子 |
| `provider_defaults.rs` / 默认模型 ID | 属于产品选择，不是纯粹的目录导出 |
| `models.rs` 里的静态表 | 目录缺行时的兜底启发式 |
| 人工整理的 `pricing.rs` 行 | 厂商计费的怪癖；Models.dev 里不一定有 |
| 新的 `ProviderKind` / 线协议方言 | 需要代码，光有 JSON 不够 |

运行时的实时刷新**不会**改写这些文件。最近安装、网络正常的用户仍能看到
Models.dev 的新行；离线的新克隆、CI 的封闭运行，以及没有缓存的首次启动，
仍然依赖种子。

---

## 维护者工具（不涉及 LLM）

### 校验 / 试运行抓取

```bash
# Fetch Models.dev + print counts (never writes disk)
python3 scripts/catalog_models_dev.py refresh

# Validate the committed offline seed still parses as Models.dev-shaped JSON
python3 scripts/catalog_models_dev.py snapshot --check \
  crates/config/assets/models_dev.bundled.json

# OpenRouter public /models listing (no API key), dry-run only
python3 scripts/catalog_models_dev.py refresh --provider openrouter \
  --sort newest --limit 100
```

脚本的设计约束（有意为之）：

- 只用公开端点——不带 `Authorization` 头，不用 API 密钥。
- 远程 JSON 里出现形似凭据的键，一律清除。
- **磁盘写入已禁用**（`--write` / `--write-cache` 失败关闭）。准备新种子要由
  维护者单独做一步，这样自动化就没法不经审阅就把远程 JSON 直接提交上去。

### 准备新的离线种子（手动）

1. 把 Models.dev 抓到本地文件（curl / 浏览器），或用
   `CODEWHALE_MODELS_DEV_PATH` 指向已保存的副本。
2. 清洗成允许列表内的结构（`models`、`providers`，可选的 `_meta`）。
   以脚本的公开文档规则作为检查清单。
3. 种子要**紧凑**——只放已发布提供商经过验证的默认值，不要整份导出
   （见现有资产里的 `_meta`）。
4. `python3 scripts/catalog_models_dev.py snapshot --check <path>`。
5. 仔细对比 diff：默认线协议 ID 应与离线的 `DEFAULT_*_MODEL` 保持一致。
6. 提一个普通 PR。不要强推目录历史。

可选：**在 PR 上**用一个便宜模型总结“新增 / 移除 / 默认风险”——
但绝不让它当 JSON 的作者。

---

## 推荐的定时任务（尚未发布）

目标：让**仓库内的离线种子**不至于腐烂，同时不给 CI 改写密钥的权力，
也不放任 LLM 自行改写。

```text
cron (daily or weekly)
  → fetch Models.dev (public, no keys)
  → validate shape + scrub
  → compare against crates/config/assets/models_dev.bundled.json
     (and optionally report new ids vs provider defaults)
  → if material change: open PR
       title: chore(catalog): refresh Models.dev offline seed
  → optional: agent comments a human-readable diff summary on the PR
```

### 自动化的范围内

- 以确定性方式从 Models.dev 摄取目录
- 不含密钥的 PR diff
- 漂移报告（新增模型 id、缺失的默认值、价格是否存在）

### 自动化的范围外

- Claude Pro/Max / 订阅制 OAuth 的“模型发现”（不是受支持的第三方路径；
  Anthropic 期望第三方工具使用 API 密钥）
- 未经审阅就让 LLM 改写 `models.rs` / `provider.rs`
- 强推 `main`，或在默认分支上悄悄改写资产
- 把 Models.dev 当作 OAuth 范围路由的唯一事实来源（Codex 名单仍特殊处理）

### 建议的工作流位置

`CodeWhale/.github/workflows/catalog-refresh.yml`（或类似名字），复用
`scripts/catalog_models_dev.py`，但前提是先做一次有意为之的**写安全**扩展，
让它只在 CI 里运行、用 bot token 创建 PR；如果写入会落进仓库，
即便走 `workflow_dispatch`，也仍需审阅。

如今的 nightly（`/.github/workflows/nightly.yml`）只构建发布产物，
**不**刷新目录。

---

## 需要一个“专门用来更新模型的模型”吗？

**核心流程不需要。**

| 任务 | 合适的工具 |
|---|---|
| 让用户看到的已知模型、窗口和价格跟着 Models.dev 保持新鲜 | 运行时实时抓取（已发布） |
| 让离线种子和发布资产在 git 中保持最新 | 定时 CI → PR（待建设） |
| 决定是否上调产品默认模型 | 人或代理（agent）在 PR 上*审阅* |
| 接入全新的提供商类型 / 方言 | 人工 PR + 测试 |

LLM 至多是目录 PR 的可选**审阅者**，不适合当目录 JSON 的**事实来源**。

---

## 认证说明（Claude / Anthropic）

刷新 Anthropic 的模型**目录**不需要 Claude Pro/Max OAuth。Models.dev 是公开的。
Codewhale 的 Anthropic 路由在推理时仍**基于 API 密钥**（`ANTHROPIC_API_KEY`）。
不要把目录自动化和订阅制 OAuth 或 Claude Code 的身份请求头绑在一起。

---

## 运维速查清单

- [ ] 正在运行的安装：确认网络未被阻断；厂商大发布后可选执行
      `/model refresh`。
- [ ] 离线 / CI 封闭环境：设置 `CODEWHALE_DISABLE_MODELS_DEV_FETCH=1`，
      或把 `CODEWHALE_MODELS_DEV_PATH` 指向测试夹具。
- [ ] 发布前：对内置种子跑 `snapshot --check`；扫一眼 `PROVIDERS.md`
      里已知的漂移。
- [ ] Models.dev 新增了某个你默认发布的主要系列之后：把种子 PR 和
      默认模型决策分开考虑。
- [ ] 刷新 Models.dev 时，绝不把 API 密钥粘进目录资产或自动化脚本的环境变量里。

---

## 议题 / 设计锚点

- 实时 Models.dev 层：#4187
- 内置种子降级（不再与实时数据争夺权威）：#4188
- 目录自动化脚本（校验 / 试运行）：#4117
- 更细的元数据清单与漂移列表：`codewhale-ops` 仓库
