# Project Structure Guide

This guide is for contributors who want to understand the Link Manager codebase. The root `README.md` and `docs/README.ko.md` explain usage from a user perspective; this document focuses on implementation layout and data flow.

## Overview

Link Manager is a Tauri app. React/TypeScript owns the UI, and Rust owns Windows file-system operations.

```text
.
├─ src/                         React frontend
│  ├─ main.tsx                  UI, state, and Tauri command calls
│  └─ styles.css                Application styling
├─ src-tauri/                   Rust/Tauri backend
│  ├─ src/lib.rs                Tauri commands, symlink, move, and storage logic
│  ├─ src/main.rs               Tauri runtime entrypoint
│  ├─ tauri.conf.json           Tauri app and bundle configuration
│  ├─ capabilities/default.json Tauri permission configuration
│  └─ icons/icon.ico            Windows bundle icon
├─ docs/                        Project documentation
├─ index.html                   Vite HTML entrypoint
├─ package.json                 Frontend and Tauri CLI scripts
├─ vite.config.ts               Vite dev server configuration
└─ tsconfig.json                TypeScript configuration
```

## Frontend

The frontend currently lives mostly in [src/main.tsx](../src/main.tsx). As the app grows, good split points would be `components/`, `types/`, and `lib/tauri.ts`.

Important types:

- `ManagedLink`: UI model for one managed file or folder symbolic link.
- `AppConfig`: app configuration; currently only `storage_root`.
- `AppStateSnapshot`: full state loaded from Rust during startup/refresh.
- `ReplacePreview`: preflight result before replacing a folder with a symlink.
- `ScanResult`, `ScanProgress`: import scan result and progress event payload.
- `TreeNode`: UI-only node for the original-path tree view.

Important components:

- `App`: top-level state owner and tab controller.
- `TreeView`, `TreeNodeView`: original-path tree view and expand/collapse state.
- `LinkListView`: flat original-path list view.
- `StorageRootPanel`: storage-root settings and target relocation actions.
- `RequiredStorageRootDialog`: blocking first-run modal for required storage-root setup.
- `ReplaceDialog`: moves an existing folder into storage and replaces the original path with a symlink.
- `ImportDialog`: imports existing symbolic links manually or via root scan.

The frontend calls Rust with `invoke("command_name", args)`. Tauri maps Rust snake_case parameters to camelCase on the JavaScript side. For example, Rust `storage_root` is passed as `{ storageRoot: value }`.

## Backend

The Rust backend is centered in [src-tauri/src/lib.rs](../src-tauri/src/lib.rs). It has three main responsibilities:

- Data models: `ManagedLink`, `AppConfig`, `LinkStatus`, `ReplacePreview`, `ScanResult`.
- File-system helpers: path expansion, mirrored path calculation, JSON load/save, file/directory move helpers, and symlink detection.
- Tauri commands: the functions called by the frontend.

Important commands:

- `get_state`: returns managed links, admin status, and app config.
- `set_storage_root`: saves the default storage root.
- `preview_replace_folder`: calculates the final storage target path and detects conflicts before replacement.
- `replace_folder`: moves the original folder into mirrored storage and creates the directory symlink.
- `import_existing_link`: adds an existing file or directory symbolic link to the manager.
- `scan_existing_links`: scans a root folder for existing links.
- `validate_link`: recalculates link status.
- `move_link_target_to_storage`: moves a target outside the storage root into the mirrored storage layout.
- `restore_link_target`: removes the symlink and moves the target back to the original path.
- `delete_link`: deletes only the symlink and preserves the target.
- `relaunch_as_admin`: starts an elevated process via UAC, then exits the current process.
- `reveal_path`: opens a path in Windows Explorer.

## Data Storage

App data is stored under `%LOCALAPPDATA%\LinkManager`.

```text
%LOCALAPPDATA%\LinkManager\links.json   Managed link metadata
%LOCALAPPDATA%\LinkManager\config.json  Storage-root setting
```

`links.json` does not store actual file or folder content. It stores management metadata such as original path, target path, storage root, kind, status, creation time, and last validation time.

## Path Mirroring

The key project rule is that storage paths mirror the original path under the selected storage root.

```text
Original path:
C:\Users\User\.code

Storage root:
D:\LinkStorage

Actual target path:
D:\LinkStorage\C\Users\User\.code
```

Rust implements this in `mirrored_target_path`. Drive letters become ordinary folder names with the colon removed. `~` input is expanded to the current user's home directory by `expand_home`.

## Main Workflows

Folder replacement:

1. `ReplaceDialog` calls `preview_replace_folder` to calculate the target path.
2. The user confirms, then the frontend calls `replace_folder`.
3. Rust moves the original folder into the mirrored target path.
4. Rust creates a directory symlink at the original path.
5. The link is saved to `links.json`.

Existing link import:

1. Manual selection chooses a file or folder symlink and calls `import_existing_link`.
2. Root scan calls `scan_existing_links`.
3. During scanning, Rust emits `scan-progress` events.
4. The frontend displays the current scanned path and counters.
5. Selected file/folder links are registered through `import_existing_link`.

Storage-root change:

1. `StorageRootPanel` saves the new root.
2. If the move toggle is enabled, the frontend calls `move_link_target_to_storage` for each target outside the new root.
3. Rust removes the old symlink, moves the target into the new mirrored path, and recreates the symlink.

Link restore:

1. `restore_link_target` verifies that the original path is a symlink.
2. Rust removes the symlink.
3. Rust moves the target back to the original path.
4. The item is removed from the manager.

## Safety Rules

- Existing target paths are never automatically merged or overwritten.
- `링크 삭제` deletes only the symlink, not the target.
- `목록에서 제거` changes only JSON metadata.
- If a move fails midway, Rust attempts to restore the previous symlink.
- Operations that mutate the file system, such as create/delete/restore/move target, require administrator privileges.

## Development Commands

Install dependencies:

```powershell
npm install
```

Run the development app:

```powershell
npm run tauri dev
```

Build the frontend:

```powershell
npm run build
```

Run Rust tests:

```powershell
cd src-tauri
cargo test
```

Build release bundles:

```powershell
npm run tauri build
```

## Contribution Notes

- UI-only changes usually stay in `src/main.tsx` and `src/styles.css`.
- Windows file-system behavior lives in `src-tauri/src/lib.rs`.
- When adding a Tauri command, register it in `invoke_handler` and match the frontend `invoke` name and argument shape.
- Long-running work, such as scans, should keep using `spawn_blocking` plus event emission to avoid UI stalls.
- New destructive actions should stay visually separated from normal actions and should default to preserving target data.
