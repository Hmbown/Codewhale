import type { ComputerUseDict } from "../types";

/**
 * Korean dictionary for `app/[locale]/computer-use/page.tsx` and the
 * Computer Use section of the install page. Product names, the menu items
 * Pause, Stop and Check for updates, and the macOS setting names stay as
 * the app shows them; the macOS names use the Korean system UI labels
 * (손쉬운 사용, 화면 기록, 응용 프로그램).
 */
export const computerUse: ComputerUseDict = {
  metaTitle: "Mac용 Computer Use · Codewhale",
  metaDescription: "Mac용 Codewhale Computer Use를 다운로드하고 설정하세요. 백그라운드 앱 제어, 권한 설정, 그리고 사용자가 직접 누르는 Pause와 Stop 제어를 제공합니다.",
  title: "Computer Use",
  lead: "당신이 계속 일하는 동안 Codewhale이 앱 안에서 작업을 대신합니다. Mac 도우미는 권한 설정, 백그라운드 앱 제어, 입력을 일시 정지하거나 중지하는 기능을 메뉴 막대에 모아 줍니다.",
  publisher: "제공: Codewhale",
  download: "Mac용 다운로드",
  downloadZip: "ZIP 아카이브(앱 내 업데이터가 사용)",
  requirements: "macOS 13.5 이상 · Apple silicon 및 Intel",
  included: "앱 하나만 다운로드하면 됩니다. Node나 컴파일러를 따로 설치할 필요가 없습니다.",
  pendingTitle: "Mac용 다운로드 준비 중",
  pendingBody: "Apple 공증과 릴리스 검사가 끝나면 공개 설치 파일이 여기에 표시됩니다.",
  unavailableTitle: "다운로드 가능 여부를 확인할 수 없습니다",
  unavailableBody: "페이지를 새로 고쳐 다시 시도하거나 아래의 공개 릴리스를 확인하세요.",
  releases: "공개 릴리스",
  receipt: "다운로드 검증 정보",
  setup: "Mac 설정하기",
  steps: [
    { title: "앱 설치", body: "디스크 이미지를 열고 Codewhale Computer Use를 “응용 프로그램”으로 드래그하세요. “응용 프로그램”에서 앱을 연 다음 메뉴 막대의 고래 아이콘에서 Computer Use를 선택합니다." },
    { title: "권한 검토", body: "설정 버튼으로 시스템 설정의 “손쉬운 사용”과 “화면 기록”을 여세요. 어떤 권한을 허용할지는 당신이 결정합니다." },
    { title: "백그라운드 검사 실행", body: "도우미가 일회용 연습 창을 열어 텍스트를 입력하고 그 창을 캡처합니다. 실행 중에 포인터나 활성 앱이 바뀌었는지도 확인합니다." },
    { title: "Codewhale에 연결", body: "Codewhale 플러그인 마켓플레이스에서 Computer Use를 검토하고 신뢰한 뒤 활성화하세요. 로컬 작업이 도우미의 Pause와 Stop 제어를 거치도록 플러그인 0.3.1 이상을 사용하세요." },
  ],
  controlsTitle: "일은 계속, 제어는 당신에게.",
  controlsBody: "지원되는 작업은 선택한 앱의 백그라운드에서 실행됩니다. 포그라운드 제어가 필요한 앱과 제스처는 당신의 승인이 있어야 합니다. 메뉴에는 대상 앱과 입력 모드가 표시되며, Pause(일시 정지)는 도우미 입력을 멈추고 Stop(중지)은 진행 중인 세션을 종료합니다.",
  updateTitle: "업데이트 시점은 당신이 결정",
  updateBody: "앱에서 Check for updates(업데이트 확인)를 선택하세요. 업데이트를 설치하기 전에 다운로드 파일, Codewhale 서명, Apple 공증을 검사하고 복구를 위해 이전 앱을 보관합니다.",
  help: "설정 및 문제 해결",
  notes: "릴리스 노트",
  demo: "백그라운드 검사 보기",
  source: "소스 및 다른 플랫폼",
  platforms: "이 다운로드는 Mac용입니다. Windows와 Linux는 현재 소스 플러그인과 호스트 측 설정을 사용합니다.",
};
