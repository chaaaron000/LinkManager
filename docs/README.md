# Link Manager

[한국어](../README.md) | [Documentation](index.md)

Windows symbolic link manager built with Tauri, React, TypeScript, and Rust.

Link Manager helps you move an existing folder into a storage root while preserving its original path as a directory symbolic link. It is useful when you want to relocate folders such as `C:\Users\User\.code` to another drive, while keeping programs that expect the original path working normally.

## Features

- Replace an existing folder with a directory symbolic link.
- Mirror the original path under the selected storage root.
- Manage links in a tree view based on the original path.
- Import existing file and directory symbolic links manually.
- Scan a root folder and import discovered file and directory symbolic links.
- Configure a default symbolic-link storage root in a dedicated tab.
- Move managed link targets that are outside the storage root into the mirrored storage layout.
- Require a storage root before entering the app.
- Optionally move existing targets when changing the storage root. This option is on by default.
- Validate managed links.
- Remove entries from the manager without touching the file system.
- Restore a linked folder back to its original path.
- Delete only the symbolic link while preserving the real target folder.
- Relaunch with administrator privileges when link operations require elevation.

## Path Mirroring

When replacing a folder, the app keeps the original path structure under the storage root.

Example:

```text
Original folder:
C:\Users\User\.code

Storage root:
D:\LinkStorage

Actual moved folder:
D:\LinkStorage\C\Users\User\.code

Symbolic link created at:
C:\Users\User\.code -> D:\LinkStorage\C\Users\User\.code
```

Drive letters are converted into normal folder names by removing the colon. The `~` prefix is expanded to the current user's home directory.

## Requirements

- Windows 10 or later.
- Microsoft Edge WebView2 Runtime.
- Node.js 22 or later for development.
- Rust 1.94 or later for development.
- Administrator privileges for creating and deleting symbolic links.

## Install Dependencies

```powershell
npm install
```

## Development

Run the Tauri development app:

```powershell
npm run tauri dev
```

Run only the Vite frontend:

```powershell
npm run dev
```

## Build

Build the frontend:

```powershell
npm run build
```

Build the Windows desktop app and installers:

```powershell
npm run tauri build
```

Generated outputs:

```text
src-tauri\target\release\link-manager.exe
src-tauri\target\release\bundle\msi\Link Manager_0.1.0_x64_en-US.msi
src-tauri\target\release\bundle\nsis\Link Manager_0.1.0_x64-setup.exe
```

## Usage

1. Launch the app.
2. If the app is not elevated, click `관리자 재실행`.
3. Click `폴더 대체`.
4. Select the original folder to relocate.
5. Select the storage root where the real folder should be moved.
6. Confirm the computed target path preview.
7. Run the replacement.
8. Use the tree view to validate, open, remove, or delete managed links.

To add links that already exist, click `기존 링크 추가`. You can select a single link or scan a root folder for file and directory symbolic links.

Use the `보관 루트` tab to save the default storage root. If no storage root is configured, the app shows a blocking setup dialog on startup. When changing the storage root, the app can also move existing target folders into the new mirrored storage layout; this toggle is enabled by default.

## Data Storage

Managed link metadata is stored in:

```text
%LOCALAPPDATA%\LinkManager\links.json
```

The default storage root is stored in:

```text
%LOCALAPPDATA%\LinkManager\config.json
```

This file stores only management metadata. The actual target folders remain wherever you moved them.

## Safety Notes

- The app does not merge or overwrite an existing target path.
- If the original folder is already a symbolic link, use the import flow instead of replacement.
- `목록에서 제거` only removes the entry from `links.json`.
- `링크 복원` removes the symbolic link, moves the target folder back to the original path, and removes the entry from `links.json`.
- `링크 삭제` removes only the symbolic link and does not delete the target folder.
- If link creation fails after moving a folder, the app attempts to restore the folder to the original path.

## Tests

Run Rust tests:

```powershell
cd src-tauri
cargo test
```

Run frontend type check and production build:

```powershell
npm run build
```

## Project Structure

```text
src/
  main.tsx       React UI and Tauri command calls
  styles.css     Application styling

src-tauri/
  src/lib.rs     Rust commands, symlink logic, storage, validation
  src/main.rs    Tauri entrypoint
  tauri.conf.json
```
