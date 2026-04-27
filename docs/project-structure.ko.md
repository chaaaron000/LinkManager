# 프로젝트 구조 가이드

이 문서는 Link Manager에 기여하려는 사람을 위한 코드베이스 안내서입니다. 루트 `README.md`와 `docs/README.ko.md`가 사용자 관점의 실행/사용법을 설명한다면, 이 문서는 구현 위치와 데이터 흐름을 빠르게 파악하는 데 초점을 둡니다.

## 한눈에 보기

Link Manager는 Tauri 앱입니다. 화면은 React/TypeScript가 담당하고, Windows 파일 시스템 작업은 Rust가 담당합니다.

```text
.
├─ src/                         React 프론트엔드
│  ├─ main.tsx                  화면, 상태, Tauri command 호출
│  └─ styles.css                전체 UI 스타일
├─ src-tauri/                   Rust/Tauri 백엔드
│  ├─ src/lib.rs                Tauri command, 심볼릭 링크/이동/저장 로직
│  ├─ src/main.rs               Tauri 실행 진입점
│  ├─ tauri.conf.json           Tauri 앱/번들 설정
│  ├─ capabilities/default.json Tauri 권한 설정
│  └─ icons/icon.ico            Windows 번들 아이콘
├─ docs/                        프로젝트 문서
├─ index.html                   Vite HTML 진입점
├─ package.json                 프론트엔드/Tauri CLI 스크립트
├─ vite.config.ts               Vite 개발 서버 설정
└─ tsconfig.json                TypeScript 설정
```

## 프론트엔드 구조

프론트엔드는 현재 [src/main.tsx](../src/main.tsx)에 단일 파일 중심으로 구성되어 있습니다. 앱 규모가 더 커지면 이 파일을 `components/`, `types/`, `lib/tauri.ts` 같은 단위로 나누기 좋습니다.

주요 타입:

- `ManagedLink`: 관리 중인 파일/폴더 심볼릭 링크 한 개의 UI 모델입니다.
- `AppConfig`: 현재는 `storage_root`만 가진 앱 설정입니다.
- `AppStateSnapshot`: 앱 시작/새로고침 시 Rust에서 가져오는 전체 상태입니다.
- `ReplacePreview`: 폴더 대체 전 미리보기 결과입니다.
- `ScanResult`, `ScanProgress`: 기존 링크 스캔 결과와 진행 상태입니다.
- `TreeNode`: 원래 경로 기준 트리뷰를 만들기 위한 UI 전용 노드입니다.

주요 컴포넌트:

- `App`: 최상위 상태와 화면 탭을 관리합니다.
- `TreeView`, `TreeNodeView`: 원래 경로 기준 트리뷰와 펼침/접힘 상태를 처리합니다.
- `StorageRootPanel`: 보관 루트 설정과 보관 루트 밖 대상 폴더 이동을 담당합니다.
- `RequiredStorageRootDialog`: 앱 첫 실행 시 보관 루트를 강제로 설정하게 하는 모달입니다.
- `ReplaceDialog`: 기존 폴더를 보관 루트 아래로 이동하고 원래 경로를 링크로 대체하는 흐름입니다.
- `ImportDialog`: 기존 심볼릭 링크를 수동 선택 또는 루트 스캔으로 가져오는 흐름입니다.

프론트엔드는 Rust 함수를 직접 호출할 때 `invoke("command_name", args)`를 사용합니다. Rust command의 snake_case 이름은 프론트에서 camelCase 인자로 넘깁니다. 예: Rust의 `storage_root` 인자는 TypeScript에서 `{ storageRoot: value }`로 전달합니다.

## 백엔드 구조

Rust 백엔드의 핵심은 [src-tauri/src/lib.rs](../src-tauri/src/lib.rs)에 있습니다. 이 파일은 세 영역으로 이해하면 편합니다.

- 데이터 모델: `ManagedLink`, `AppConfig`, `LinkStatus`, `ReplacePreview`, `ScanResult`.
- 파일 시스템 유틸리티: 경로 확장, 경로 미러링, JSON 저장/로드, 파일/디렉터리 이동, 심볼릭 링크 판별.
- Tauri command: 프론트엔드가 호출하는 실제 기능.

주요 command:

- `get_state`: 관리 링크 목록, 관리자 권한 여부, 앱 설정을 반환합니다.
- `set_storage_root`: 기본 보관 루트를 저장합니다.
- `preview_replace_folder`: 폴더 대체 전 실제 이동 경로와 충돌 여부를 계산합니다.
- `replace_folder`: 원래 폴더를 보관 루트 아래 미러 경로로 이동하고 심볼릭 링크를 생성합니다.
- `import_existing_link`: 기존 파일/디렉터리 심볼릭 링크를 관리 목록에 추가합니다.
- `scan_existing_links`: 루트 폴더 아래의 기존 링크를 스캔합니다.
- `validate_link`: 링크 상태를 다시 계산합니다.
- `move_link_target_to_storage`: 대상 폴더가 보관 루트 밖에 있을 때 보관 루트 안의 미러 경로로 이동합니다.
- `restore_link_target`: 심볼릭 링크를 제거하고 대상 폴더를 원래 경로로 되돌립니다.
- `delete_link`: 심볼릭 링크만 삭제하고 대상 폴더는 보존합니다.
- `relaunch_as_admin`: UAC로 관리자 권한 프로세스를 띄운 뒤 기존 프로세스를 종료합니다.
- `reveal_path`: Windows Explorer로 경로를 엽니다.

