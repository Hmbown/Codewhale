import type { HomeDict } from "../types";

/**
 * Korean home dictionary — native copy for the Tidal Folio landing page,
 * in the current direction: your models, more capable together; agents
 * and control on your own machine; availability stated per surface as it
 * is today. Product vocabulary stays literal (Plan / Work / Operate, Ask /
 * Auto-Review / Full Access, Codewhale, TUI, codewhale exec, Fleet).
 */

export const home: HomeDict = {
  metaTitle: "Codewhale — 원하는 모델로 개발하고 작업을 자동화하세요",
  metaDescription:
    "오픈소스 에이전트와 직접 선택한 호스팅형 또는 로컬 AI 모델을 사용해 소프트웨어를 개발하고 파일을 다루며 일상적인 작업을 자동화하세요.",
  heroTitle: "원하는 모델로 개발하고 작업을 자동화하세요",
  heroIntro:
    "{brand}은 소프트웨어를 개발하고 파일을 다루며 반복 작업을 재사용 가능한 워크플로로 바꿀 수 있는 에이전트를 제공합니다. 무엇을 이루고 싶은지 알려 주고 작업에 맞는 호스팅형 또는 로컬 모델을 선택하면, 작업을 진행하면서 제공업체도 자유롭게 바꿀 수 있습니다.",
  getCodewhale: "Codewhale 받기",
  heroInstallAria: "설치 명령",
  exploreProduct: "제품 살펴보기",
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
  gainHeading: "Codewhale로 할 수 있는 일",
  gainLede: "프로젝트나 질문, 자동화하고 싶은 작업에서 시작해 에이전트 하나와 함께 진행하거나 큰 작업의 여러 부분을 여러 에이전트에게 나누어 맡길 수 있습니다.",
  gain: [
    [
      "만들고 싶은 것을 구현하세요",
      "만들고 싶은 것을 설명하고 코드를 읽고 파일을 편집하며 명령을 실행하고 결과를 확인할 수 있는 에이전트와 함께 작업하세요."
    ],
    [
      "일상적인 작업을 자동화하세요",
      "반복하는 작업을 스크립트와 워크플로로 만들어 두면 필요할 때마다 터미널에서 다시 실행할 수 있습니다."
    ],
    [
      "다양한 모델을 사용하세요",
      "에이전트에 호스팅형 또는 로컬 모델을 사용하고 서로 다른 모델과 역할이 각각 적합한 작업을 맡도록 할 수 있습니다."
    ]
  ],
  modelsHeading: "작업마다 선택할 수 있는 다양한 모델",
  modelsBody:
    "호스팅형 모델 제공업체에 직접 연결하거나 게이트웨이로 여러 제공업체를 이용하거나 모델을 로컬에서 실행한 뒤, 작업하면서 세션별로 사용할 모델을 선택할 수 있습니다.",
  modelsFacts: [
    ["호스팅", "codewhale auth set --provider <id>으로 저장한 내 API 키"],
    ["게이트웨이", "하나의 엔드포인트로 여러 모델, 제공자는 여전히 내가 선택"],
    ["로컬", "localhost의 vLLM, SGLang, Ollama — 보통 키 불필요"],
  ],
  modelsLink: "모델과 제공업체 살펴보기",
  startHeading: "Codewhale 시작하기",
  startLede: "Codewhale을 설치하고 모델을 연결하면 터미널에서 첫 작업을 설명할 수 있으며, 여러 에이전트가 작업을 나누어 맡도록 하고 싶을 때 Fleet을 추가할 수 있습니다.",
  startGuideLink: "시작 가이드 읽기",
  startVocabularyLink: "제품 용어 보기",
  availabilityHeading: "Codewhale을 사용할 수 있는 곳",
  availabilityLede: "Codewhale은 지금 터미널에서 사용할 수 있으며, 웹 앱과 데스크톱 앱, 클라우드 컴퓨터는 개발 중입니다.",
  availability: [
    [
      "터미널",
      "출시됨",
      "Linux, macOS, Windows용 릴리스 바이너리를 GitHub에서 제공합니다. npm과 Cargo로도 설치할 수 있습니다. Android에서 Termux로 실행하는 버전은 미리보기입니다."
    ],
    [
      "웹 앱",
      "개발 미리보기",
      "개발 미리보기에서 계정 접속과 브라우저 페어링을 이용할 수 있습니다."
    ],
    [
      "데스크톱",
      "개발 빌드",
      "macOS 앱은 개발 중이며, 공개 다운로드는 추후 제공될 예정입니다."
    ],
    [
      "클라우드 컴퓨터",
      "개발 중",
      "작업을 실행할 수 있는 호스팅 컴퓨터."
    ]
  ],
  availabilityNote: "Codewhale 계정 없이도 터미널을 사용할 수 있으며, 호스팅형 모델 사용 요금은 이용하는 제공업체에서 청구합니다.",
  accountLink: "계정 만들기",
  surfacesHeading: "Codewhale로 작업하는 방법",
  surfaces: [
    ["TUI", "대화형 터미널 작업"],
    ["codewhale exec", "스크립트와 CI"],
    ["로컬 웹 클라이언트","localhost 인터페이스. 호스팅형 브라우저 작업 공간은 개발 중"],
    ["Runtime API + MCP", "로컬 통합"],
    ["Fleet","여러 에이전트가 하나의 작업을 함께 수행"],
  ],
  runtimeLink: "연동 기능 살펴보기",
  installBandHeading: "macOS 또는 Linux에 Codewhale을 설치하세요",
  copy: "복사",
  copied: "복사됨 ✓",
  binaries: "바이너리",
  chinaMirrors: "중국 미러",
  installGuideLink: "설치 가이드 읽기",
  communityHeading: "Codewhale을 함께 개선해 주세요",
  communityBody: "버그를 발견했거나 새로운 기능에 대한 아이디어가 있거나 첫 풀 리퀘스트를 보내고 싶다면, 여러분의 이야기를 듣고 앞으로의 작업을 함께 이어 가고 싶습니다.",
  communityLinksAria: "커뮤니티 링크",
  contribute: "풀 리퀘스트 보내기",
};
