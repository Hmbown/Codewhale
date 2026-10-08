# Ubuntu Lighthouse bootstrap

Choose the trusted networks that may reach SSH before running the bootstrap:

```bash
sudo SSH_ALLOWED_CIDRS='203.0.113.4/32,2001:db8::/64' \
  bash scripts/tencent-lighthouse/bootstrap-ubuntu.sh
```

Replace the example networks with your own. Every IPv4 /8–/32 or IPv6 /16–/128
CIDR is validated before packages, users, files, or firewall rules change.
The script adds the narrow rules before removing the broad OpenSSH rule.
Keep the Lighthouse console firewall consistent with this policy.

If a public SSH endpoint is deliberately required, explicitly pass
`SSH_ALLOW_ANY_SOURCE=1` instead. Omitting both settings stops the bootstrap.
The final instructions install the unified CLI, which includes the TUI and
runtime server, using Rust 1.89 or newer.

## 简体中文

运行上面的命令前，请把示例网段替换为允许访问 SSH 的可信来源。脚本接受逗号或
空格分隔的 IPv4 /8–/32 和 IPv6 /16–/128 CIDR，并在安装软件包、创建用户、修改
文件或防火墙规则之前验证整个列表。它先添加受限规则，再删除对所有来源开放的
OpenSSH 规则。Lighthouse 控制台防火墙也应采用一致的策略。

确实需要公开 SSH 时，必须显式设置 `SSH_ALLOW_ANY_SOURCE=1`。两个设置都未
提供时，脚本会停止。最后的安装步骤使用 Rust 1.89 或更新版本，安装包含 TUI 和
运行时服务的统一 CLI。
