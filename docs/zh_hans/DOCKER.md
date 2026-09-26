# Docker

> 英文原文：[DOCKER.md](../DOCKER.md)。
> 最后与英文同步日期（last synced with English revision）：2026-09-26。

Codewhale 每次发布都会把一个多架构的 Linux 镜像推到 GitHub Container
Registry。

```bash
docker pull ghcr.io/hmbown/codewhale:latest
```

## 快速开始

用 Docker 管理的数据卷运行已发布的镜像：

```bash
docker volume create codewhale-home

docker run --rm -it \
  -e DEEPSEEK_API_KEY="$DEEPSEEK_API_KEY" \
  -v codewhale-home:/home/codewhale/.codewhale \
  -v "$PWD:/workspace" \
  -w /workspace \
  ghcr.io/hmbown/codewhale:latest
```

想获得可复现的安装，请用固定的发布标签：

```bash
docker run --rm -it \
  -e DEEPSEEK_API_KEY="$DEEPSEEK_API_KEY" \
  -v codewhale-home:/home/codewhale/.codewhale \
  -v "$PWD:/workspace" \
  -w /workspace \
  ghcr.io/hmbown/codewhale:vX.Y.Z
```

把 `vX.Y.Z` 换成
[GitHub Releases](https://github.com/Hmbown/CodeWhale/releases) 里的标签。

## 默认镜像约定

`ghcr.io/hmbown/codewhale:latest` 和各个 semver 标签都是保守的运行时镜像：

- 容器以非 root 的 `codewhale` 用户运行，UID/GID 为 `1000:1000`
- 镜像不授予免密 `sudo`
- 镜像的用途是让 Codewhale 跑在挂载进来的工作区上，而不是在运行时改动基础操作系统
- 用户状态应放在挂载到 `/home/codewhale/.codewhale` 的数据卷里

这个默认值是有意为之。想保持最小信任边界，就继续用它。如果某个项目需要
`apt-get`、编译工具链、Node/Python 包管理器、自定义 CA 证书，或者 Docker 里
其他类似主机的环境，请另外构建一个显式的工具箱镜像，不要改动默认镜像的约定。

## 可选：工具箱/自定义镜像

仓库里有一个示例
[`docs/examples/Dockerfile.toolbox`](../examples/Dockerfile.toolbox)，它在官方
镜像上加了免密 `sudo` 和常用开发软件包。想要可复现的项目环境时，就用固定的
Codewhale 标签构建它：

```bash
docker build -f docs/examples/Dockerfile.toolbox \
  --build-arg CODEWHALE_IMAGE=ghcr.io/hmbown/codewhale:vX.Y.Z \
  --build-arg TOOLBOX_PACKAGES="git openssh-client curl build-essential pkg-config python3 python3-pip nodejs npm" \
  -t codewhale-toolbox:my-project .
```

`latest` 只用于一次性测试。共享项目请把 `CODEWHALE_IMAGE` 固定住；新增软件包
要走审阅，就像改动其他开发环境一样。

用同一套工作区和状态挂载运行工具箱镜像：

```bash
docker volume create codewhale-my-project-home

docker run --rm -it \
  -e DEEPSEEK_API_KEY="$DEEPSEEK_API_KEY" \
  -v codewhale-my-project-home:/home/codewhale/.codewhale \
  -v "$PWD:/workspace" \
  -w /workspace \
  codewhale-toolbox:my-project
```

在这个可选镜像里，Codewhale 可以用 `sudo apt-get update`、
`sudo apt-get install -y <package>` 这类命令。想要可重复的容器，就把这些包固化进
工具箱 Dockerfile，别让长期运行的容器慢慢漂移。

不要把 API 密钥、SSH 私钥或其他密钥固化进自定义镜像。API 密钥在运行时传入；
SSH 材料要显式挂载，最好只读，并且只给真正需要它的项目。

### Compose 工具箱模板

如果你更想要一个可重复的 `docker compose` 入口，请用
[`docs/examples/compose.toolbox.yml`](../examples/compose.toolbox.yml)。它会从
[`docs/examples/Dockerfile.toolbox`](../examples/Dockerfile.toolbox) 构建工具箱
镜像，并把项目状态卷显式写出来：

```bash
CODEWHALE_IMAGE=ghcr.io/hmbown/codewhale:vX.Y.Z \
CODEWHALE_TOOLBOX_IMAGE=codewhale-toolbox:my-project \
CODEWHALE_HOME_VOLUME=codewhale-my-project-home \
CODEWHALE_WORKSPACE="$PWD" \
docker compose -f docs/examples/compose.toolbox.yml run --rm codewhale
```

每个需要独立工具链或独立 `.codewhale` 状态的项目，都该用不同的
`CODEWHALE_TOOLBOX_IMAGE` 和 `CODEWHALE_HOME_VOLUME`。Compose 文件里还给出了
SSH 材料和本地 CA 证书的可选只读挂载；除非项目确实需要，否则让它们保持注释状态。

## 多个独立项目

每个项目用一个具名状态卷，这样会话、配置、技能、记忆和离线队列就不会跨工作区
互相污染：

```bash
project="$(basename "$PWD")"
image="codewhale-toolbox:${project}"
docker volume create "codewhale-${project}-home"

docker run --rm -it \
  --name "codewhale-${project}" \
  -e DEEPSEEK_API_KEY="$DEEPSEEK_API_KEY" \
  -v "codewhale-${project}-home:/home/codewhale/.codewhale" \
  -v "$PWD:/workspace" \
  -w /workspace \
  "$image"
```

工具链不同的项目，就构建不同的工具箱标签，例如
`codewhale-toolbox:frontend` 和 `codewhale-toolbox:backend`。issue #2217 里
讨论的独立启动器可以建立在这套约定之上，但它有意留在核心 Docker 镜像之外。

## 项目引导脚本

Codewhale 不会自动执行 `.codewhale/setup.sh`，也不会自动执行旧版的
`.deepseek/setup.sh`。如果你留着这类文件当本地的项目配方，就显式运行它。
团队共用的环境，优先用提交进仓库的项目脚本或工具箱 Dockerfile，这样环境
可以被审阅和重建。

例如，在启动 Codewhale 之前运行一个已提交的引导脚本：

```bash
docker run --rm -it \
  -e DEEPSEEK_API_KEY="$DEEPSEEK_API_KEY" \
  -v codewhale-my-project-home:/home/codewhale/.codewhale \
  -v "$PWD:/workspace" \
  -w /workspace \
  --entrypoint bash \
  codewhale-toolbox:my-project \
  -lc './scripts/bootstrap-dev.sh && exec codewhale'
```

需要 `sudo` 的引导脚本请用工具箱镜像。默认镜像不会提权。

## 自定义 CA 证书与代理

企业代理、dev-sidecar、自签名内部服务这类场景，最好把受信任的 CA 证书固化进
自定义工具箱镜像：

```dockerfile
USER root
COPY docker/certs/*.crt /usr/local/share/ca-certificates/
RUN update-ca-certificates
USER codewhale
```

复制到 `/usr/local/share/ca-certificates/` 的文件必须用 `.crt` 扩展名。私有 CA
材料不要放进公开镜像。

只在本地运行时，把证书以只读方式挂载，并在容器启动时更新信任库：

```bash
docker run --rm -it \
  -e DEEPSEEK_API_KEY="$DEEPSEEK_API_KEY" \
  -v codewhale-my-project-home:/home/codewhale/.codewhale \
  -v "$PWD:/workspace" \
  -v "$PWD/docker/certs:/usr/local/share/ca-certificates/local:ro" \
  -w /workspace \
  --entrypoint bash \
  codewhale-toolbox:my-project \
  -lc 'sudo update-ca-certificates && exec codewhale'
```

这套 CA 流程需要那个可选的工具箱镜像，因为默认镜像里没有免密 `sudo`。

## 本地构建

在本地检出目录里构建镜像：

```bash
docker build -t codewhale .
```

然后用同一个由 Docker 管理的数据卷运行它：

```bash
docker run --rm -it \
  -e DEEPSEEK_API_KEY="$DEEPSEEK_API_KEY" \
  -v codewhale-home:/home/codewhale/.codewhale \
  -v "$PWD:/workspace" \
  -w /workspace \
  codewhale
```

没有配置 Docker Hub 发布；GHCR 是受支持的预构建镜像仓库。

## 环境变量

| 变量              | 是否必需 | 说明                                      |
|-----------------------|----------|--------------------------------------------------|
| `DEEPSEEK_API_KEY`    | 是      | DeepSeek API 密钥                                 |
| `DEEPSEEK_BASE_URL`   | 否       | 自定义 API base URL（例如 `https://api.deepseek.com`） |
| `DEEPSEEK_NO_COLOR`   | 否       | 设为 `1` 可关闭终端彩色输出     |

## 数据卷

挂载 `/home/codewhale/.codewhale`，让会话、配置、技能、记忆和离线队列在容器
重启后依然保留。镜像里也保留 `/home/codewhale/.deepseek` 以兼容旧版本。
Docker 管理的具名卷是最稳妥的默认选择，因为 Docker 创建它时，会把属主设成
容器能写入的那个用户：

```bash
-v codewhale-home:/home/codewhale/.codewhale
```

不挂载这个卷，容器每次启动都是全新的。

如果改为绑定挂载主机上已有的目录，镜像会以非 root 的 `codewhale` 用户运行，
UID/GID 为 `1000:1000`。挂载的目录必须对该用户可写，否则启动时创建
`.codewhale/tasks` 下的运行时目录可能会失败。在 Linux 主机上，要么用上面的具名卷，
要么显式准备好绑定挂载：

```bash
mkdir -p ~/.codewhale
sudo chown -R 1000:1000 ~/.codewhale

docker run --rm -it \
  -e DEEPSEEK_API_KEY="$DEEPSEEK_API_KEY" \
  -v ~/.codewhale:/home/codewhale/.codewhale \
  ghcr.io/hmbown/codewhale:latest
```

这条 `chown` 会改变主机上 `~/.codewhale` 目录的属主。如果不想让容器里的 UID
拥有你的本地配置，就跳过它，改用具名卷。

## 非交互 / 流水线用法

stdin 不是 TTY 时，`codewhale` 会退到调度器的一次性模式（`codewhale -c "…"`）。
把提示词用 stdin 传进去：

```bash
echo "Explain the Cargo.toml in structured English." | \
  docker run --rm -i -e DEEPSEEK_API_KEY ghcr.io/hmbown/codewhale:latest
```

## 在本地构建

```bash
# Single platform (your host architecture)
docker build -t codewhale .

# Multi-platform (requires a builder with emulation)
docker buildx create --use
docker buildx build --platform linux/amd64,linux/arm64 -t codewhale .
```

## 开发容器

仓库里有一个给 VS Code / GitHub Codespaces 用的
[`.devcontainer/devcontainer.json`](../../.devcontainer/devcontainer.json) 配置。
它会构建一个专用的开发镜像，里面有 Rust 工具链、Git、`pkg-config`，以及工作区
需要的 DBus 开发头文件。首次打开会运行 `cargo build --locked`，并安装
rust-analyzer 和其他编辑器扩展。

源码检出仍从主机挂载。Codewhale 状态和 Cargo 构建产物改用 Docker 具名卷。
这样一来，就算 VS Code 提供不了 POSIX 风格的 `HOME` 变量（Windows 上尤其
如此），这套配置照样能用，构建也不会经由 Windows 绑定挂载写入数千个小
文件。改动 Dev Container 配置后要重建容器。

## 发布状态

Docker 镜像发布是发布门禁的一部分。镜像会以 semver 标签加 `latest` 的形式
发布到 GHCR，覆盖 `linux/amd64` 和 `linux/arm64`。
