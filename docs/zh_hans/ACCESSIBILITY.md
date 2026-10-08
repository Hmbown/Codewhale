# 无障碍

> 英文原文：[ACCESSIBILITY.md](../ACCESSIBILITY.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-29。

Codewhale 运行在终端里，所以平台自带的无障碍栈（屏幕阅读器、放大镜、终端级主题）
承担了大部分工作。TUI 提供少量开关，让屏幕阅读器用户和低动效用户降低视觉动效
和信息密度。

## 快速参考

| 开关 | 默认值 | 效果 |
| --- | --- | --- |
| `NO_ANIMATIONS=1` 环境变量 | 未设置 | 启动时强制 `low_motion = true` 和 `fancy_animations = false`。覆盖 `settings.toml` 里保存的任何设置。 |
| `CODEWHALE_ASCII_SAFE=1` 环境变量 | 未设置 | 在终端后端把装饰性 Unicode 和制表符号换成窄 ASCII。标签、焦点、状态和控件仍然可用。 |
| `low_motion` 设置 | `false` | 冻结装饰性和状态动画，但不改变模型文本的送达方式。页脚的水条由 `fancy_animations` 单独控制。 |
| `fancy_animations` 设置 | `true` | 启用表现力强的实时状态装饰。设为 `false` 可让实时回合的装饰保持静止。 |
| `ocean_treatment` 设置 | `ombre` | 选择背景外观：`ombre` 绘制随状态变化的水柱；`flat` 使用普通的主题表面。两者保留相同的状态标记和空闲环境动效；外观与动效设置相互独立。 |
| `status_indicator` 设置 | `cw` | 静态的排版式页眉标记。设为 `dots` 用旧版动画，设为 `off` 隐藏它；`whale` 已废弃，会归一化为 `cw`。 |
| `calm_mode` 设置 | `true` | 默认折叠工具输出的细节，并精简状态消息。屏幕阅读器如果每次重绘都要念一遍，这一项就很有用。 |
| `show_thinking` 设置 | `true` | 设为 `false` 可从 TUI 展示中隐藏模型的 `reasoning_content` 块。规范的会话/回放回执保持不变。 |
| `thinking_default_expanded` 设置 | `false` | 设为 `true` 可让可见的思考块初始展开。空格键仍可折叠或展开选中的块。 |
| `show_tool_details` 设置 | `false` | 设为 `true` 可在行内展开工具调用；两种情况下细节都可按需查看。 |
| `inline_diffs` 设置 | `full` | 用 `summary` 或 `off` 降低行内 File-change 的密度。任何模式下都可用 Alt/Option+V 查看实际应用的证据。 |

## 配色对比度保证

调色板在两处强制 WCAG 对比度下限，代码真正保证的也就这些，不多不少：

* **绘制时**，每个文本单元都会针对它实际渲染所在的表面，把对比度提升到
  4.5:1（终端后端里的 `enforce_cell_contrast`）。框架装饰（边框、块字形）不做钳制；
  自带整套自定义调色板的社区预设（Catppuccin、Tokyo Night、Dracula、Gruvbox、
  Claude、Matrix、Solarized Light、Terminal）不参与绘制时的对比度钳制，
  因为它们的作者已经调过这些配色对。
* **按主题**，一个审计（`theme_contrast_violations`）要求每个可选预设都守住
  同样的下限：正文、soft 和 muted 文本在每个主表面上都达到 4.5:1（包括选中
  和错误表面）；提示文本和弱化文本为 3:1；状态、警告、成功和信息角色为 3:1，
  因为它们本就冗余——每个状态还带一个字形和一个文字标签，颜色从不是唯一的
  通道。diff 的前景/背景对要求 3:1。
* **Terminal**（透明）主题按设计豁免：它绘制 `Color::Reset` 表面和 ANSI 强调色，
  让宿主终端自己的配色透出来。这些颜色归终端所有，无法测量，所以审计跳过它们，
  而不是宣称它们通过了检查（`theme_uses_terminal_owned_surfaces` 把这项豁免写明）。
* **Grayscale** 主题“极简配色、高对比”的标语，代码确实强制执行：它的各层级正文文本
  在每个表面上对比度都超过 4.5:1。
* ASCII 档位（`CODEWHALE_ASCII_SAFE=1`）就算没有装饰字形，也保留标签、焦点和状态，
  所以上面那套不依赖颜色的冗余在最朴素的渲染模式下依然成立。

## 标准环境变量接口

把它们写进 shell 配置文件，让每个会话都生效：

```bash
# Force low-motion + no fancy animations.
export NO_ANIMATIONS=1

# Force the terminal-safe ASCII rendering tier.
export CODEWHALE_ASCII_SAFE=1

# Optional: respect the wider terminal-color convention.
export NO_COLOR=1            # terminal-owned colors; bold/underline remain
```

`NO_COLOR` 非空时会抑制 TUI 里的前景色、背景色和下划线颜色。空值则保持正常的
终端颜色检测。这遵循 [NO_COLOR 约定](https://no-color.org/)，同时保留文本修饰
和选中符号。ASCII 渲染和降低动效是两个独立的选择。

`NO_ANIMATIONS` 接受 `1`、`true`、`yes` 或 `on`（不区分大小写）。其他任何值
（包括 `0`、`false`、空值或未设置）都不会动你保存的设置。

这个覆盖只在启动时应用一次。会话中途改变环境变量没有效果——设置只在下一次
启动时重新读取。

## 用 `/config` 配置

这些开关也能从命令面板里改：

* `/config low_motion on --save`
* `/config fancy_animations off --save`
* `/config calm_mode on --save`
* `/config status_indicator off --save`

这样写入的设置，在新安装上会保存到 `~/.codewhale/settings.toml`。旧版的
`~/.deepseek/settings.toml` 和平台配置目录里的设置则保留下来，作为兼容回退。只要设了
`NO_ANIMATIONS` 环境变量，它在启动时依然优先，所以想让保存的选择生效，
就得取消这个环境变量。

Tilix 和 Terminator 的会话会自动以低动效模式启动，因为这类基于 VTE 的终端
在回合执行期间出现过可见的重绘闪烁。如果你的终端版本渲染正常，启动后仍然
可以覆盖已保存的设置。

## 屏幕阅读器用户的注意事项

* `low_motion` 把空闲重绘循环放慢到每帧约 120ms，并冻结状态标记，但不会合成
  模型文本，也不会给它限流。配合 `calm_mode`，重绘频率足够低，VoiceOver / Orca 的
  播报会跟随模型输出线性推进，而不是每个 tick 都把整屏重念一遍。
* 对话记录（transcript）是纯文本——没有图片，也没有 canvas 渲染——所以任何集成了平台
  无障碍服务的终端（例如 macOS Terminal.app、iTerm2、Ghostty、Windows Terminal）
  都会把渲染后的内容原样透传。
* 如果 `low_motion = true` 时仍有界面元素产生动效，请针对
  [`PRIOR: Screen-reader / accessibility flag`](https://github.com/codewhale-hq/CodeWhale/issues/450)
  提一个 issue，并附上截图或终端录制。

## 相关 issue / 历史

* [#450](https://github.com/codewhale-hq/CodeWhale/issues/450) ——
  记录已有的开关，加入 `NO_ANIMATIONS` 启动覆盖，并撰写本页。
* [#449](https://github.com/codewhale-hq/CodeWhale/issues/449) ——
  页脚状态栏现在使用当前主题的对比配色对，而不再用单独定制的调色板。
