//! Перевод сборки на другую версию игры: план (что станет с модами и мирами)
//! и создание новой сборки по выбору игрока. Старая сборка не меняется.

use majorlauncher_core::upgrade::{self, Choice, Plan};
use majorlauncher_core::{Builds, Instance};
use tauri::{AppHandle, State};

use crate::play::Phase;
use crate::{AppState, CmdResult, text};

#[tauri::command]
pub async fn upgrade_plan(
    state: State<'_, AppState>,
    id: String,
    version: String,
) -> CmdResult<Plan> {
    let data = state.data()?;
    upgrade::plan(&state.http, &data, &id, &version)
        .await
        .map_err(text)
}

/// Создать сборку на новой версии. Прогресс — в «Загрузках».
#[tauri::command]
pub async fn upgrade_run(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    choice: Choice,
) -> CmdResult<Instance> {
    let data = state.data()?;
    {
        let play = state.play.lock().unwrap();
        if play.status.phase != Phase::Idle && play.status.build.as_deref() == Some(id.as_str()) {
            return Err(
                "сборка запущена — закройте игру: миры копируются, пока игра их не пишет".into(),
            );
        }
    }
    let old = Builds::new(data.clone()).get(&id).map_err(text)?;
    let task = crate::tasks::start(
        &app,
        format!("Перевод на {}", choice.version),
        format!("из сборки «{}»", old.name),
    );
    let progress = task.progress();
    let stage = {
        let task = task.clone();
        move |s: &str| task.stage(s)
    };
    let result = upgrade::run(&state.http, &data, &id, &choice, &progress, &stage).await;
    task.finish(&result);
    result.map_err(text)
}
