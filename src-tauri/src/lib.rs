use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    env,
    ffi::OsStr,
    fs, io,
    os::windows::{ffi::OsStrExt, fs::symlink_dir},
    path::{Component, Path, PathBuf, Prefix},
    process::{exit, Command},
    ptr::null_mut,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::Emitter;
use windows::core::PCWSTR;
use windows::Win32::{
    Foundation::HWND,
    UI::Shell::{IsUserAnAdmin, ShellExecuteW},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum LinkKind {
    DirectorySymlink,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum LinkStatus {
    Ok,
    MissingTarget,
    MissingLink,
    NotSymlink,
    WrongTarget,
    AccessDenied,
    UnknownError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedLink {
    pub id: String,
    pub name: String,
    pub original_path: String,
    pub target_path: String,
    pub storage_root: Option<String>,
    pub kind: LinkKind,
    pub status: LinkStatus,
    pub created_at: u64,
    pub last_checked_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub storage_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSnapshot {
    pub links: Vec<ManagedLink>,
    pub is_admin: bool,
    pub config: AppConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplacePreview {
    pub original_path: String,
    pub storage_root: String,
    pub target_path: String,
    pub target_exists: bool,
    pub original_exists: bool,
    pub original_is_symlink: bool,
    pub is_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub original_path: String,
    pub target_path: String,
    pub already_managed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanProgress {
    pub current_path: String,
    pub scanned_count: usize,
    pub found_count: usize,
    pub done: bool,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn app_data_dir() -> Result<PathBuf, String> {
    let base = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .ok_or_else(|| "Cannot resolve app data directory".to_string())?;
    Ok(base.join("LinkManager"))
}

fn store_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("links.json"))
}

fn config_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("config.json"))
}

fn load_links_from(path: &Path) -> Result<Vec<ManagedLink>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

fn save_links_to(path: &Path, links: &[ManagedLink]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let text = serde_json::to_string_pretty(links).map_err(|err| err.to_string())?;
    fs::write(path, text).map_err(|err| err.to_string())
}

fn load_links() -> Result<Vec<ManagedLink>, String> {
    load_links_from(&store_path()?)
}

fn save_links(links: &[ManagedLink]) -> Result<(), String> {
    save_links_to(&store_path()?, links)
}

fn load_config() -> Result<AppConfig, String> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppConfig { storage_root: None });
    }
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    if text.trim().is_empty() {
        return Ok(AppConfig { storage_root: None });
    }
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

fn save_config(config: &AppConfig) -> Result<AppConfig, String> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let text = serde_json::to_string_pretty(config).map_err(|err| err.to_string())?;
    fs::write(path, text).map_err(|err| err.to_string())?;
    Ok(config.clone())
}

fn wide_null(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}

fn is_running_as_admin() -> bool {
    unsafe { IsUserAnAdmin().as_bool() }
}

fn expand_home(input: &str) -> Result<PathBuf, String> {
    let trimmed = input.trim();
    if trimmed == "~" || trimmed.starts_with("~/") || trimmed.starts_with("~\\") {
        let home = env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .ok_or_else(|| "Cannot resolve USERPROFILE for ~ expansion".to_string())?;
        let rest = trimmed
            .trim_start_matches('~')
            .trim_start_matches(['/', '\\']);
        return Ok(home.join(rest));
    }
    Ok(PathBuf::from(trimmed))
}

fn normalize_path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn mirrored_target_path(original: &Path, storage_root: &Path) -> Result<PathBuf, String> {
    let mut target = storage_root.to_path_buf();
    let mut components = original.components();

    match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                target.push((letter as char).to_string());
            }
            _ => return Err("Only local drive paths are supported".to_string()),
        },
        _ => return Err("Original path must be an absolute Windows drive path".to_string()),
    }

    match components.next() {
        Some(Component::RootDir) => {}
        _ => return Err("Original path must include a drive root".to_string()),
    }

    for component in components {
        if let Component::Normal(part) = component {
            target.push(part);
        }
    }

    Ok(target)
}

