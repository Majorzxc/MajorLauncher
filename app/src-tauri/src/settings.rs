//! Настройки: оформление, запуск игры, папка данных.

use std::path::PathBuf;

use majorlauncher_core::datadir::{self, DirCheck};
use majorlauncher_core::{Appearance, Config, GameSettings, system};
use serde::Serialize;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_opener::OpenerExt;

use crate::{AppState, CmdResult, paint, text};

#[tauri::command]
pub fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().get().clone()
}

#[tauri::command]
pub fn set_appearance(
    window: WebviewWindow,
    state: State<AppState>,
    appearance: Appearance,
) -> CmdResult<Config> {
    let mut config = state.config.lock().unwrap();
    config.set_appearance(appearance).map_err(text)?;
    paint(&window, config.get());
    Ok(config.get().clone())
}

#[tauri::command]
pub fn set_game(state: State<AppState>, game: GameSettings) -> CmdResult<Config> {
    let mut config = state.config.lock().unwrap();
    config.set_game(game).map_err(text)?;
    Ok(config.get().clone())
}

#[derive(Serialize)]
pub struct Memory {
    total_mb: Option<u64>,
    /// Сколько даст режим «авто».
    auto_mb: u32,
}

/// Объём ОЗУ и память «авто» — для ползунка памяти.
#[tauri::command]
pub fn system_memory() -> Memory {
    let total_mb = system::total_ram_mb();
    Memory {
        total_mb,
        auto_mb: system::auto_memory_mb(total_mb),
    }
}

/// Куда предложить положить папку данных при первом запуске — с проверкой.
#[tauri::command]
pub async fn data_dir_suggestion() -> DirCheck {
    tauri::async_runtime::spawn_blocking(|| datadir::inspect(&datadir::suggest()))
        .await
        .expect("проверка папки не паникует")
}

#[tauri::command]
pub async fn inspect_dir(path: String) -> DirCheck {
    tauri::async_runtime::spawn_blocking(move || datadir::inspect(&PathBuf::from(path)))
        .await
        .expect("проверка папки не паникует")
}

#[tauri::command]
pub fn set_data_dir(state: State<AppState>, path: String) -> CmdResult<Config> {
    let mut config = state.config.lock().unwrap();
    config.set_data_dir(PathBuf::from(path)).map_err(text)?;
    Ok(config.get().clone())
}

/// Открыть папку данных в проводнике. Путь знает ядро, окно его не передаёт.
#[tauri::command]
pub fn open_data_dir(app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    let data = state.data()?;
    app.opener()
        .open_path(data.root().display().to_string(), None::<&str>)
        .map_err(text)
}
