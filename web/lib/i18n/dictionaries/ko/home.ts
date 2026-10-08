import type { HomeDict } from "../types";

/**
 * Korean home dictionary — native copy for the whale-road landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale: 어떤 모델과도 쓸 수 있는 오픈소스 코딩 에이전트",
  metaDescription:
    "Codewhale은 터미널용 오픈소스 코딩 에이전트입니다. 직접 선택한 호스팅형 또는 로컬 모델로 프로젝트를 읽고 파일을 편집하며 테스트를 실행합니다.",
  heroTitle: "어떤 모델과도 쓸 수 있는 오픈소스 코딩 에이전트",
  heroIntro:
    "{brand}은 터미널에서 프로젝트를 읽고 파일을 편집하며 테스트를 실행합니다. 호스팅형 또는 로컬 모델을 연결하고, 어떤 작업에 내 승인이 필요한지 선택하세요.",
  getCodewhale: "Codewhale 설치",
  heroInstallAria: "설치 명령",
  exploreProduct: "작동 방식 보기",
  shotPreview: "터미널 미리보기",
  shotBuild: "v{version} 개발 빌드",
  screenshotAlt:
    "Codewhale v{version} 개발 빌드: 고래 마크, 새 세션, 메시지 입력창, Ask 권한, Work 모드와 모델 상태. 격리된 터미널의 실제 출력을 렌더링했습니다.",
  latestRelease: "최신 릴리스 {tag}",
  releaseUnavailable: "릴리스 상태를 확인할 수 없음",
  currentSource: "소스",
  sourceCandidate: "미공개",
  publishedRelease: "공개됨",
  figcaptionSourceCandidate: "미공개",
  chapterTerminal: "당신의 터미널",
  chapterTerminalTitle: "실행되는 편집과 명령을 하나씩 확인하세요",
  gainHeading: "작업은 맡기고 제어는 직접 유지하세요",
  gainLede: "버그 수정, 모듈 설명, 반복 작업 자동화처럼 원하는 결과를 요청하세요. 에이전트 하나로 시작하고, 작업이 커지면 에이전트를 추가하세요.",
  gain: [
    [
      "코드를 변경하고 확인하세요",
      "에이전트가 프로젝트를 살펴보고 파일을 편집하며 테스트를 실행합니다. 작업이 진행되는 동안 각 편집과 명령 결과를 확인할 수 있습니다."
    ],
    [
      "반복 작업을 자동화하세요",
      "스크립트와 CI에서 codewhale exec를 실행하세요. Fleet을 사용하면 큰 작업을 여러 에이전트에게 나눌 수 있습니다."
    ],
    [
      "제어권을 유지하세요",
      "작업을 시작하기 전에 권한을 설정하고, 승인 요청에 응답하며, 언제든지 작업을 중지할 수 있습니다. /receipts를 실행하면 세션의 모든 파일, 명령, 승인이 나열됩니다."
    ]
  ],
  chapterModels: "당신의 모델",
  modelsHeading: "작업마다 모델을 선택하세요",
  modelsBody:
    "세션마다 기본 제공 제공업체, OpenAI 호환 엔드포인트, 또는 로컬 모델을 선택하세요. 모델 연결은 Codewhale 계정과 별개로 유지됩니다.",
  modelsFacts: [
    ["호스팅", "codewhale auth set --provider <id>으로 저장한 내 API 키"],
    ["게이트웨이", "하나의 엔드포인트로 여러 모델 사용, 제공업체는 여전히 내가 선택"],
    ["로컬", "localhost의 vLLM, SGLang 또는 Ollama, 보통 키 불필요"],
  ],
  modelsLink: "모델과 제공업체 둘러보기",
  startHeading: "설치하고, 모델을 연결하고, 작업을 실행하세요",
  startLede: "프로젝트 폴더에서 세 단계로 첫 작업을 실행하세요. 여러 에이전트가 필요한 작업이라면 나중에 Fleet을 추가하세요.",
  startGuideLink: "시작 가이드 따라 하기",
  startVocabularyLink: "제품 용어 보기",
  chapterAvailability: "실행 환경",
  availabilityHeading: "지금 터미널에서 사용하세요",
  availabilityLede: "터미널, 로컬 브라우저 클라이언트, 커뮤니티 CodeWhale GUI는 지금 사용할 수 있습니다. 데스크톱 앱과 새로 만드는 호스팅 웹 앱은 개발 중이며 같은 세션 모델을 공유합니다.",
  availability: [
    [
      "터미널과 로컬 브라우저",
      "출시됨",
      "Linux, macOS, Windows에 설치한 뒤 codewhale을 실행하거나, 로컬 브라우저 클라이언트를 쓰려면 codewhale web을 실행하세요. npm과 Cargo로도 설치할 수 있으며, Android의 Termux 버전은 미리보기입니다."
    ],
    [
      "CodeWhale GUI (VS Code)",
      "사용 가능",
      "커뮤니티가 관리하는 별도 프로젝트입니다. 같은 Codewhale Runtime 위에서 VS Code 사이드바로 대화, 스레드, 파일 변경을 다룹니다. VS Code Marketplace에서 설치하세요.",
      "https://marketplace.visualstudio.com/items?itemName=HengQuWorld.brotherwhale-vscode"
    ],
    [
      "호스팅 웹 앱",
      "개발 미리보기",
      "데스크톱 앱에 맞춰 다시 만드는 중입니다. 지금은 로그인한 뒤 실행 중인 터미널 세션에서 /rc를 입력하면 웹에서 이어서 작업할 수 있습니다. 호스팅 작업 실행은 아직 검증 중입니다."
    ],
    [
      "데스크톱",
      "개발 빌드",
      "Codewhale의 주 클라이언트가 되어 가는 네이티브 앱으로, 폴더, 대화, 모델 연결을 하나의 창에서 다룹니다. 아직 공개 다운로드는 없습니다."
    ],
    [
      "클라우드 컴퓨터",
      "개발 중",
      "작업을 실행하는 호스팅 컴퓨터."
    ]
  ],
  availabilityNote: "터미널, 로컬 브라우저, GUI는 Codewhale 계정이 필요 없습니다. 호스팅 웹과 데스크톱은 계정을 사용하지만, 계정이 모델 연결을 대신하지는 않습니다. 내 키로 사용한 요금은 제공업체가 청구합니다.",
  accountLink: "계정 만들기",
  surfacesHeading: "에이전트가 다룰 수 있는 범위를 넓히세요",
  surfaces: [
    ["파일과 명령", "설정한 권한 범위 안에서 프로젝트를 읽고 파일을 편집하며 테스트를 실행하고 출력을 확인합니다."],
    ["플러그인과 MCP", "더 많은 도구와 서비스를 연결합니다. 각 플러그인은 직접 검토하고 활성화하기 전까지 꺼져 있습니다."],
    ["Computer Use · 미리보기", "에이전트가 다른 앱을 보고 조작할 수 있게 하는 플러그인입니다. 직접 활성화하고, 플러그인이 요청하는 시스템 권한을 부여합니다."],
    ["저장된 세션", "대화와 도구 결과를 함께 보관하고, 처음부터 다시 시작하지 않고 이어서 작업합니다. 로컬 브라우저는 내 컴퓨터의 같은 세션을 엽니다."],
    ["Fleet", "서로 다른 모델과 역할을 가진 에이전트에게 작업의 각 부분을 맡기고 진행 상황을 확인합니다."],
  ],
  runtimeLink: "모든 연동 기능 보기",
  installBandHeading: "macOS 또는 Linux에 설치하세요",
  copy: "복사",
  copied: "복사됨 ✓",
  binaries: "바이너리",
  chinaMirrors: "중국 미러",
  installGuideLink: "설치 가이드 읽기",
  communityHeading: "Codewhale 개발에 함께 참여하세요",
  communityBody: "GitHub에서 버그를 보고하거나 기능을 제안하거나 첫 풀 리퀘스트를 보내세요. 작고 테스트를 거친 수정을 환영합니다.",
  communityLinksAria: "커뮤니티 링크",
  contribute: "풀 리퀘스트 보내기",
};
