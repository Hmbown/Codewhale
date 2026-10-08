# 工作间（Workroom）架构

> 英文原文：[WORKROOM_ARCHITECTURE.md](../WORKROOM_ARCHITECTURE.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-29。

## 目的

工作间是 Codewhale 面向聊天的抽象，用来表示持久、可寻址的智能体（agent）工作线程。
它们位于 Runtime API 的临时线程模型与面向用户的表面（TUI、移动端、聊天桥）之间。

这是一份草案性的 v0.9 架构说明。在 v0.8.62 中，只存在协议数据类型和链接解析器。
Runtime 端点、持久状态、移动端渲染，以及模型可见的链接解析，都是计划中的后续工作。

## 组件图

```
┌─────────────────────────────────────────────────────┐
│ User surfaces                                        │
│  ┌──────┐  ┌─────────┐  ┌──────────┐               │
│  │ TUI  │  │ Mobile  │  │ Bridges  │               │
│  └──┬───┘  └────┬────┘  └────┬─────┘               │
│     │           │            │                       │
│     └───────────┼────────────┘                       │
│                 │  future HTTP + workroom links      │
├─────────────────┼───────────────────────────────────┤
│ Runtime API     │                                    │
│  ┌──────────────┴──────────────┐                    │
│  │ Planned workroom endpoints  │                    │
│  │  GET /workrooms             │                    │
│  │  GET /workroom/:id/threads  │                    │
│  │  GET /workroom/resolve      │                    │
│  └──────────────┬─────────────┘                    │
│                 │                                    │
│  ┌──────────────┴─────────────┐                    │
│  │ Existing endpoints         │                    │
│  │  /thread /app /prompt ...  │                    │
│  └────────────────────────────┘                    │
└─────────────────────────────────────────────────────┘
```

## 数据流

1. **创建。** 未来的工作间在启动一个带工作间上下文（标题、工作区、外部引用）的线程时创建。
   工作间 id 是稳定的，可以作为 `codewhale://workroom/...` 链接分享。

2. **事件发布。** 每个智能体动作（工具调用、审批、失败）都会作为 `WorkroomEvent`
   记录在工作间的事件日志里。事件带有 `AgentAttribution` 元数据，
   追踪是哪个提供商（provider）、模型和智能体产生了它们。

3. **链接解析。** 当 `codewhale://workroom/...` 链接出现在聊天表面时，
   未来的 `resolve_workroom_link` 工具（或 API 端点）会解析它并返回有作用域的上下文：
   线程元数据、外部引用和最近的事件摘要。调用方模型随后可以决定是否读取完整的线程对话记录（transcript）。

4. **列出。** 未来的 `/workrooms` 端点返回所有可见工作间的摘要
   （id、标题、updated_at、活跃线程数）。各个表面消费它来支撑收件箱/最近活动视图。

## 状态存储

持久化的工作间状态应当与现有 Codewhale 状态放在一起：

```
~/.codewhale/
├── workrooms/
│   ├── wr_abc123.json     # Workroom metadata + event log
│   └── wr_def456.json
├── threads/               # Existing thread state (unchanged)
├── checkpoints/
├── config.toml
└── ...
```

每个 `.json` 文件会包含工作间元数据（`Workroom` 结构体）、一组 `WorkroomThread` 描述符，
以及一组有界的最近 `WorkroomEvent` 记录。这个状态存储尚未实现。

## Crate 职责

| Crate | 职责 |
|---|---|
| `codewhale-protocol` | 类型：`Workroom`、`WorkroomId`、`WorkroomThread`、`WorkroomEvent`、`WorkroomLink`、`ExternalThreadRef`、`AgentAttribution` |
| `codewhale-app-server` | 未来的端点：`GET /workrooms`、`GET /workroom/:id/threads`、`GET /workroom/resolve` |
| `codewhale-tui` | 未来面向模型的链接解析，以及可选的任务面板收件箱 |
| `codewhale-state` | 未来：持久工作间存储（第 2 阶段） |

## 阶段状态

| 阶段 | 功能 | 状态 |
|---|---|---|
| 1 | RFC 设计文档 | ✅ 已完成 |
| 1 | 协议数据类型 | ✅ 已完成（含测试） |
| 1 | App-server 工作间端点 | ⏳ 尚未开始 |
| 1 | `resolve_workroom_link` 工具 | ⏳ 尚未开始 |
| 1 | 安全模型文档 | ✅ 已完成 |
| 1 | 架构文档 | ✅ 已完成 |
| 2 | 持久工作间状态存储 | ⏳ 尚未开始 |
| 2 | 移动端页面的工作间收件箱 | ⏳ 尚未开始 |
| 2 | 聊天桥事件集成 | ⏳ 尚未开始 |
| 2 | TUI 任务面板收件箱 | ⏳ 尚未开始 |
