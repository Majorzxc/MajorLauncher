//! Моды в сборке: отчёт (что за мод, обновления, предупреждения), действия
//! из предупреждений, обновление и закрепление.

use majorlauncher_core::Builds;
use majorlauncher_core::Instance;
use majorlauncher_core::catalog::Target;
use majorlauncher_core::mods::{self, Action, Report};
use tauri::{AppHandle, State};

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
    let mut report = mods::report(&state.http, &data, &id, online)
        .await
        .map_err(text)?;
    // Системные предупреждения (место, память, видеокарта) — в тот же список.
    let game = state.config.lock().unwrap().get().game.clone();
    let enabled = report.mods.iter().filter(|m| m.enabled).count();
    let system =
        blocking(move || Ok(crate::optimize::system_issues(&data, &id, &game, enabled))).await?;
    report.issues.extend(system);
    report.issues.sort_by_key(|i| i.level);
    Ok(report)
}

/// Выключить, удалить, «доверять», оставить новую копию.
#[tauri::command]
pub async fn mods_apply(state: State<'_, AppState>, id: String, action: Action) -> CmdResult<()> {
    let data = state.data()?;
    blocking(move || {
        let reason = match &action {
            Action::Disable { file } => Some(format!("перед выключением {file}")),
            Action::Delete { file } => Some(format!("перед удалением {file}")),
            Action::DisableExtra { .. } => Some("перед выключением лишних копий модов".into()),
            // «Доверять» файлы не меняет.
            _ => None,
        };
        if let Some(reason) = reason {
            crate::snapshots::before(&data, &id, &reason)?;
        }
        mods::apply(&data, &id, &action).map_err(text)
    })
    .await
}

/// Убрать мод из сборки (свой файл переезжает в библиотеку).
#[tauri::command]
pub async fn mods_remove(state: State<'_, AppState>, id: String, file: String) -> CmdResult<()> {
    let data = state.data()?;
    blocking(move || {
        crate::snapshots::before(&data, &id, &format!("перед удалением {file}"))?;
        mods::remove(&data, &id, &file).map_err(text)
    })
    .await
}

/// Обновить: `only` — пары (версия, старый файл); пусто — всё, кроме закреплённых.
#[tauri::command]
pub async fn mods_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    only: Vec<(String, String)>,
    label: Option<String>,
) -> CmdResult<Vec<String>> {
    let data = state.data()?;
    let label = label.unwrap_or_else(|| "Обновление модов".into());
    crate::snapshots::before_async(&data, &id, format!("перед обновлением: {label}")).await?;
    let place = crate::catalog::place(&data, &Target::Build { id: id.clone() });
    let task = crate::tasks::start(&app, label, place);
    let progress = task.progress();
    let result = mods::update(&state.http, &data, &id, &only, &progress).await;
    task.finish(&result);
    result.map_err(text)
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