fn is_symlink(path: &Path) -> Result<bool, io::Error> {
    Ok(fs::symlink_metadata(path)?.file_type().is_symlink())
}

fn read_link_target(path: &Path) -> Result<PathBuf, String> {
    fs::read_link(path).map_err(|err| err.to_string())
}

fn status_for(original: &Path, expected_target: &Path) -> LinkStatus {
    let metadata = match fs::symlink_metadata(original) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return LinkStatus::MissingLink,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            return LinkStatus::AccessDenied
        }
        Err(_) => return LinkStatus::UnknownError,
    };

    if !metadata.file_type().is_symlink() {
        return LinkStatus::NotSymlink;
    }

    if !expected_target.exists() {
        return LinkStatus::MissingTarget;
    }

    match fs::read_link(original) {
        Ok(actual) if same_path(&actual, expected_target) => LinkStatus::Ok,
        Ok(_) => LinkStatus::WrongTarget,
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => LinkStatus::AccessDenied,
        Err(_) => LinkStatus::UnknownError,
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = left.to_string_lossy().replace('/', "\\").to_lowercase();
    let right = right.to_string_lossy().replace('/', "\\").to_lowercase();
    left == right
}

fn path_is_under(path: &Path, root: &Path) -> bool {
    let path = path.to_string_lossy().replace('/', "\\").to_lowercase();
    let mut root = root.to_string_lossy().replace('/', "\\").to_lowercase();
    while root.ends_with('\\') {
        root.pop();
    }
    path == root || path.starts_with(&format!("{root}\\"))
}

fn copy_dir_recursive(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let destination = to.join(entry.file_name());
        let metadata = entry.file_type()?;
        if metadata.is_dir() {
            copy_dir_recursive(&source, &destination)?;
        } else if metadata.is_file() {
            fs::copy(&source, &destination)?;
        }
    }
    Ok(())
}

fn move_dir_cross_volume(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            copy_dir_recursive(from, to).map_err(|err| {
                format!("Failed to move folder. Rename error: {rename_err}. Copy error: {err}")
            })?;
            fs::remove_dir_all(from).map_err(|err| {
                format!("Copied folder to target, but failed to remove original: {err}")
            })
        }
    }
}

fn make_id(path: &Path) -> String {
    let clean = path
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    format!("{}-{}", now_secs(), clean.trim_matches('-'))
}

fn upsert_link(mut next: ManagedLink) -> Result<ManagedLink, String> {
    let mut links = load_links()?;
    let original = next.original_path.to_lowercase();
    next.status = status_for(Path::new(&next.original_path), Path::new(&next.target_path));

    if let Some(existing) = links
        .iter_mut()
        .find(|link| link.original_path.to_lowercase() == original)
    {
        next.id = existing.id.clone();
        next.created_at = existing.created_at;
        *existing = next.clone();
    } else {
        links.push(next.clone());
    }

    save_links(&links)?;
    Ok(next)
}

#[tauri::command]
fn get_state() -> Result<AppStateSnapshot, String> {
    let mut links = load_links()?;
    for link in &mut links {
        link.status = status_for(Path::new(&link.original_path), Path::new(&link.target_path));
        link.last_checked_at = Some(now_secs());
    }
    save_links(&links)?;

    Ok(AppStateSnapshot {
        links,
        is_admin: is_running_as_admin(),
        config: load_config()?,
    })
}

#[tauri::command]
fn set_storage_root(storage_root: String) -> Result<AppConfig, String> {
    let root = expand_home(&storage_root)?;
    if !root.exists() {
        fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    }
    if !root.is_dir() {
        return Err("Storage root must be a folder".to_string());
    }
    save_config(&AppConfig {
        storage_root: Some(normalize_path_string(&root)),
    })
}

