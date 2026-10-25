//! Экспорт сборки в `.mrpack`: что получится и запись файла, который
//! выбрал игрок в окне «Сохранить как».

use std::path::PathBuf;

use majorlauncher_core::export::{self, Exported, Options, Preview};
use tauri::State;

use crate::{AppState, CmdResult, text};

#[tauri::command]
pub async fn export_preview(state: State<'_, AppState>, id: String) -> CmdResult<Preview> {
    let data = state.data()?;
    export::preview(&state.http, &data, &id).await.map_err(text)
}

#[tauri::command]
pub async fn export_build(
    state: State<'_, AppState>,
    id: String,
    options: Options,
    path: String,
) -> CmdResult<Exported> {
    let data = state.data()?;
    let mut path = PathBuf::from(path.trim());
    if path
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("mrpack"))
    {
        path.set_extension("mrpack");
    }
    export::export(&state.http, &data, &id, &options, &path)
        .await
        .map_err(text)
}
