//! Импорт сборок из других лаунчеров: поиск на ПК, папка по выбору игрока,
//! угадывание версии по модам и сам импорт (копия — чужие файлы не меняются).

use std::path::PathBuf;

use majorlauncher_core::Instance;
use majorlauncher_core::importer::{self, Found, Request};
use tauri::{AppHandle, State};

use crate::builds::blocking;
use crate::{AppState, CmdResult, text};

/// Сборки других лаунчеров на этом ПК.
#[tauri::command]
pub async fn import_scan() -> CmdResult<Vec<Found>> {
    blocking(|| Ok(importer::scan())).await
}

/// Папка, которую выбрал игрок.
#[tauri::command]
pub async fn import_inspect(path: String) -> CmdResult<Vec<Found>> {
    blocking(move || importer::inspect(&PathBuf::from(path.trim())).map_err(text)).await
}

/// Версия и загрузчик по модам — для сборок без описания (Modrinth App, папка).
#[tauri::command]
pub async fn import_guess(state: State<'_, AppState>, found: Found) -> CmdResult<Found> {
    let data = state.data()?;
    let mut found = found;
    importer::guess(&state.http, &data, &mut found)
        .await
        .map_err(text)?;
    Ok(found)
}

#[tauri::command]
pub async fn import_run(
    app: AppHandle,
    state: State<'_, AppState>,
    request: Request,
) -> CmdResult<Instance> {
    let data = state.data()?;
    let task = crate::tasks::start(
        &app,
        format!("Импорт «{}»", request.name),
        "в сборки".to_string(),
    );
    let stage = {
        let task = task.clone();
        move |s: &str| task.stage(s)
    };
    let result = importer::import(&state.http, &data, &request, &stage).await;
    task.finish(&result);
    result.map_err(text)
}