#[tauri::command]
fn preview_replace_folder(
    original_path: String,
    storage_root: String,
) -> Result<ReplacePreview, String> {
    let original = expand_home(&original_path)?;
    let storage = expand_home(&storage_root)?;
    let target = mirrored_target_path(&original, &storage)?;
    let original_is_symlink = match is_symlink(&original) {
        Ok(value) => value,
        Err(err) if err.kind() == io::ErrorKind::NotFound => false,
        Err(err) => return Err(err.to_string()),
    };

    Ok(ReplacePreview {
        original_path: normalize_path_string(&original),
        storage_root: normalize_path_string(&storage),
        target_path: normalize_path_string(&target),
        target_exists: target.exists(),
        original_exists: original.exists(),
        original_is_symlink,
        is_admin: is_running_as_admin(),
    })
}

#[tauri::command]
fn replace_folder(original_path: String, storage_root: String) -> Result<ManagedLink, String> {
    if !is_running_as_admin() {
        return Err("Administrator privileges are required".to_string());
    }

    let original = expand_home(&original_path)?;
    let storage = expand_home(&storage_root)?;
    let target = mirrored_target_path(&original, &storage)?;

    if !original.exists() {
        return Err("Original folder does not exist".to_string());
    }
    if !original.is_dir() {
        return Err("Original path must be a folder".to_string());
    }
    if is_symlink(&original).map_err(|err| err.to_string())? {
        return Err("Original path is already a symbolic link. Use import instead.".to_string());
    }
    if target.exists() {
        return Err(
            "Target path already exists. Automatic merge/overwrite is disabled.".to_string(),
        );
    }

    move_dir_cross_volume(&original, &target)?;

    match symlink_dir(&target, &original) {
        Ok(()) => {}
        Err(err) => {
            let _ = fs::remove_dir(&original);
            let _ = fs::rename(&target, &original);
            return Err(format!("Moved folder, but failed to create symlink: {err}"));
        }
    }

    let name = original
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Folder")
        .to_string();

    upsert_link(ManagedLink {
        id: make_id(&original),
        name,
        original_path: normalize_path_string(&original),
        target_path: normalize_path_string(&target),
        storage_root: Some(normalize_path_string(&storage)),
        kind: LinkKind::DirectorySymlink,
        status: LinkStatus::Ok,
        created_at: now_secs(),
        last_checked_at: Some(now_secs()),
    })
}

#[tauri::command]
fn import_existing_link(original_path: String) -> Result<ManagedLink, String> {
    let original = expand_home(&original_path)?;
    if !is_symlink(&original).map_err(|err| err.to_string())? {
        return Err("Selected path is not a symbolic link".to_string());
    }
    let target = read_link_target(&original)?;
    if !target.is_dir() {
        return Err("Only directory symbolic links are supported".to_string());
    }

    let name = original
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Imported Link")
        .to_string();

    upsert_link(ManagedLink {
        id: make_id(&original),
        name,
        original_path: normalize_path_string(&original),
        target_path: normalize_path_string(&target),
        storage_root: None,
        kind: LinkKind::DirectorySymlink,
        status: LinkStatus::Ok,
        created_at: now_secs(),
        last_checked_at: Some(now_secs()),
    })
}

#[tauri::command]
async fn scan_existing_links(
    app_handle: tauri::AppHandle,
    root_path: String,
) -> Result<Vec<ScanResult>, String> {
    tauri::async_runtime::spawn_blocking(move || scan_existing_links_inner(app_handle, root_path))
        .await
        .map_err(|err| err.to_string())?
}

