# Link Manager

[English](docs/README.md) | [문서 목록](docs/)

Tauri, React, TypeScript, Rust로 만든 Windows 심볼릭 링크 관리 프로그램입니다.

Link Manager는 기존 폴더를 다른 보관 루트로 이동한 뒤, 원래 경로에는 디렉터리 심볼릭 링크를 만들어 줍니다. 예를 들어 `C:\Users\User\.code` 같은 폴더를 다른 드라이브로 옮기면서도, 기존 경로를 기대하는 프로그램들이 그대로 동작하게 만들 수 있습니다.

## 주요 기능

- 기존 폴더를 디렉터리 심볼릭 링크로 대체합니다.
- 선택한 보관 루트 아래에 원본 경로 구조를 그대로 재현합니다.
- 원래 경로 기준 트리뷰로 관리 중인 링크를 보여줍니다.
- 이미 존재하는 파일/폴더 심볼릭 링크를 수동으로 추가할 수 있습니다.
- 루트 폴더를 스캔해서 발견된 파일/폴더 심볼릭 링크를 가져올 수 있습니다.
- 별도 탭에서 기본 심볼릭 링크 보관 루트를 지정할 수 있습니다.
- 보관 루트 밖에 있는 관리 대상 폴더를 미러링된 보관 루트 경로로 이동할 수 있습니다.
- 보관 루트가 없으면 앱 진입 전에 필수 설정 팝업을 표시합니다.
- 보관 루트 변경 시 기존 대상 폴더를 함께 이동할지 선택할 수 있으며 기본값은 켜짐입니다.
- 관리 중인 링크 상태를 검증합니다.
- 파일 시스템은 건드리지 않고 관리 목록에서만 제거할 수 있습니다.
- 링크된 폴더를 원래 경로로 복원할 수 있습니다.
- 실제 대상 폴더는 보존하고 심볼릭 링크만 삭제할 수 있습니다.
- 링크 작업에 관리자 권한이 필요하면 관리자 권한으로 다시 실행할 수 있습니다.

## 경로 미러링

폴더를 대체할 때 앱은 원본 경로 구조를 보관 루트 아래에 그대로 만듭니다.

예시:

```text
원래 폴더:
C:\Users\User\.code

보관 루트:
D:\LinkStorage

실제로 이동되는 폴더:
D:\LinkStorage\C\Users\User\.code

생성되는 심볼릭 링크:
C:\Users\User\.code -> D:\LinkStorage\C\Users\User\.code
```

드라이브 문자는 콜론을 제거한 일반 폴더명으로 변환됩니다. `~` 접두사는 현재 사용자의 홈 디렉터리로 확장됩니다.

## 요구 사항

- Windows 10 이상.
- Microsoft Edge WebView2 Runtime.
- 개발 시 Node.js 22 이상.
- 개발 시 Rust 1.94 이상.
- 심볼릭 링크 생성 및 삭제에는 관리자 권한이 필요합니다.

## 의존성 설치

```powershell
npm install
```

## 개발 실행

Tauri 개발 앱 실행:

```powershell
npm run tauri dev
```

Vite 프론트엔드만 실행:

```powershell
npm run dev
```

## 빌드

프론트엔드 빌드:

```powershell
npm run build
```

Windows 데스크톱 앱과 설치 파일 빌드:

```powershell
npm run tauri build
```

생성되는 파일:

```text
src-tauri\target\release\link-manager.exe
src-tauri\target\release\bundle\msi\Link Manager_0.1.0_x64_en-US.msi
src-tauri\target\release\bundle\nsis\Link Manager_0.1.0_x64-setup.exe
```

## 사용법

1. 앱을 실행합니다.
2. 관리자 권한이 아니라면 `관리자 재실행`을 클릭합니다.
3. `폴더 대체`를 클릭합니다.
4. 이동할 원래 폴더를 선택합니다.
5. 실제 폴더가 이동될 보관 루트를 선택합니다.
6. 계산된 대상 경로 미리보기를 확인합니다.
7. 대체 작업을 실행합니다.
8. 트리뷰에서 링크를 검증하거나, 위치를 열거나, 목록에서 제거하거나, 링크를 삭제합니다.

이미 존재하는 링크를 추가하려면 `기존 링크 추가`를 클릭합니다. 단일 링크를 선택하거나 루트 폴더를 스캔해서 파일/폴더 심볼릭 링크를 가져올 수 있습니다.

`보관 루트` 탭에서는 기본 보관 루트를 저장할 수 있습니다. 보관 루트가 지정되어 있지 않으면 앱 시작 시 필수 설정 팝업이 뜨며, 저장하기 전에는 앱을 사용할 수 없습니다. 보관 루트를 변경할 때는 기존 대상 폴더를 새 보관 루트의 원본 경로 미러링 구조로 함께 이동할 수 있고, 이 토글은 기본적으로 켜져 있습니다.

## 데이터 저장 위치

관리 중인 링크 메타데이터는 다음 위치에 저장됩니다.

```text
%LOCALAPPDATA%\LinkManager\links.json
```

기본 보관 루트 설정은 다음 위치에 저장됩니다.

```text
%LOCALAPPDATA%\LinkManager\config.json
```

이 파일에는 관리용 메타데이터만 저장됩니다. 실제 대상 폴더는 사용자가 이동한 위치에 그대로 남아 있습니다.

## 안전 규칙

- 앱은 이미 존재하는 대상 경로에 자동 병합하거나 덮어쓰지 않습니다.
- 원래 폴더가 이미 심볼릭 링크라면 폴더 대체 대신 가져오기 기능을 사용합니다.
- `목록에서 제거`는 `links.json` 항목만 삭제합니다.
- `링크 복원`은 심볼릭 링크를 제거하고 대상 폴더를 원래 경로로 되돌린 뒤 `links.json` 항목을 삭제합니다.
- `링크 삭제`는 심볼릭 링크만 삭제하며 대상 폴더는 삭제하지 않습니다.
- 폴더 이동 후 링크 생성이 실패하면 가능한 경우 폴더를 원래 경로로 복구합니다.

## 테스트

Rust 테스트 실행:

```powershell
cd src-tauri
cargo test
```

프론트엔드 타입 체크 및 프로덕션 빌드:

```powershell
npm run build
```

## 프로젝트 구조

```text
src/
  main.tsx       React UI와 Tauri command 호출
  styles.css     애플리케이션 스타일

src-tauri/
  src/lib.rs     Rust command, 심볼릭 링크 로직, 저장소, 검증
  src/main.rs    Tauri 진입점
  tauri.conf.json
```
