//! Вкладки страницы сборки (миры, моды, скриншоты, логи) и статистика.
//! Окно называет сборку и имя файла — пути строит ядро.

use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use majorlauncher_core::Builds;
use majorlauncher_core::content::{self, FileEntry, Folder, LogFile, LogText, World};
use majorlauncher_core::playtime::{Filter, Stats, Summary, resolve_worlds, summarize};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::builds::blocking;
use crate::{AppState, CmdResult, text};

fn game_dir(state: &State<AppState>, id: &str) -> CmdResult<std::path::PathBuf> {
    Builds::new(state.data()?).game_dir(id).map_err(text)
}

#[tauri::command]
pub async fn build_files(
    state: State<'_, AppState>,
    id: String,
    folder: Folder,
) -> CmdResult<Vec<FileEntry>> {
    let dir = game_dir(&state, &id)?;
    blocking(move || content::files(&dir, folder).map_err(text)).await
}

/// Добавить в сборку файлы, выбранные игроком. Возвращает ошибки по файлам.
#[tauri::command]
pub async fn build_add_files(
    state: State<'_, AppState>,
    id: String,
    folder: Folder,
    paths: Vec<String>,
) -> CmdResult<Vec<String>> {
    let dir = game_dir(&state, &id)?;
    let data = state.data()?;
    let paths: Vec<std::path::PathBuf> = paths.into_iter().map(Into::into).collect();
    blocking(move || {
        if folder == Folder::Mods {
            crate::snapshots::before(&data, &id, "перед добавлением своих файлов")?;
        }
        Ok(content::add_files(&dir, folder, &paths))
    })
    .await
}

#[tauri::command]
pub fn build_set_enabled(
    state: State<AppState>,
    id: String,
    folder: Folder,
    file: String,
    enabled: bool,
) -> CmdResult<String> {
    content::set_enabled(&game_dir(&state, &id)?, folder, &file, enabled).map_err(text)
}

#[tauri::command]
pub async fn build_worlds(state: State<'_, AppState>, id: String) -> CmdResult<Vec<World>> {
    let dir = game_dir(&state, &id)?;
    blocking(move || content::worlds(&dir).map_err(text)).await
}

#[tauri::command]
pub async fn build_logs(state: State<'_, AppState>, id: String) -> CmdResult<Vec<LogFile>> {
    let dir = game_dir(&state, &id)?;
    blocking(move || content::logs(&dir).map_err(text)).await
}

/// Миниатюра скриншота как data URL.
#[tauri::command]
pub async fn build_thumbnail(
    state: State<'_, AppState>,
    id: String,
    file: String,
) -> CmdResult<String> {
    let data = state.data()?;
    let dir = game_dir(&state, &id)?;
    blocking(move || {
        let png = content::thumbnail(&data, &dir, &file).map_err(text)?;
        Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
    })
    .await
}

/// Скриншот для окна обрезки обложки (уменьшенный).
#[tauri::command]
pub async fn build_screenshot(
    state: State<'_, AppState>,
    id: String,
    file: String,
) -> CmdResult<String> {
    let dir = game_dir(&state, &id)?;
    blocking(move || {
        let picture = content::screenshot_preview(&dir, &file).map_err(text)?;
        Ok(crate::builds::data_url(&picture))
    })
    .await
}

/// Скриншот целиком — для просмотра в окне.
#[tauri::command]
pub async fn build_screenshot_full(
    state: State<'_, AppState>,
    id: String,
    file: String,
) -> CmdResult<String> {
    let dir = game_dir(&state, &id)?;
    blocking(move || {
        let picture = content::screenshot_full(&dir, &file).map_err(text)?;
        Ok(crate::builds::data_url(&picture))
    })
    .await
}

/// Убрать скриншот в корзину Windows.
#[tauri::command]
pub async fn build_trash_screenshot(
    state: State<'_, AppState>,
    id: String,
    file: String,
) -> CmdResult<()> {
    let dir = game_dir(&state, &id)?;
    blocking(move || content::trash_screenshot(&dir, &file).map_err(text)).await
}

/// Текст лога или краш-отчёта для просмотра в окне.
#[tauri::command]
pub async fn build_log_text(
    state: State<'_, AppState>,
    id: String,
    file: String,
) -> CmdResult<LogText> {
    let dir = game_dir(&state, &id)?;
    blocking(move || content::read_log(&dir, &file).map_err(text)).await
}

/// Показать файл в проводнике (выделенным).
#[tauri::command]
pub fn build_reveal(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    target: Target,
) -> CmdResult<()> {
    let path = target_path(&game_dir(&state, &id)?, target)?;
    app.opener().reveal_item_in_dir(path).map_err(text)
}

/// Что открыть в проводнике или в программе по умолчанию.
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Target {
    /// Папка внутри сборки: mods, saves, screenshots…
    Folder {
        name: String,
    },
    /// Файл из папки сборки (скриншот).
    File {
        folder: Folder,
        file: String,
    },
    World {
        folder: String,
    },
    Log {
        file: String,
    },
}

#[tauri::command]
pub fn build_open(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    target: Target,
) -> CmdResult<()> {
    let path = target_path(&game_dir(&state, &id)?, target)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(text)
}

fn target_path(dir: &std::path::Path, target: Target) -> CmdResult<std::path::PathBuf> {
    Ok(match target {
        Target::Folder { name } => {
            let allowed = [
                "mods",
                "resourcepacks",
                "shaderpacks",
                "screenshots",
                "saves",
                "logs",
            ];
            if !allowed.contains(&name.as_str()) {
                return Err(format!("папку «{name}» открыть нельзя"));
            }
            let path = dir.join(&name);
            std::fs::create_dir_all(&path).map_err(text)?;
            path
        }
        Target::File { folder, file } => content::inside(dir, folder, &file).map_err(text)?,
        Target::World { folder } => {
            if folder.contains(['/', '\\', ':']) || folder == ".." || folder == "." {
                return Err(format!("недопустимое имя мира «{folder}»"));
            }
            dir.join("saves").join(folder)
        }
        Target::Log { file } => content::log_path(dir, &file).map_err(text)?,
    })
}

#[tauri::command]
pub async fn stats_summary(state: State<'_, AppState>, filter: Filter) -> CmdResult<Summary> {
    let data = state.data()?;
    blocking(move || {
        let mut records = Stats::new(&data).load().map_err(text)?;
        // Миры сборок читаются один раз на сборку: названия из level.dat.
        let mut cache: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut worlds = |build: &str| {
            cache
                .entry(build.to_string())
                .or_insert_with(|| content::world_names(&data.instance(build)))
                .clone()
        };
        resolve_worlds(&mut records, &mut worlds);
        let mut summary = summarize(&records, &filter);
        summary.rename_worlds(&mut worlds);
        Ok(summary)
    })
    .await
}
