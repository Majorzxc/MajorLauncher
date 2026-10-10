//! Вкладки страницы сборки (миры, моды, скриншоты, логи) и статистика.
//! Окно называет сборку и имя файла — пути строит ядро.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use majorlauncher_core::Builds;
use majorlauncher_core::content::{self, FileEntry, Folder, LogFile, World};
use majorlauncher_core::playtime::{Filter, Stats, Summary, summarize};
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

/// Сделать скриншот обложкой сборки: середина кадра, 256×256.
#[tauri::command]
pub async fn build_cover_from_screenshot(
    state: State<'_, AppState>,
    id: String,
    file: String,
) -> CmdResult<()> {
    let builds = Builds::new(state.data()?);
    let dir = game_dir(&state, &id)?;
    blocking(move || {
        let path = content::inside(&dir, Folder::Screenshots, &file).map_err(text)?;
        let png = content::cover_from(&path).map_err(text)?;
        builds.set_cover(&id, &png).map_err(text)
    })
    .await
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
    let dir = game_dir(&state, &id)?;
    let path = match target {
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
        Target::File { folder, file } => content::inside(&dir, folder, &file).map_err(text)?,
        Target::World { folder } => {
            if folder.contains(['/', '\\', ':']) || folder == ".." || folder == "." {
                return Err(format!("недопустимое имя мира «{folder}»"));
            }
            dir.join("saves").join(folder)
        }
        Target::Log { file } => content::log_path(&dir, &file).map_err(text)?,
    };
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(text)
}

#[tauri::command]
pub async fn stats_summary(state: State<'_, AppState>, filter: Filter) -> CmdResult<Summary> {
    let data = state.data()?;
    blocking(move || {
        let records = Stats::new(&data).load().map_err(text)?;
        Ok(summarize(&records, &filter))
    })
    .await
}
