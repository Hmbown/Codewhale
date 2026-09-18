<!-- source: README.md sha256:ebf1c69de0dc -->
# Codewhale

Codewhale은 사용자가 선택한 호스팅 모델이나 로컬 모델로 프로젝트를 읽고, 파일을 편집하고, 명령을 실행하며, 작업 결과를 확인하는 오픈 소스 에이전트입니다. 터미널에서 하나의 작업으로 시작하세요. 더 큰 작업은 서로 다른 모델과 역할을 가진 에이전트에게 나누어 맡길 수 있습니다.

![터미널에서 실행 중인 Codewhale](web/public/codewhale-tui-171acee.png)

*v0.9.12 개발 빌드의 터미널 미리보기입니다.*

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja-JP.md) · [Tiếng Việt](README.vi.md) · [Bahasa Indonesia](README.id.md) · [Español](README.es-419.md) · [Português](README.pt-BR.md) · [Русский](README.ru.md) · [Українська](README.uk.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [繁體中文](README.zh-TW.md) · [हिन्दी](README.hi.md) · [Türkçe](README.tr.md) · [Italiano](README.it.md) · [Polski](README.pl.md) · [العربية](README.ar.md) · [Català](README.ca.md)

[![CI](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml/badge.svg)](https://github.com/Hmbown/CodeWhale/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/codewhale-cli?label=crates.io)](https://crates.io/crates/codewhale-cli)
[![npm](https://img.shields.io/npm/v/codewhale?label=npm)](https://www.npmjs.com/package/codewhale)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/37gfS3ksug)

## 설치

macOS 또는 Linux에 새로 설치할 때는 공식 GitHub 릴리스를 사용하세요.

```bash
curl -fsSL https://codewhale.net/install.sh | sh
"$HOME/.local/bin/codewhale"
```

설치 도구는 공개된 최신 릴리스를 선택합니다. [변경 이력](CHANGELOG.md)에는 다음 릴리스의 미공개 후보 버전도 설명되어 있지만, 해당 릴리스가 공개되기 전에는 그 변경 사항이 공개 다운로드에 포함되지 않습니다.

Windows에서는 [GitHub Releases](https://github.com/Hmbown/CodeWhale/releases/latest)에서 맞는 설치 프로그램이나 아카이브를 받으세요. 기존 직접 설치는 `codewhale update`로 업데이트하고, 확인만 하려면 `codewhale update --check`를 사용하세요. 업데이트 도구는 실행 파일 경로를 표시하며 더 최신인 빌드는 유지합니다. npm과 Cargo는 보조 패키지 설치 방법입니다. 패키지 관리자 설치에서 이전하거나 PATH를 설정하려면 [설치 안내서](docs/INSTALL.md)를 참조하세요.

처음 실행하면 공급자 연결 과정을 안내하며, 오프라인으로 Codewhale을 설정할 수도 있습니다. 모델의 응답을 받으려면 호스팅 모델이나 로컬 모델에 연결해야 합니다. Codewhale은 보조 패키지 설치 경로로 npm과 Cargo를 지원하며, Docker, Nix, Scoop, Android/Termux와 선택적으로 사용할 수 있는 CNB 미러도 지원합니다. 패키지 관리자로 설치한 기존 버전에는 이전 안내가 제공됩니다. [설치 및 PATH 도움말](docs/INSTALL.md)을 참조하세요.

각 셸에서 Tab 자동 완성은 명령 한 줄로 설정할 수 있습니다 — `codewhale completion bash|zsh|fish|powershell|elvish`. [셸 자동 완성](docs/INSTALL.md#8-shell-completions)을 참조하세요.

## 사용법

프로젝트 폴더에서 터미널을 열고 `codewhale`을 실행하세요. `/provider`로 공급자를, `/model`로 모델을 선택한 다음 구체적인 작업을 설명하세요:

```text
Fix the failing tests and explain what changed.
```

TUI를 열지 않고 작업을 실행할 수도 있습니다:

```bash
codewhale exec "fix the failing tests and explain what changed"
```

Codewhale은 저장소를 읽고, 파일을 편집하고, 명령을 실행하고, 결과를 확인하며 목표를 향해 계속 작업할 수 있습니다. 파일을 변경하거나 셸 명령을 실행하지 않고 살펴보려면 `/mode plan`을 사용하고, 변경을 수행하려면 `/mode work`를 사용하세요. `Shift+Tab`을 누르면 Ask, Auto-Review, Full Access를 선택할 수 있습니다. 각 설정이 허용하는 작업은 [모드 및 권한 안내서](docs/MODES.md)에서 확인하세요.

## 터미널, 앱, Computer Use

터미널과 그래픽 클라이언트는 Codewhale Runtime에 연결하며, Runtime이 에이전트와 도구를 실행합니다:

- **터미널:** `codewhale`은 대화형 인터페이스를 열고, `codewhale exec`는 스크립트나 CI 작업에서 태스크를 실행합니다.
- **로컬 브라우저:** `codewhale web`은 같은 Runtime을 사용하는 내장 [로컬 웹 클라이언트](docs/WEB.md)를 엽니다.
- **Codewhale 데스크톱 앱(GPUI):** 네이티브 GPUI 데스크톱 앱이 제품 클라이언트 방향입니다(2026-09-14 결정; 단계 계획은 비공개 codehwhale-gpui 저장소의 docs/TRANSITION.md에 있음). app.codewhale.net의 호스티드 웹 앱은 단계적으로 종료되며, 마케팅 사이트, 로그인, 결제, 법률, 다운로드 페이지는 웹에 영구적으로 유지됩니다. 이용 가능 여부는 [제품 페이지](https://codewhale.net/en/product)에서 확인할 수 있습니다.

**Computer Use는 다른 애플리케이션을 관찰하고 조작하는 도구를 추가합니다.** 이 플러그인은 현재 소스에 포함되어 있습니다. 사용 전에 요청하는 접근 권한을 검토하고 활성화하세요. OS 권한과 플랫폼 요구 사항도 충족해야 합니다. 포함된 [Computer Use 안내서](crates/tui/plugins/computer-use/README.md)와 [플러그인 설정](docs/PLUGINS.md)을 참조하세요.

VS Code에서는 커뮤니티가 관리하는 CodeWhale 확장이 사이드바에서 로컬 Runtime에 연결합니다. [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode)에서 설치하세요. 소스 코드는 [GitHub](https://github.com/HengQuWorld/CodeWhale-VSCode)에 있습니다.

## Codewhale을 선택하는 이유

- **모델을 선택하세요.** 호스팅 공급자에 연결하거나 Ollama, vLLM, SGLang을 통해 로컬 모델을 사용할 수 있습니다. `/provider`로 공급자를 바꾸고 `/model`로 모델을 선택하세요.
- **계속 주도권을 가지세요.** 제안된 작업과 그 결과로 생긴 파일 변경을 확인하세요. 승인 설정은 언제 검토가 필요한지 결정하며, Full Access에서도 반드시 지켜야 하는 정책 경계는 유지됩니다. `/undo`와 `/restore`는 작업 공간의 변경을 복구하는 데 도움이 됩니다.
- **긴 작업도 체계적으로 관리하세요.** 세션을 저장하고, 지속되는 `/goal`을 설정하고, 워크플로 실행 전에 검토하며, 에이전트의 내부 지시가 대화 기록에 섞이지 않도록 여러 에이전트를 조율할 수 있습니다.
- **이미 사용 중인 에이전트를 확장하세요.** MCP 서버와 스킬을 연결하고, 훅을 구성하고, 에이전트 역할을 프로젝트나 개인 설정에 읽기 쉬운 파일로 보관할 수 있습니다.

명령과 키보드 단축키를 보려면 TUI에서 `/help`를 실행하세요.

## 안전

Codewhale은 사용자가 허용한 접근 권한으로 사용자의 컴퓨터에서 실행됩니다. 승인 모드와 저장소 규칙은 에이전트가 할 수 있는 일을 제한하며, 지원되는 환경에서는 선택적 OS 샌드박싱으로 더 강력한 실행 경계를 추가할 수 있습니다. 가격이 알려지지 않은 모델은 무료로 표시하지 않고 미확인 상태로 둡니다.

정확한 정책 적용 순서는 [권한 부여 순서](docs/AUTHORIZATION_ORDER.md)에서, 로컬 설정은 [구성](docs/CONFIGURATION.md)에서 확인하세요.

## 문서

- [공급자와 로컬 모델](docs/PROVIDERS.md)
- [에이전트 팀](docs/FLEET.md)
- [MCP](docs/MCP.md), [훅](docs/HOOKS.md), [구성](docs/CONFIGURATION.md)
- [로컬 웹 클라이언트](docs/WEB.md)
- [전체 문서](docs)
- [저장소 구조 및 기여 가이드](CONTRIBUTING.md#project-structure)

## 커뮤니티 참여

**버그 보고, 기능 제안, pull request를 환영합니다.** Codewhale을 몇 달간 사용했든 처음 사용해 보든 누구나 참여할 수 있습니다. 필요한 공급자가 없거나 워크플로가 불편하거나 터미널 UI가 작업을 방해한다면 [issue를 등록](https://github.com/Hmbown/CodeWhale/issues/new/choose)하거나 [pull request를 보내](CONTRIBUTING.md) 함께 개선해 주세요. 첫 기여도 환영하며, 반영된 작업에는 기여자의 이름을 남깁니다.

[Discord](https://discord.gg/37gfS3ksug)에 참여하거나 WeChat에서 Hunter(`hunterbown`)를 추가한 뒤 Whale Brothers 그룹 참여를 요청하세요.

## 프로젝트 역사

Codewhale은 `deepseek-tui`로 시작했으며 해당 구성 및 세션과의 호환성을 계속 유지합니다. 현재는 특정 공급자에 종속되지 않고 독립적으로 관리되며, 어떤 모델 공급자와도 제휴하지 않습니다.

모든 기여자와 프로젝트의 성장을 도운 오픈 소스 커뮤니티에 감사드립니다. [기여자 기록](docs/CONTRIBUTORS.md)을 확인하세요.

## 라이선스

[MIT](LICENSE). 다른 오픈 소스 프로젝트를 바탕으로 수정한 부분은 [타사 고지](docs/THIRD_PARTY_NOTICES.md)에 기록되어 있습니다.