fn scan_existing_links_inner(
    app_handle: tauri::AppHandle,
    root_path: String,
) -> Result<Vec<ScanResult>, String> {
    let root = expand_home(&root_path)?;
    if !root.is_dir() {
        return Err("Scan root must be a folder".to_string());
    }

    let managed: BTreeSet<String> = load_links()?
        .into_iter()
        .map(|link| link.original_path.to_lowercase())
        .collect();
    let mut results = Vec::new();
    let mut stack = vec![root];
    let mut scanned_count = 0usize;

    while let Some(current) = stack.pop() {
        scanned_count += 1;
        let current_path = normalize_path_string(&current);
        if scanned_count == 1 || scanned_count % 10 == 0 {
            let _ = app_handle.emit(
                "scan-progress",
                ScanProgress {
                    current_path: current_path.clone(),
                    scanned_count,
                    found_count: results.len(),
                    done: false,
                },
            );
        }

        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };

            if metadata.file_type().is_symlink() {
                if let Ok(target) = fs::read_link(&path) {
                    if target.is_dir() {
                        let original_path = normalize_path_string(&path);
                        results.push(ScanResult {
                            already_managed: managed.contains(&original_path.to_lowercase()),
                            original_path,
                            target_path: normalize_path_string(&target),
                        });
                        let _ = app_handle.emit(
                            "scan-progress",
                            ScanProgress {
                                current_path: normalize_path_string(&path),
                                scanned_count,
                                found_count: results.len(),
                                done: false,
                            },
                        );
                    }
                }
            } else if metadata.is_dir() {
                stack.push(path);
            }
        }
    }

    results.sort_by(|a, b| a.original_path.cmp(&b.original_path));
    let _ = app_handle.emit(
        "scan-progress",
        ScanProgress {
            current_path: "Scan complete".to_string(),
            scanned_count,
            found_count: results.len(),
            done: true,
        },
    );
    Ok(results)
}

#[tauri::command]
fn validate_link(id: String) -> Result<ManagedLink, String> {
    let mut links = load_links()?;
    let Some(link) = links.iter_mut().find(|link| link.id == id) else {
        return Err("Managed link not found".to_string());
    };
    link.status = status_for(Path::new(&link.original_path), Path::new(&link.target_path));
    link.last_checked_at = Some(now_secs());
    let updated = link.clone();
    save_links(&links)?;
    Ok(updated)
}

#[tauri::command]
fn remove_from_manager(id: String) -> Result<Vec<ManagedLink>, String> {
    let mut links = load_links()?;
    links.retain(|link| link.id != id);
    save_links(&links)?;
    Ok(links)
}

#[tauri::command]
fn delete_link(id: String) -> Result<Vec<ManagedLink>, String> {
    if !is_running_as_admin() {
        return Err("Administrator privileges are required".to_string());
    }

    let mut links = load_links()?;
    let Some(index) = links.iter().position(|link| link.id == id) else {
        return Err("Managed link not found".to_string());
    };

    let original = PathBuf::from(&links[index].original_path);
    if original.exists() || fs::symlink_metadata(&original).is_ok() {
        if !is_symlink(&original).map_err(|err| err.to_string())? {
            return Err(
                "Refusing to delete because the original path is not a symbolic link".to_string(),
            );
        }
        fs::remove_dir(&original).map_err(|err| err.to_string())?;
    }

    links.remove(index);
    save_links(&links)?;
    Ok(links)
}

#[tauri::command]
fn restore_link_target(id: String) -> Result<Vec<ManagedLink>, String> {
    if !is_running_as_admin() {
        return Err("Administrator privileges are required".to_string());
    }

    let mut links = load_links()?;
    let Some(index) = links.iter().position(|link| link.id == id) else {
        return Err("Managed link not found".to_string());
    };

    let original = PathBuf::from(&links[index].original_path);
    let target = PathBuf::from(&links[index].target_path);

    if !target.exists() {
        return Err("Target folder does not exist".to_string());
    }
    if !target.is_dir() {
        return Err("Target path must be a folder".to_string());
    }
    if !is_symlink(&original).map_err(|err| err.to_string())? {
        return Err("Original path must be a symbolic link".to_string());
    }

    fs::remove_dir(&original).map_err(|err| err.to_string())?;

    if let Err(err) = move_dir_cross_volume(&target, &original) {
        let _ = symlink_dir(&target, &original);
        return Err(err);
    }

    links.remove(index);
    save_links(&links)?;
    Ok(links)
}

