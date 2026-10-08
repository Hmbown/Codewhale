import type { HomeDict } from "../types";

/** Korean home copy: useful work with chosen models and connected tools. */

export const home: HomeDict = {
  metaTitle: "Codewhale: 내 모델과 도구로 원하는 것을 만들기",
  metaDescription:
    "Codewhale로 앱을 만들고, 업무를 자동화하고, 연결된 도구로 작업하세요. 오픈 소스로, 기존 모델 API와 로컬 또는 자체 호스팅 추론을 사용할 수 있습니다.",
  heroTitle: "생각한 것을 직접 만드세요.",
  heroIntro:
    "앱을 만들고, 작업을 자동화하고, 모아 둔 조사 자료를 유용한 결과로 바꾸세요. {brand}는 이미 쓰는 모델 API, 자체 추론 환경, 직접 연결한 도구와 함께 작동합니다.",
  getCodewhale: "Codewhale 설치",
  heroInstallAria: "설치 명령",
  exploreProduct: "Codewhale 둘러보기",
  shotPreview: "터미널 미리보기",
  screenshotAlt:
    "Codewhale v{version}: 대화, 메시지 입력창, 세션 제어를 보여 주는 실제 터미널 캡처.",
  latestRelease: "최신 릴리스 {tag}",
  releaseUnavailable: "릴리스 상태를 확인할 수 없음",
  currentSource: "소스",
  sourceCandidate: "미공개",
  publishedRelease: "공개됨",
  gainHeading: "쓸모 있는 것을 만드세요.",
  gainLede: "원하는 결과부터 정하세요. Codewhale은 파일, 명령, 연결된 도구로 작업하고, 접근 범위와 승인 방식은 사용자가 결정합니다.",
  gain: [
    [
      "앱과 도구 만들기",
      "아이디어를 작동하는 앱, 유용한 스크립트, 기존 프로젝트의 새 기능으로 만드세요. 에이전트가 함께 작성하고 실행하며 테스트합니다."
    ],
    [
      "반복하는 업무 자동화",
      "반복 작업을 터미널, 스크립트, CI에서 실행하는 워크플로로 바꾸세요. 병렬로 진행할 수 있는 일에는 Fleet 에이전트 팀을 활용하세요."
    ],
    [
      "쓰고 있는 도구 연결",
      "플러그인, MCP 서버 또는 API로 Gmail과 Slack 같은 도구를 연결하세요. 이 서비스를 파일과 명령과 함께 같은 작업에 활용하세요."
    ]
  ],
  exampleTasks: [
    "예약을 받는 앱을 만들어 줘.",
    "매출 CSV로 매주 다시 만들 수 있는 보고서를 만들어 줘.",
    "연결된 이메일의 메시지를 할 일 목록으로 정리해 줘.",
  ],
  // A static example report built from local sample orders.
  reportTitle: "주간 매출 보고서",
  reportSampleLabel: "보고서 예시 · 샘플 데이터",
  reportDescription: "Codewhale에 주문을 주별로 집계하고 다음 CSV에도 쓸 수 있도록 처리 과정을 저장해 달라고 요청하세요.",
  reportSourceLabel: "입력 데이터:",
  reportColumns: ["주 시작일","주문 수","매출 (USD)"],
  reportTotalLabel: "합계",
  reportTrend: "첫째 주 대비 마지막 주 매출 변화: {change}.",
  reportDownloadLabel: "보고서 CSV 다운로드",
  chapterModels: "당신의 모델",
  modelsHeading: "내가 선택한 모델을 그대로.",
  modelsBody:
    "이미 결제하는 모델 API, 호환 게이트웨이, 내 하드웨어의 추론을 사용하세요. 세션마다, Fleet의 각 에이전트마다 모델을 선택할 수 있습니다.",
  modelsFacts: [
    [
      "내 API 계정",
      "자신의 키로 OpenAI, Anthropic, Google, DeepSeek 등에 연결하세요."
    ],
    [
      "내 게이트웨이",
      "OpenAI 호환 엔드포인트를 연결하고 제공되는 모델을 선택하세요."
    ],
    [
      "내 추론 환경",
      "Ollama, vLLM, SGLang으로 로컬 또는 자체 호스팅 모델을 실행하세요."
    ]
  ],
  modelsLink: "모델과 제공업체 둘러보기",
  startHeading: "할 일을 가져오세요. 시작하세요.",
  startLede: "Codewhale을 설치하고 모델을 연결한 뒤, 해 볼 만한 일을 맡기세요. 에이전트 하나로 시작하고, 필요할 때 도구나 팀을 추가하세요.",
  startGuideLink: "시작 가이드 따라 하기",
  startVocabularyLink: "제품 용어 보기",
  chapterAvailability: "실행 환경",
  availabilityHeading: "터미널에서 시작하세요.",
  availabilityLede: "터미널과 로컬 브라우저 클라이언트는 지금 사용할 수 있습니다. 네이티브 데스크톱 앱과 새로 구축하는 호스팅 웹 앱은 개발 중입니다.",
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
  availabilityNote: "터미널, 로컬 브라우저, GUI는 Codewhale 계정이 필요 없습니다. 호스팅 웹과 데스크톱은 계정을 사용합니다. 본인의 제공업체 키를 사용하는 경우, 해당 사용 요금은 그 제공업체가 청구합니다.",
  accountLink: "계정 만들기",
  surfacesHeading: "하나의 작업에 파일, 앱, 에이전트까지.",
  surfaces: [
    [
      "파일과 터미널",
      "파일을 만들고, 명령을 실행하고, 데이터를 살펴보고, 만든 것을 테스트하세요. 작업 폴더와 권한은 직접 설정합니다."
    ],
    [
      "플러그인과 연결된 앱",
      "플러그인과 MCP로 스킬과 도구를 추가하세요. 에이전트가 사용할 연결을 검토하고 활성화합니다."
    ],
    [
      "브라우저와 컴퓨터 사용 · 미리보기",
      "허용한 접근 범위에서 브라우저 도구와 Computer Use 플러그인으로 앱과 웹사이트에서 작업하세요."
    ],
    [
      "이어서 할 수 있는 세션",
      "대화, 도구 결과, 작업 기록을 함께 보관하세요. 터미널이나 로컬 브라우저 클라이언트에서 작업을 다시 이어 갑니다."
    ],
    [
      "에이전트 팀",
      "Fleet으로 큰 일을 역할과 모델이 다른 에이전트에게 나누고, 한곳에서 진행 상황을 확인하세요."
    ]
  ],
  runtimeLink: "모든 연동 기능 보기",
  installBandHeading: "macOS 또는 Linux에 설치하세요",
  copy: "복사",
  copied: "복사됨 ✓",
  binaries: "바이너리",
  chinaMirrors: "중국 미러",
  installGuideLink: "설치 가이드 읽기",
  communityHeading: "Codewhale을 나만의 도구로.",
  communityBody: "Codewhale은 오픈 소스입니다. 코드를 읽고, 플러그인을 만들고, 워크플로를 공유하며 다음 버전을 함께 개선하세요.",
  communityLinksAria: "커뮤니티 링크",
  contribute: "GitHub에서 기여하기",
};
