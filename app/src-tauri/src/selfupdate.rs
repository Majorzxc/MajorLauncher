//! Обновление самого лаунчера (раздел 5.13 ТЗ): проверка вместе с фоновой
//! проверкой обновлений и по кнопке, загрузка — сама, если игрок не выключил,
//! установка — только по кнопке и не во время игры. Подпись, версию и хеш
//! проверяет ядро (`majorlauncher_core::selfupdate`).

use std::path::PathBuf;

use majorlauncher_core::selfupdate::{self, Available};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{AppState, CmdResult, text};

#[derive(Default)]
pub struct LauncherUpdate {
    available: Option<Available>,
    /// Скачанный и проверенный установщик.
    ready: Option<PathBuf>,
    checked: Option<u64>,
    busy: bool,
    error: Option<String>,
    /// О какой версии уже сказали — повторно не уведомляем.
    notified: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct LauncherView {
    current: &'static str,
    available: Option<Available>,
    ready: bool,
    checked: Option<u64>,
    busy: bool,
    error: Option<String>,
}

fn view(u: &LauncherUpdate) -> LauncherView {
    LauncherView {
        current: majorlauncher_core::VERSION,
        available: u.available.clone(),
        ready: u.ready.is_some(),
        checked: u.checked,
        busy: u.busy,
        error: u.error.clone(),
    }
}

fn emit(app: &AppHandle) {
    let v = view(&app.state::<AppState>().launcher.lock().unwrap());
    let _ = app.emit("launcher-update", v);
}

/// Куда класть установщик: кэш папки данных (всё тяжёлое — там), без неё —
/// временная папка Windows.
fn dir(state: &AppState) -> PathBuf {
    match state.data() {
        Ok(data) => data.cache().join("launcher-update"),
        Err(_) => std::env::temp_dir().join("MajorLauncher-update"),
    }
}

/// Проверить и, если так настроено, скачать. Ошибки сети — в `error`, не тостом:
/// фоновой проверке нечего кричать, что GitHub не ответил.
pub async fn run(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut u = state.launcher.lock().unwrap();
        if u.busy {
            return;
        }
        u.busy = true;
        u.error = None;
    }
    emit(app);
    let settings = state.config.lock().unwrap().get().updates.clone();
    let found = selfupdate::check(&state.http, settings.channel, majorlauncher_core::VERSION).await;
    let dir = dir(&state);
    let mut fresh = None;
    {
        let mut u = state.launcher.lock().unwrap();
        u.checked = Some(crate::updates::now());
        match found {
            Ok(a) => {
                u.ready = a.as_ref().and_then(|a| selfupdate::ready(&dir, a));
                if let Some(a) = &a
                    && u.notified.as_deref() != Some(&a.version)
                {
                    u.notified = Some(a.version.clone());
                    fresh = Some(a.version.clone());
                }
                u.available = a;
            }
            Err(e) => u.error = Some(text(e)),
        }
    }
    let download = settings.launcher_download && state.launcher.lock().unwrap().ready.is_none();
    if download {
        fetch(app).await;
    }
    state.launcher.lock().unwrap().busy = false;
    emit(app);
    if let Some(v) = fresh {
        let ready = state.launcher.lock().unwrap().ready.is_some();
        let _ = app.emit(
            "launcher-update-found",
            if ready {
                format!("MajorLauncher {v} скачан — установится по кнопке, за пару секунд")
            } else {
                format!("Вышел MajorLauncher {v}")
            },
        );
    }
}

/// Скачать установщик найденной версии (задача видна в «Загрузках»).
async fn fetch(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let Some(a) = state.launcher.lock().unwrap().available.clone() else {
        return false;
    };
    let task = crate::tasks::start(
        app,
        format!("Обновление MajorLauncher {}", a.version),
        "лаунчер",
    );
    let result = selfupdate::download(&state.http, &dir(&state), &a).await;
    task.finish(&result);
    let mut u = state.launcher.lock().unwrap();
    match result {
        Ok(path) => {
            u.ready = Some(path);
            true
        }
        Err(e) => {
            u.error = Some(text(e));
            false
        }
    }
}

#[tauri::command]
pub fn launcher_update_view(state: State<AppState>) -> LauncherView {
    view(&state.launcher.lock().unwrap())
}

#[tauri::command]
pub async fn launcher_update_check(app: AppHandle) -> LauncherView {
    run(&app).await;
    view(&app.state::<AppState>().launcher.lock().unwrap())
}

/// «Обновить»: скачать, если ещё нет, запустить установщик и закрыться —
/// установщик сам откроет новую версию.
#[tauri::command]
pub async fn launcher_update_install(app: AppHandle) -> CmdResult<()> {
    let state = app.state::<AppState>();
    if state.play.lock().unwrap().game_alive() {
        return Err(
            "сначала закройте игру: установщик закрывает лаунчер, а с ним и учёт игры".into(),
        );
    }
    let ready = state.launcher.lock().unwrap().ready.clone();
    let path = match ready {
        Some(p) => p,
        None => {
            {
                let mut u = state.launcher.lock().unwrap();
                if u.busy {
                    return Err("обновление ещё скачивается — подождите".into());
                }
                u.busy = true;
            }
            emit(&app);
            let ok = fetch(&app).await;
            state.launcher.lock().unwrap().busy = false;
            emit(&app);
            if !ok {
                let error = state.launcher.lock().unwrap().error.clone();
                return Err(error.unwrap_or_else(|| "нет обновления для установки".into()));
            }
            state
                .launcher
                .lock()
                .unwrap()
                .ready
                .clone()
                .expect("скачано")
        }
    };
    let available = state
        .launcher
        .lock()
        .unwrap()
        .available
        .clone()
        .ok_or("нет обновления для установки")?;
    selfupdate::install(&path, &available).map_err(text)?;
    app.exit(0);
    Ok(())
}