#[tauri::command]
fn move_link_target_to_storage(id: String) -> Result<ManagedLink, String> {
    if !is_running_as_admin() {
        return Err("Administrator privileges are required".to_string());
    }

    let config = load_config()?;
    let storage_root = config
        .storage_root
        .ok_or_else(|| "Storage root is not configured".to_string())?;
    let storage = expand_home(&storage_root)?;

    let mut links = load_links()?;
    let Some(index) = links.iter().position(|link| link.id == id) else {
        return Err("Managed link not found".to_string());
    };

    let original = PathBuf::from(&links[index].original_path);
    let current_target = PathBuf::from(&links[index].target_path);
    let next_target = mirrored_target_path(&original, &storage)?;

    if path_is_under(&current_target, &storage) {
        links[index].storage_root = Some(normalize_path_string(&storage));
        links[index].status = status_for(&original, &current_target);
        links[index].last_checked_at = Some(now_secs());
        let updated = links[index].clone();
        save_links(&links)?;
        return Ok(updated);
    }

    if !current_target.exists() {
        return Err("Current target folder does not exist".to_string());
    }
    if !current_target.is_dir() {
        return Err("Current target path must be a folder".to_string());
    }
    if !is_symlink(&original).map_err(|err| err.to_string())? {
        return Err("Original path must be a symbolic link".to_string());
    }
    if next_target.exists() {
        return Err(
            "Mirrored storage target already exists. Automatic merge/overwrite is disabled."
                .to_string(),
        );
    }

    fs::remove_dir(&original).map_err(|err| err.to_string())?;
    if let Err(err) = move_dir_cross_volume(&current_target, &next_target) {
        let _ = symlink_dir(&current_target, &original);
        return Err(err);
    }

    if let Err(err) = symlink_dir(&next_target, &original) {
        let _ = move_dir_cross_volume(&next_target, &current_target);
        let _ = symlink_dir(&current_target, &original);
        return Err(format!(
            "Moved target, but failed to recreate symlink: {err}"
        ));
    }

    links[index].target_path = normalize_path_string(&next_target);
    links[index].storage_root = Some(normalize_path_string(&storage));
    links[index].status = status_for(&original, &next_target);
    links[index].last_checked_at = Some(now_secs());
    let updated = links[index].clone();
    save_links(&links)?;
    Ok(updated)
}

#[tauri::command]
fn relaunch_as_admin() -> Result<(), String> {
    let exe = env::current_exe().map_err(|err| err.to_string())?;
    let operation = wide_null("runas");
    let file = wide_null(exe.as_os_str());
    let result = unsafe {
        ShellExecuteW(
            HWND(null_mut()),
            PCWSTR(operation.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };

    if result.0 as isize <= 32 {
        return Err("Failed to relaunch with administrator privileges".to_string());
    }

    exit(0);
}

#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    let path = expand_home(&path)?;
    let target = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(&path).to_path_buf()
    };
    Command::new("explorer")
        .arg(target)
        .spawn()
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_storage_root,
            preview_replace_folder,
            replace_folder,
            import_existing_link,
            scan_existing_links,
            validate_link,
            remove_from_manager,
            delete_link,
            restore_link_target,
            move_link_target_to_storage,
            relaunch_as_admin,
            reveal_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrors_windows_drive_path_under_storage_root() {
        let original = PathBuf::from(r"C:\Users\User\.code");
        let storage = PathBuf::from(r"D:\LinkStorage");
        let target = mirrored_target_path(&original, &storage).unwrap();
        assert_eq!(target, PathBuf::from(r"D:\LinkStorage\C\Users\User\.code"));
    }

    #[test]
    fn expands_home_prefix() {
        let expanded = expand_home(r"~\.code").unwrap();
        assert!(expanded.ends_with(r".code"));
        assert!(expanded.is_absolute());
    }

    #[test]
    fn json_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("links.json");
        let links = vec![ManagedLink {
            id: "one".to_string(),
            name: ".code".to_string(),
            original_path: r"C:\Users\User\.code".to_string(),
            target_path: r"D:\Store\C\Users\User\.code".to_string(),
            storage_root: Some(r"D:\Store".to_string()),
            kind: LinkKind::DirectorySymlink,
            status: LinkStatus::Ok,
            created_at: 1,
            last_checked_at: Some(2),
        }];

        save_links_to(&file, &links).unwrap();
        let loaded = load_links_from(&file).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].original_path, links[0].original_path);
    }

    #[test]
    fn missing_link_status_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        let target = dir.path().join("target");
        assert_eq!(status_for(&missing, &target), LinkStatus::MissingLink);
    }
}
