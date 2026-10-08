# codewhale 运维手册（Operations Runbook）

> 英文原文：[OPERATIONS_RUNBOOK.md](../OPERATIONS_RUNBOOK.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-29。

本手册覆盖本地 CLI/TUI 运行时（runtime）的实用调试与事故响应。

## 快速分诊

1. 确认二进制与配置：
   - `cargo run -- --version`
   - `cat ~/.codewhale/config.toml`（或检查已配置的 profile）
2. 打开详细日志：
   - `RUST_LOG=codewhale_tui=debug cargo run`
   - HTTP 重试／重连：`RUST_LOG=codewhale_tui::client=debug cargo run`
3. 抓取当前状态：
   - `ls ~/.codewhale/sessions`
   - `ls ~/.codewhale/sessions/checkpoints`
   - `ls ~/.codewhale/tasks`

## 事故：回合（turn）挂起或流停止

症状：
- TUI 一直停在加载状态
- 智能体输出不完整且没有结束

检查：
1. 查看重试／健康日志（`codewhale_tui::client`）
2. 验证端点连通性：
   - `curl -sS https://api.deepseek.com/beta/models -H "Authorization: Bearer $DEEPSEEK_API_KEY"`
3. 确认工具输出里没有本地沙箱（sandbox）／权限死锁

处置：
1. 如果有一个前台 shell 命令正在运行，按 `Ctrl+B` 把它移到后台（回合继续运行，该命令会变成 `/jobs` 下的后台作业）；如果你要取消这个回合，就改用 `Ctrl+C`。
2. 如果该命令是在后台启动的，请让智能体用 `Bash` 加上 `action: "cancel"` 和返回的进程 id 来取消。
3. 当你要停掉请求本身时，用 `Esc` 或 `Ctrl+C` 中断当前回合。
4. 重试提示词（prompt）；如果仍然失败，重启 TUI。
5. 重启后，确认之前排队中／在途的运行时回合显示为已中断，而不是仍处于运行状态。

## 事故：网络中断／离线行为

预期行为：
- 离线模式生效期间，新的提示词会排入队列
- 队列状态按会话（session）持久化到
  `~/.codewhale/sessions/checkpoints/<session-id>.offline_queue.json`；旧的全局
  `offline_queue.json` 会在升级时被采纳一次

检查：
1. 在 TUI 中打开队列：`/queue list`
2. 确认持久化的队列文件存在，且时间戳在更新

处置：
1. 恢复连通性
2. 重新发送排队的条目（从 `/queue edit <n>` + Enter，或走正常的输入流程）
3. 确认队列为空时队列文件会被清除

## 事故：需要崩溃恢复

预期行为：
- 每个会话都会把检查点（checkpoint）写到
  `~/.codewhale/sessions/checkpoints/<session-id>.json`；旧的 `latest.json`
  仍会被读取用于恢复，但不再写入
- 除非提供 `--resume`/`--continue`，启动时会开一个新的会话

处置：
1. 用 `codewhale --resume <id>` 显式恢复先前的工作（别名
   `codewhale resume <id>`；`codewhale --continue` 会恢复该工作区里最新的
   已中断检查点），或在 TUI 里按 `Ctrl+R`
2. 如果需要检查检查点内容，就查看 `checkpoints/<session-id>.json`（或残留的旧
   `latest.json`）里的 schema 不匹配／细节
3. 如果 schema 比二进制支持的更新，就升级二进制，或删除过期的检查点

## 事故：持久化状态的 schema 错误

症状：
- 类似 `schema vX is newer than supported vY` 的错误

受影响的存储：
- 会话（`~/.codewhale/sessions/*.json`）
- 运行时线程／回合／条目记录
- 任务（`~/.codewhale/tasks/tasks/*.json`）

处置：
1. 确认二进制版本与迁移预期
2. 在编辑之前备份状态目录
3. 二选一：
   - 换用更新且兼容的二进制运行，或
   - 归档不兼容的记录并重新生成状态

## 事故：MCP／工具执行失败

检查：
1. 校验 `~/.codewhale/mcp.json` 的 schema 与服务器命令路径
2. 确认服务器进程可以手动启动
3. 在 TUI 历史／日志里检查沙箱拒绝

处置：
1. 带上所需的审批（approval）重试（只在合适时才用 YOLO）
2. 暂时禁用出问题的 MCP 服务器，把问题隔离开
3. 用 `/mcp` 诊断验证之后再重新启用

## 事后清单

1. 保留日志和相关的状态文件
2. 记录触发条件、影响与缓解措施
3. 增加或更新回归测试（重试／恢复／schema）
4. 如果行为有变化，就更新本手册和架构文档