## 데이터 저장

앱 데이터는 `%LOCALAPPDATA%\LinkManager` 아래에 저장됩니다.

```text
%LOCALAPPDATA%\LinkManager\links.json   관리 중인 링크 목록
%LOCALAPPDATA%\LinkManager\config.json  보관 루트 설정
```

`links.json`은 실제 폴더 데이터를 저장하지 않습니다. 원래 경로, 대상 경로, 보관 루트, 상태, 생성/검증 시각 같은 관리용 메타데이터만 저장합니다.

## 핵심 경로 규칙

이 앱의 가장 중요한 규칙은 보관 루트 아래에 원본 경로 구조를 그대로 미러링하는 것입니다.

```text
원래 경로:
C:\Users\User\.code

보관 루트:
D:\LinkStorage

실제 대상 경로:
D:\LinkStorage\C\Users\User\.code
```

구현은 Rust의 `mirrored_target_path`가 담당합니다. 드라이브 문자는 콜론을 제거한 폴더명으로 바꿉니다. `~` 입력은 `expand_home`에서 현재 사용자 홈 디렉터리로 확장합니다.

## 주요 작업 흐름

폴더 대체:

1. 프론트의 `ReplaceDialog`가 `preview_replace_folder`로 대상 경로를 미리 계산합니다.
2. 사용자가 실행하면 `replace_folder`를 호출합니다.
3. Rust가 원래 폴더를 미러 경로로 이동합니다.
4. 원래 경로에 디렉터리 심볼릭 링크를 만듭니다.
5. 성공 시 `links.json`에 관리 항목을 저장합니다.

기존 링크 가져오기:

1. 수동 선택은 파일/폴더 심볼릭 링크 경로를 골라 `import_existing_link`를 직접 호출합니다.
2. 루트 스캔은 `scan_existing_links`를 호출합니다.
3. 스캔 중 Rust는 `scan-progress` 이벤트를 emit합니다.
4. 프론트는 이벤트를 받아 현재 스캔 경로와 카운터를 표시합니다.
5. 선택된 파일/폴더 링크 항목만 `import_existing_link`로 등록합니다.

보관 루트 변경:

1. `StorageRootPanel`에서 새 루트를 저장합니다.
2. 토글이 켜져 있으면 새 루트 밖에 있는 대상 폴더마다 `move_link_target_to_storage`를 호출합니다.
3. Rust는 기존 링크를 제거하고 대상 폴더를 새 미러 경로로 이동한 뒤 링크를 다시 만듭니다.

링크 복원:

1. `restore_link_target`이 원래 경로가 심볼릭 링크인지 확인합니다.
2. 링크를 제거합니다.
3. 대상 폴더를 원래 경로로 이동합니다.
4. 관리 목록에서 항목을 삭제합니다.

## 안전 원칙

- 대상 경로가 이미 있으면 자동 병합/덮어쓰기를 하지 않습니다.
- `링크 삭제`는 대상 폴더를 삭제하지 않습니다.
- `목록에서 제거`는 JSON만 변경합니다.
- 이동 도중 실패하면 가능한 경우 기존 링크를 복구하려고 시도합니다.
- 생성/삭제/복원/대상 이동처럼 파일 시스템을 바꾸는 작업은 관리자 권한을 요구합니다.

## 개발 명령

의존성 설치:

```powershell
npm install
```

개발 앱 실행:

```powershell
npm run tauri dev
```

프론트엔드 빌드:

```powershell
npm run build
```

Rust 테스트:

```powershell
cd src-tauri
cargo test
```

릴리스 번들 빌드:

```powershell
npm run tauri build
```

## 기여 시 참고

- UI만 바꾸는 작업은 대체로 `src/main.tsx`, `src/styles.css`에서 끝납니다.
- Windows 파일 시스템 동작을 바꾸는 작업은 `src-tauri/src/lib.rs`를 수정해야 합니다.
- 새 Tauri command를 추가하면 `invoke_handler`에 등록하고, 프론트에서 `invoke` 이름/인자를 맞춰야 합니다.
- 스캔처럼 오래 걸리는 작업은 UI 멈춤을 피하기 위해 `spawn_blocking`과 이벤트 emit 방식을 유지하는 편이 좋습니다.
- 위험 작업을 추가할 때는 일반 액션과 분리된 UI에 배치하고, 대상 폴더를 삭제하지 않는 방향을 기본값으로 잡습니다.
