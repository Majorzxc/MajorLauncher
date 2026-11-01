//! Анализатор ошибок для окна: проблемы последней игры, разбор любого лога
//! по кнопке и страница драйвера видеокарты для кнопки «Обновить драйвер».

use std::collections::HashMap;

use majorlauncher_core::analyzer::{self, Analysis, Session};
use majorlauncher_core::health;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::builds::blocking;
use crate::{AppState, CmdResult, text};

/// Разбор последней игры сборки.
#[tauri::command]
pub fn problems_last(state: State<AppState>, id: String) -> CmdResult<Option<Session>> {
    Ok(analyzer::last_session(&state.data()?, &id))
}

/// Сколько проблем было в последней игре — для значка на плитке.
#[tauri::command]
pub async fn problems_counts(state: State<'_, AppState>) -> CmdResult<HashMap<String, usize>> {
    let data = state.data()?;
    blocking(move || Ok(analyzer::problem_counts(&data))).await
}

/// «Разобрать» лог или отчёт о падении из вкладки «Логи».
#[tauri::command]
pub async fn analyze_log(
    state: State<'_, AppState>,
    id: String,
    file: String,
) -> CmdResult<Analysis> {
    let data = state.data()?;
    analyzer::analyze_file(&state.http, &data, &id, &file)
        .await
        .map_err(text)
}

/// Версия правил разбора и откуда они (встроенные или с GitHub).
#[tauri::command]
pub fn rules_info(state: State<AppState>) -> CmdResult<(u32, bool)> {
    Ok(analyzer::remote::info(&state.data()?))
}

/// Страница драйверов производителя видеокарты (отдельной, если их две).
#[tauri::command]
pub async fn open_driver_page(app: AppHandle) -> CmdResult<()> {
    let hw = blocking(|| Ok(health::hardware())).await?;
    let gpu = hw
        .gpus
        .iter()
        .find(|g| g.discrete)
        .or(hw.gpus.first())
        .map(|g| g.name.to_lowercase())
        .unwrap_or_default();
    let url = if gpu.contains("nvidia") {
        "https://www.nvidia.com/ru-ru/drivers/"
    } else if gpu.contains("amd") || gpu.contains("radeon") || gpu.contains("ati ") {
        "https://www.amd.com/ru/support/download/drivers.html"
    } else if gpu.contains("intel") {
        "https://www.intel.com/content/www/us/en/download-center/home.html"
    } else {
        "https://www.google.com/search?q=%D0%B4%D1%80%D0%B0%D0%B9%D0%B2%D0%B5%D1%80+%D0%B2%D0%B8%D0%B4%D0%B5%D0%BE%D0%BA%D0%B0%D1%80%D1%82%D1%8B"
    };
    app.opener().open_url(url, None::<&str>).map_err(text)
}
