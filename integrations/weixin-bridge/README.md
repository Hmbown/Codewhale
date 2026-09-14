# Weixin Bot Bridge

此 bridge 让微信个人账号通过扫码登录控制本地 `codewhale serve --http` runtime。
使用腾讯 iLink Bot 协议（参考 `@tencent-weixin/openclaw-weixin`），
无需公众号注册即可工作。

与现有的 `integrations/wechat-bridge`（公众号客服消息模式）不同，
此 bridge 直接登录**个人微信账号**，通过长轮询 `getUpdates` 收发消息。

## 安全模型

- `codewhale serve --http` 绑定于 `127.0.0.1`。
- `/v1/*` runtime 调用使用 `CODEWHALE_RUNTIME_TOKEN`。
- 微信用户必须加入白名单，除非首次配对时设置 `WEIXIN_ALLOW_UNLISTED=true`。
- 仅支持私聊；暂不支持群聊。
- 工具审批通过文本命令：`/allow <approval_id>` 或 `/deny <approval_id>`。
- bridge 主动向微信服务器发起长轮询请求，无需公网端口。

## Quick Start

使用两个终端。在第一个终端启动本地 runtime：

```bash
export CODEWHALE_RUNTIME_TOKEN="$(openssl rand -hex 32)"
codewhale serve --http --host 127.0.0.1 --port 7878 --auth-token "$CODEWHALE_RUNTIME_TOKEN"
```

在第二个终端启动 bridge：

```bash
cd integrations/weixin-bridge
export CODEWHALE_RUNTIME_TOKEN="<与上面相同的 token>"
export WEIXIN_ALLOW_UNLISTED=true
npm start
```

### 单终端启动

如果不想开两个终端、也不想手动传递 token，用 `npm run bridge`：

```bash
cd integrations/weixin-bridge
npm run bridge
```

它会自动生成一个 `CODEWHALE_RUNTIME_TOKEN`，在后台启动 `codewhale serve --http`，
等 `/health` 返回正常后再以前台方式启动 bridge，两者共用同一个 token。
按 `Ctrl-C` 会同时停止 bridge 和 runtime。

可用环境变量覆盖默认值：

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `CODEWHALE_RUNTIME_PORT` | `7878` | runtime 监听端口 |
| `CODEWHALE_RUNTIME_TOKEN` | 自动生成 | 复用已有 token 时设置 |
| `WEIXIN_ALLOW_UNLISTED` | `true` | 首次配对模式 |
| `WEIXIN_STATE_DIR` | `./.state` | 状态目录（单终端模式默认写入仓库内，避免 `/var/lib` 权限问题） |

例如换端口并复用已有 token：

```bash
CODEWHALE_RUNTIME_PORT=7999 CODEWHALE_RUNTIME_TOKEN="$MY_TOKEN" npm run bridge
```

首次启动会用文本二维码直接在终端打印登录二维码，用微信扫码即可：

```text
请用微信扫描以下二维码登录：
  █▀▀▀▀▀█ █  ▄▄█▀▀▀ ▄▄█▄▀▀▀▀ ▀▄ █▀▀▀▀▀█
  █ ███ █ ██▄ █▄▀█▀▀▄▀▄▀ █▄ █ ▄ █ ███ █
  ...
  ▀▀▀▀▀▀▀ ▀▀▀▀ ▀▀ ▀▀▀▀ ▀  ▀  ▀ ▀▀▀▀ ▀▀▀
```

二维码下方同时打印原始 URL，便于在二维码显示异常（字体缺字、终端不支持半块字符）
时手动打开。二维码在本地生成，不会经由第三方服务。

登录凭证（`bot_token`）会自动保存到 `WEIXIN_STATE_DIR`，后续启动无需重新扫码。
线程映射与长轮询游标也写入同一目录，该目录会在启动时自动创建并先做一次可写性
探测 —— 不可写时 bridge 会立即报错退出，而不是等到你发消息后才失败。
扫码授权窗口为 5 分钟，超时需重启 bridge 重新获取。

### 在微信中验证

登录成功后，**在微信里给这个 bot 发一条 `/status`**。收到回复即表示链路已打通
（bridge 已连上微信、runtime 也可达）。没有收到回复时不要继续后面的步骤，
先按终端输出的报错排查。

回复内容有两种：

- 已授权（`WEIXIN_ALLOW_UNLISTED=true`，或你已在白名单里）→ 返回 runtime 与工作区状态。
- 未授权 → 返回拒绝消息，其中**带有你的 `user_id`**：

  ```text
  This WeChat user is not in WEIXIN_CHAT_ALLOWLIST.
  user_id=<你的 user_id>

  For first pairing, add this user_id to WEIXIN_CHAT_ALLOWLIST,
  or temporarily set WEIXIN_ALLOW_UNLISTED=true.
  ```

