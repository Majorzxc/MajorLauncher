//! Настройки окна: оформление, аккаунт для шапки, папка данных.

use std::path::PathBuf;

use majorlauncher_core::datadir::{self, DirCheck};
use majorlauncher_core::{Accounts, Appearance, Config};
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

#[derive(Serialize)]
pub struct AccountInfo {
    name: String,
    kind: &'static str,
}

/// Аккаунт по умолчанию для плашки в шапке. `None` — папка данных ещё
/// не выбрана или аккаунтов нет.
#[tauri::command]
pub fn current_account(state: State<AppState>) -> CmdResult<Option<AccountInfo>> {
    let Ok(data) = state.data() else {
        return Ok(None);
    };
    let accounts = Accounts::load(&data).map_err(text)?;
    Ok(accounts.default_account().map(|a| AccountInfo {
        name: a.name.clone(),
        kind: a.kind.label(),
    }))
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
