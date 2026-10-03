//! Сборки: список, создание, изменение, корзина, архив, обложки.
//! Окно передаёт только идентификаторы сборок — пути строит ядро.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use majorlauncher_core::builds::{Entry, Patch, State as BuildState, TrashEntry};
use majorlauncher_core::{Builds, Instance, install};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::{AppState, CmdResult, text};

fn builds(state: &State<AppState>) -> CmdResult<Builds> {
    Ok(Builds::new(state.data()?))
}

/// Запустить тяжёлую работу с диском в отдельном потоке, чтобы окно не подвисало.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("операция прервана: {e}"))?
}

#[derive(Serialize)]
pub struct VersionInfo {
    id: String,
    kind: String,
    date: String,
}

/// Версии игры для мастера создания сборки (сеть; без сети — сохранённый список).
#[tauri::command]
pub async fn versions_list(
    state: State<'_, AppState>,
    snapshots: bool,
) -> CmdResult<Vec<VersionInfo>> {
    let data = state.data()?;
    let manifest = install::fetch_manifest(&data, &state.http)
        .await
        .map_err(text)?;
    Ok(manifest
        .versions
        .into_iter()
        .filter(|v| v.kind == "release" || (snapshots && v.kind == "snapshot"))
        .map(|v| VersionInfo {
            date: v.release_time.chars().take(10).collect(),
            id: v.id,
            kind: v.kind,
        })
        .collect())
}

#[tauri::command]
pub async fn builds_list(state: State<'_, AppState>) -> CmdResult<Vec<Entry>> {
    let b = builds(&state)?;
    blocking(move || b.list().map_err(text)).await
}

#[tauri::command]
pub fn builds_groups(state: State<AppState>) -> CmdResult<Vec<String>> {
    builds(&state)?.groups().map_err(text)
}

#[tauri::command]
pub fn build_create(
    state: State<AppState>,
    name: String,
    version: String,
    group: Option<String>,
) -> CmdResult<Instance> {
    builds(&state)?
        .create(&name, &version, group.as_deref())
        .map_err(text)
}

#[tauri::command]
pub fn build_update(state: State<AppState>, id: String, patch: Patch) -> CmdResult<Instance> {
    builds(&state)?.update(&id, patch).map_err(text)
}

#[tauri::command]
pub async fn build_duplicate(state: State<'_, AppState>, id: String) -> CmdResult<Instance> {
    let b = builds(&state)?;
    blocking(move || b.duplicate(&id).map_err(text)).await
}

#[tauri::command]
pub async fn build_trash(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let b = builds(&state)?;
    blocking(move || b.trash(&id).map_err(text)).await
}

#[tauri::command]
pub async fn build_archive(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let b = builds(&state)?;
    blocking(move || b.archive(&id).map_err(text)).await
}

#[tauri::command]
pub async fn build_unarchive(state: State<'_, AppState>, id: String) -> CmdResult<Instance> {
    let b = builds(&state)?;
    blocking(move || b.unarchive(&id).map_err(text)).await
}

/// Размер папки сборки — считается отдельно, чтобы список открывался сразу.
#[tauri::command]
pub async fn build_size(state: State<'_, AppState>, id: String) -> CmdResult<u64> {
    let b = builds(&state)?;
    blocking(move || b.size(&id).map_err(text)).await
}

#[tauri::command]
pub fn build_open_folder(app: AppHandle, state: State<AppState>, id: String) -> CmdResult<()> {
    let data = state.data()?;
    builds(&state)?.get(&id).map_err(text)?;
    app.opener()
        .open_path(data.instance(&id).display().to_string(), None::<&str>)
        .map_err(text)
}

/// Обложка как data URL — сразу подставляется в `<img>`.
#[tauri::command]
pub fn build_cover(
    state: State<AppState>,
    id: String,
    archived: bool,
) -> CmdResult<Option<String>> {
    let kind = if archived {
        BuildState::Archived
    } else {
        BuildState::Active
    };
    Ok(builds(&state)?
        .cover(&id, kind)
        .map_err(text)?
        .map(|png| format!("data:image/png;base64,{}", STANDARD.encode(png))))
}

/// Сохранить обложку: окно уже уменьшило картинку до 256×256 и прислало PNG.
#[tauri::command]
pub fn build_set_cover(state: State<AppState>, id: String, png_base64: String) -> CmdResult<()> {
    let png = STANDARD
        .decode(png_base64)
        .map_err(|_| "обложка повреждена".to_string())?;
    builds(&state)?.set_cover(&id, &png).map_err(text)
}

#[tauri::command]
pub fn build_remove_cover(state: State<AppState>, id: String) -> CmdResult<()> {
    builds(&state)?.remove_cover(&id).map_err(text)
}

/// Прочитать картинку, выбранную игроком в диалоге, чтобы окно её уменьшило.
/// Только картинки и не больше 15 МБ.
#[tauri::command]
pub async fn read_image(path: String) -> CmdResult<String> {
    blocking(move || {
        let lower = path.to_lowercase();
        let mime = [
            (".png", "image/png"),
            (".jpg", "image/jpeg"),
            (".jpeg", "image/jpeg"),
            (".webp", "image/webp"),
            (".gif", "image/gif"),
            (".bmp", "image/bmp"),
        ]
        .into_iter()
        .find(|(ext, _)| lower.ends_with(ext))
        .map(|(_, m)| m)
        .ok_or("подойдёт картинка PNG, JPG, WebP, GIF или BMP")?;
        let size = std::fs::metadata(&path).map_err(text)?.len();
        if size > 15 * 1024 * 1024 {
            return Err("картинка больше 15 МБ — выберите поменьше".into());
        }
        let bytes = std::fs::read(&path).map_err(text)?;
        Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
    })
    .await
}

#[tauri::command]
pub fn group_add(state: State<AppState>, name: String) -> CmdResult<()> {
    builds(&state)?.add_group(&name).map_err(text)
}

#[tauri::command]
pub fn group_delete(state: State<AppState>, name: String) -> CmdResult<()> {
    builds(&state)?.delete_group(&name).map_err(text)
}

#[tauri::command]
pub async fn trash_list(state: State<'_, AppState>) -> CmdResult<Vec<TrashEntry>> {
    let b = builds(&state)?;
    blocking(move || b.trash_list().map_err(text)).await
}

#[tauri::command]
pub async fn trash_restore(state: State<'_, AppState>, trash_id: String) -> CmdResult<Instance> {
    let b = builds(&state)?;
    blocking(move || b.restore(&trash_id).map_err(text)).await
}

#[tauri::command]
pub async fn trash_purge(state: State<'_, AppState>, trash_id: String) -> CmdResult<()> {
    let b = builds(&state)?;
    blocking(move || b.purge(&trash_id).map_err(text)).await
}