拿到 `user_id` 后写入白名单并关闭配对模式：

```bash
export WEIXIN_CHAT_ALLOWLIST="<上一步返回的 user_id>"
export WEIXIN_ALLOW_UNLISTED=false
npm start
```

之后不带 `/` 前缀的任意文本都会作为 prompt 发给 runtime，回复会发回微信。

> 注意：同一条微信消息只会被处理一次（bridge 按 `user_id:message_id` 去重）。
> 重新测试时请发送**新消息**，重发旧消息不会有任何反应。

注意：bridge **不读取 `.env` 文件**，也不读取 `/etc/codewhale/*.env`（那些是给 systemd
`EnvironmentFile=` 用的）。手动运行时环境变量必须通过 `export` 传入，或使用
`node --env-file=.env src/index.mjs`。

## 设置

systemd 部署时，把环境变量写入 env 文件并由单元引用：

```bash
cd integrations/weixin-bridge
npm install --omit=dev
cp .env.example /etc/codewhale/weixin-bridge.env
sudoedit /etc/codewhale/weixin-bridge.env
node src/index.mjs
```

首次启动时会显示一个二维码，用微信扫描以完成登录授权。
登录凭证会自动保存，后续启动无需重新扫码。

## 命令

- `/status`
- `/threads`
- `/new`
- `/resume <thread_id>`
- `/model <name|default>`
- `/interrupt`
- `/compact`
- `/allow <approval_id> [remember]`
- `/deny <approval_id>`

其他所有内容均作为 Codewhale 提示发送。

## 首次配对

1. 设置 `WEIXIN_ALLOW_UNLISTED=true` 启动 bridge。
2. 扫码登录后，在微信中发送 `/status`。
3. Bridge 会将你的 `user_id` 返回给你（若白名单为空则显示在拒绝消息中）。
4. 将 `user_id` 加入 `WEIXIN_CHAT_ALLOWLIST`。
5. 将 `WEIXIN_ALLOW_UNLISTED` 改回 `false` 并重启 bridge。

## 环境变量

| 变量 | 必填 | 说明 |
|------|------|------|
| `CODEWHALE_RUNTIME_URL` | 否 | Runtime HTTP 地址（默认 `http://127.0.0.1:7878`） |
| `CODEWHALE_RUNTIME_TOKEN` | **是** | Runtime Bearer 令牌 |
| `CODEWHALE_WORKSPACE` | 否 | 工作区路径（默认 cwd） |
| `CODEWHALE_MODEL` | 否 | 模型名称（默认 `auto`） |
| `CODEWHALE_MODE` | 否 | 运行模式（默认 `agent`） |
| `WEIXIN_CHAT_ALLOWLIST` | 否 | 逗号分隔的允许用户 ID |
| `WEIXIN_ALLOW_UNLISTED` | 否 | 首次配对模式（默认 `false`） |
| `WEIXIN_STATE_DIR` | 否 | 状态持久化目录（默认 `/var/lib/codewhale-weixin-bot-bridge`） |
| `WEIXIN_THREAD_MAP_PATH` | 否 | 线程映射文件路径（默认 `<WEIXIN_STATE_DIR>/thread-map.json`） |
| `WEIXIN_MAX_REPLY_CHARS` | 否 | 单条回复最大字符数（默认 `3500`） |
| `CODEWHALE_TURN_TIMEOUT_MS` | 否 | Turn 超时（默认 `900000`） |
| `WEIXIN_LONGPOLL_TIMEOUT_MS` | 否 | 长轮询超时（默认 `35000`） |

旧的 `WEXIN_*`（拼写错误）变量名仍作为已弃用别名被识别，启动时会打印一次弃用警告。

## 架构

```
微信客户端 ──getUpdates 长轮询──▶ Weixin Bot Bridge ──HTTP──▶ codewhale serve --http
                  ◀──sendMessage──                                  (127.0.0.1:7878)
```

Bridge 通过扫码获取 `bot_token`，然后长轮询 `POST /ilink/bot/getupdates`
以接收消息，并通过 `POST /ilink/bot/sendmessage` 发送回复。
所有消息均带有 `context_token` 以维持会话上下文。

## 与 wechat-bridge 的区别

| 特性 | wechat-bridge | weixin-bot-bridge |
|------|---------------|-------------------|
| 账号类型 | 微信公众号 | 个人微信 |
| 登录方式 | App ID + Secret 配置 | 扫码登录 |
| 消息协议 | 公众号回调 + 客服消息 | iLink Bot 长轮询 + sendMessage |
| 公网需求 | 需要（回调 URL） | 不需要 |
| 消息类型 | 仅文本 | 文本/图片/语音/视频/文件（MVP仅文本） |
