//! Настройки: оформление, запуск игры, папка данных.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use majorlauncher_core::datadir::{self, DirCheck};
use majorlauncher_core::relocate::{self, Method, Plan};
use majorlauncher_core::{Appearance, Config, GameSettings, content, system};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_opener::OpenerExt;

use crate::builds::blocking;
use crate::play::Phase;
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

/// Картинка фона окна (data URL) или `None`.
#[tauri::command]
pub fn background_image(state: State<AppState>) -> Option<String> {
    let jpeg = state.config.lock().unwrap().background()?;
    Some(format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg)))
}

/// Поставить картинку фона. Ядро уменьшает её под экран `width×height`.
#[tauri::command]
pub async fn set_background_image(
    state: State<'_, AppState>,
    path: String,
    width: u32,
    height: u32,
) -> CmdResult<Config> {
    let jpeg = blocking(move || content::background(Path::new(&path), width, height).map_err(text))
        .await?;
    let mut config = state.config.lock().unwrap();
    config.set_background(&jpeg).map_err(text)?;
    Ok(config.get().clone())
}

#[tauri::command]
pub fn remove_background_image(state: State<AppState>) -> CmdResult<Config> {
    let mut config = state.config.lock().unwrap();
    config.remove_background().map_err(text)?;
    Ok(config.get().clone())
}

/// Текущая папка данных — даже когда она не выбрана, это ошибка.
fn current_dir(state: &AppState) -> CmdResult<PathBuf> {
    Ok(state.data()?.root().to_path_buf())
}

/// Сколько занимает папка данных, байт.
#[tauri::command]
pub async fn data_size(state: State<'_, AppState>) -> CmdResult<u64> {
    let dir = current_dir(&state)?;
    blocking(move || Ok(relocate::size(&dir))).await
}

/// Что будет при переносе в выбранную папку: тот же диск или копия, хватит ли места.
#[tauri::command]
pub async fn data_move_plan(
    state: State<'_, AppState>,
    path: String,
    size: u64,
) -> CmdResult<Plan> {
    let from = current_dir(&state)?;
    blocking(move || {
        let to = relocate::target(Path::new(path.trim()));
        Ok(relocate::plan(&from, &to, size))
    })
    .await
}

#[derive(Serialize)]
pub struct Moved {
    method: Method,
    /// Где остались старые данные (после копии на другой диск).
    old: Option<String>,
    old_size: u64,
}

/// Перенести папку данных. Прогресс — событием `data-move`.
#[tauri::command]
pub async fn data_move(app: AppHandle, state: State<'_, AppState>, to: String) -> CmdResult<Moved> {
    let from = current_dir(&state)?;
    let to = PathBuf::from(to.trim());
    let cancel = {
        let mut r = state.relocation.lock().unwrap();
        if r.active {
            return Err("перенос уже идёт".into());
        }
        r.active = true;
        r.old = None;
        r.cancel = Arc::new(AtomicBool::new(false));
        r.cancel.clone()
    };
    // Игра проверяется после пометки «идёт перенос»: запуск проверяет
    // перенос под замком игры, и одновременно они не начнутся.
    if state.play.lock().unwrap().status.phase != Phase::Idle {
        state.relocation.lock().unwrap().active = false;
        return Err("сначала закройте игру и дождитесь конца загрузки".into());
    }

    let events = app.clone();
    let (src, dst) = (from.clone(), to.clone());
    let result = blocking(move || {
        let mut last: Option<Instant> = None;
        relocate::relocate(&src, &dst, &cancel, &mut |p| {
            // Окну хватает десяти обновлений в секунду.
            if last.is_none_or(|t| t.elapsed() >= Duration::from_millis(100)) || p.done == p.total {
                last = Some(Instant::now());
                let _ = events.emit("data-move", p);
            }
        })
        .map_err(text)
    })
    .await;

    let state = app.state::<AppState>();
    let finish = |old: Option<PathBuf>| {
        let mut r = state.relocation.lock().unwrap();
        r.active = false;
        r.old = old;
    };
    let method = match result {
        Ok(method) => method,
        Err(e) => {
            finish(None);
            return Err(e);
        }
    };
    // Данные уже на новом месте — запоминаем его. Если даже это не вышло,
    // возвращаем как было, чтобы лаунчер не смотрел в пустоту.
    if let Err(e) = state.config.lock().unwrap().set_data_dir(to.clone()) {
        match method {
            Method::Renamed => {
                let _ = std::fs::rename(&to, &from);
            }
            Method::Copied => {
                let _ = std::fs::remove_dir_all(&to);
            }
        }
        finish(None);
        return Err(format!(
            "не удалось сохранить новый путь, всё возвращено как было: {e}"
        ));
    }
    let old = (method == Method::Copied).then_some(from);
    let old_size = match &old {
        Some(dir) => {
            let dir = dir.clone();
            blocking(move || Ok(relocate::size(&dir)))
                .await
                .unwrap_or(0)
        }
        None => 0,
    };
    let moved = Moved {
        method,
        old: old.as_ref().map(|p| p.display().to_string()),
        old_size,
    };
    finish(old);
    Ok(moved)
}

#[tauri::command]
pub fn data_move_cancel(state: State<AppState>) {
    state
        .relocation
        .lock()
        .unwrap()
        .cancel
        .store(true, Ordering::Relaxed);
}

/// Удалить старую папку после копирования — по кнопке игрока. Путь знает
/// только лаунчер: окно не может попросить удалить что-то другое.
#[tauri::command]
pub async fn data_remove_old(state: State<'_, AppState>) -> CmdResult<()> {
    let old = state
        .relocation
        .lock()
        .unwrap()
        .old
        .clone()
        .ok_or("старой папки нет")?;
    let current = current_dir(&state)?;
    blocking(move || relocate::remove_old(&old, &current).map_err(text)).await?;
    state.relocation.lock().unwrap().old = None;
    Ok(())
}
