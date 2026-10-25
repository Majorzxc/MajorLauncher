//! Снапшоты сборки: список, «Сделать снапшот», откат, удаление. Снапшот перед
//! изменением сборки делают сами команды изменений через [`before`].

use majorlauncher_core::DataDir;
use majorlauncher_core::snapshots::{self, Info};
use tauri::State;

use crate::builds::blocking;
use crate::play::Phase;
use crate::{AppState, CmdResult, text};

/// Сохранить состояние сборки перед изменением. Не вышло — изменение не
/// делаем: без снапшота его нельзя будет откатить.
pub fn before(data: &DataDir, id: &str, reason: &str) -> CmdResult<()> {
    snapshots::take(data, id, reason, false)
        .map(|_| ())
        .map_err(|e| format!("не удалось сохранить снапшот сборки перед изменением: {e}"))
}

/// То же в фоне — для команд, которые сами идут асинхронно.
pub async fn before_async(data: &DataDir, id: &str, reason: String) -> CmdResult<()> {
    let (data, id) = (data.clone(), id.to_string());
    blocking(move || before(&data, &id, &reason)).await
}

#[tauri::command]
pub async fn snapshots_list(state: State<'_, AppState>, id: String) -> CmdResult<Vec<Info>> {
    let data = state.data()?;
    blocking(move || snapshots::list(&data, &id).map_err(text)).await
}

/// «Сделать снапшот» по кнопке: такой не удаляется сам.
#[tauri::command]
pub async fn snapshot_take(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let data = state.data()?;
    blocking(move || {
        snapshots::take(&data, &id, "сделан вручную", true)
            .map(|_| ())
            .map_err(text)
    })
    .await
}

/// Откатить сборку к снапшоту. Пока сборка запускается или идёт, нельзя:
/// игра держит файлы модов.
#[tauri::command]
pub async fn snapshot_restore(
    state: State<'_, AppState>,
    id: String,
    snapshot: String,
) -> CmdResult<()> {
    let data = state.data()?;
    {
        let play = state.play.lock().unwrap();
        if play.status.phase != Phase::Idle && play.status.build.as_deref() == Some(id.as_str()) {
            return Err("сборка запущена или готовится к запуску — откатите её после игры".into());
        }
    }
    blocking(move || snapshots::restore(&data, &id, &snapshot).map_err(text)).await
}

#[tauri::command]
pub async fn snapshot_delete(
    state: State<'_, AppState>,
    id: String,
    snapshot: String,
) -> CmdResult<()> {
    let data = state.data()?;
    blocking(move || snapshots::delete(&data, &id, &snapshot).map_err(text)).await
}
