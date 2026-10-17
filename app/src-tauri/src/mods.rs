//! Моды в сборке: отчёт (что за мод, обновления, предупреждения), действия
//! из предупреждений, обновление и закрепление.

use majorlauncher_core::Builds;
use majorlauncher_core::Instance;
use majorlauncher_core::mods::{self, Action, Report};
use tauri::State;

use crate::builds::blocking;
use crate::{AppState, CmdResult, text};

/// Отчёт о модах сборки. `online` — спросить Modrinth (опознание, обновления);
/// без него — только то, что уже известно (быстро, для кнопки «Играть»).
#[tauri::command]
pub async fn mods_report(
    state: State<'_, AppState>,
    id: String,
    online: bool,
) -> CmdResult<Report> {
    let data = state.data()?;
    mods::report(&state.http, &data, &id, online)
        .await
        .map_err(text)
}

/// Выключить, удалить, «доверять», оставить новую копию.
#[tauri::command]
pub async fn mods_apply(state: State<'_, AppState>, id: String, action: Action) -> CmdResult<()> {
    let data = state.data()?;
    blocking(move || mods::apply(&data, &id, &action).map_err(text)).await
}

/// Убрать мод из сборки (свой файл переезжает в библиотеку).
#[tauri::command]
pub async fn mods_remove(state: State<'_, AppState>, id: String, file: String) -> CmdResult<()> {
    let data = state.data()?;
    blocking(move || mods::remove(&data, &id, &file).map_err(text)).await
}

/// Обновить: `only` — пары (версия, старый файл); пусто — всё, кроме закреплённых.
#[tauri::command]
pub async fn mods_update(
    state: State<'_, AppState>,
    id: String,
    only: Vec<(String, String)>,
) -> CmdResult<Vec<String>> {
    let data = state.data()?;
    mods::update(&state.http, &data, &id, &only)
        .await
        .map_err(text)
}

#[tauri::command]
pub fn mods_pin(
    state: State<AppState>,
    id: String,
    project: String,
    pinned: bool,
) -> CmdResult<Instance> {
    Builds::new(state.data()?)
        .pin_mod(&id, &project, pinned)
        .map_err(text)
}
