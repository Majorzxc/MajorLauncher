//! Фоновые обновления (раздел 5.13 ТЗ): через 45 секунд после запуска и
//! дальше раз в 6 часов лаунчер спрашивает Modrinth, что нового у модов и
//! сборок из каталога. Ставит сам — только если игрок включил автообновление;
//! перед этим снапшот, запущенную сборку не трогает.

use std::collections::HashSet;
use std::time::Duration;

use majorlauncher_core::mrpack::{self, Origin, PackUpdate};
use majorlauncher_core::updates::{self, BuildUpdates, UpdateSettings};
use majorlauncher_core::{Config, Instance, mods};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::play::Phase;
use crate::{AppState, CmdResult, text};

/// Что известно об обновлениях сейчас.
#[derive(Default)]
pub struct Updates {
    pub list: Vec<BuildUpdates>,
    /// Секунды с 1970 года.
    pub checked: Option<u64>,
    pub running: bool,
    /// О чём уже сказали игроку — повторно не уведомляем.
    notified: HashSet<String>,
}

#[derive(Serialize, Clone)]
pub struct UpdatesView {
    list: Vec<BuildUpdates>,
    checked: Option<u64>,
    running: bool,
}

fn view(u: &Updates) -> UpdatesView {
    UpdatesView {
        list: u.list.clone(),
        checked: u.checked,
        running: u.running,
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Запустить фоновую проверку на всё время работы лаунчера.
pub fn start_background(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio_sleep(Duration::from_secs(45)).await;
        loop {
            let check = app
                .state::<AppState>()
                .config
                .lock()
                .unwrap()
                .get()
                .updates
                .check;
            if check {
                run(&app).await;
            }
            tokio_sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}

async fn tokio_sleep(d: Duration) {
    majorlauncher_core::cancel::sleep(d).await;
}

/// Сборка сейчас запускается или идёт — её не трогаем.
fn busy(app: &AppHandle, id: &str) -> bool {
    let state = app.state::<AppState>();
    let play = state.play.lock().unwrap();
    play.status.phase != Phase::Idle && play.status.build.as_deref() == Some(id)
}

/// Проверить все сборки; по настройкам — обновить.
pub async fn run(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut u = state.updates.lock().unwrap();
        if u.running {
            return;
        }
        u.running = true;
    }
    let _ = app.emit("updates", view(&state.updates.lock().unwrap()));
    let Ok(data) = state.data() else {
        state.updates.lock().unwrap().running = false;
        return;
    };
    let settings: UpdateSettings = state.config.lock().unwrap().get().updates.clone();
    let mut list = Vec::new();
    for id in updates::builds_to_check(&data) {
        let Ok(mut found) = updates::check_build(&state.http, &data, &id).await else {
            continue;
        };
        if settings.auto_mods && found.mods > 0 && !busy(app, &id) {
            let task = crate::tasks::start(
                app,
                format!("Автообновление модов ({})", found.mods),
                format!("сборка «{}»", found.name),
            );
            let progress = task.progress();
            let result = async {
                crate::snapshots::before_async(&data, &id, "перед автообновлением модов".into())
                    .await?;
                mods::update(&state.http, &data, &id, &[], &progress)
                    .await
                    .map_err(text)
            }
            .await;
            task.finish(&result);
            if let Ok(titles) = result {
                let _ = app.emit(
                    "updates-applied",
                    format!("Обновлены моды в «{}»: {}", found.name, titles.join(", ")),
                );
                found.mods = 0;
                found.titles.clear();
            }
        }
        if settings.auto_packs
            && let Some(p) = found.pack.clone()
            && !busy(app, &id)
        {
            let task = crate::tasks::start(
                app,
                format!("Обновление сборки до {}", p.version_number),
                format!("сборка «{}»", found.name),
            );
            let progress = task.progress();
            let result = mrpack::update(&state.http, &data, &id, &p.version_id, &progress).await;
            task.finish(&result);
            if result.is_ok() {
                let _ = app.emit(
                    "updates-applied",
                    format!("Сборка «{}» обновлена до {}", found.name, p.version_number),
                );
                found.pack = None;
            }
        }
        if found.mods > 0 || found.pack.is_some() {
            list.push(found);
        }
    }
    // Уведомить о новом — один раз на каждое обновление.
    let fresh: Vec<String> = {
        let mut u = state.updates.lock().unwrap();
        let mut fresh = Vec::new();
        for b in &list {
            let mut keys: Vec<String> = b
                .titles
                .iter()
                .map(|t| format!("{}|{t}", b.build))
                .collect();
            if let Some(p) = &b.pack {
                keys.push(format!("{}|pack|{}", b.build, p.version_id));
            }
            if keys.iter().any(|k| !u.notified.contains(k)) {
                let mut parts = Vec::new();
                if b.mods > 0 {
                    parts.push(format!("модов: {}", b.mods));
                }
                if let Some(p) = &b.pack {
                    parts.push(format!("новая версия сборки {}", p.version_number));
                }
                fresh.push(format!("«{}» — {}", b.name, parts.join(", ")));
            }
            u.notified.extend(keys);
        }
        u.list = list;
        u.checked = Some(now());
        u.running = false;
        fresh
    };
    let _ = app.emit("updates", view(&state.updates.lock().unwrap()));
    if !fresh.is_empty() {
        let _ = app.emit(
            "updates-found",
            format!("Есть обновления: {}", fresh.join("; ")),
        );
    }
}

#[tauri::command]
pub fn updates_list(state: State<AppState>) -> UpdatesView {
    view(&state.updates.lock().unwrap())
}

/// «Проверить сейчас».
#[tauri::command]
pub async fn updates_check(app: AppHandle) -> UpdatesView {
    run(&app).await;
    view(&app.state::<AppState>().updates.lock().unwrap())
}

#[tauri::command]
pub fn set_updates(state: State<AppState>, updates: UpdateSettings) -> CmdResult<Config> {
    let mut config = state.config.lock().unwrap();
    config.set_updates(updates).map_err(text)?;
    Ok(config.get().clone())
}

/// Сборка из каталога: откуда она и есть ли новая версия.
#[derive(Serialize)]
pub struct PackInfo {
    origin: Option<Origin>,
    update: Option<PackUpdate>,
}

#[tauri::command]
pub async fn pack_info(state: State<'_, AppState>, id: String) -> CmdResult<PackInfo> {
    let data = state.data()?;
    let origin = mrpack::origin(&data, &id).filter(|o| o.project_id.is_some());
    let update = match origin {
        Some(_) => mrpack::check_update(&state.http, &data, &id)
            .await
            .map_err(text)?,
        None => None,
    };
    Ok(PackInfo { origin, update })
}

/// «Обновить сборку» до новой версии из каталога.
#[tauri::command]
pub async fn pack_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    version: String,
) -> CmdResult<Instance> {
    let data = state.data()?;
    if busy(&app, &id) {
        return Err("сборка запущена — обновите её после игры".into());
    }
    let name = majorlauncher_core::Builds::new(data.clone())
        .get(&id)
        .map_err(text)?
        .name;
    let task = crate::tasks::start(&app, "Обновление сборки", format!("«{name}»"));
    let progress = task.progress();
    let result = mrpack::update(&state.http, &data, &id, &version, &progress).await;
    task.finish(&result);
    let instance = result.map_err(text)?;
    // Уже не новость — убрать из списка обновлений.
    {
        let mut u = state.updates.lock().unwrap();
        for b in u.list.iter_mut().filter(|b| b.build == id) {
            b.pack = None;
        }
        u.list.retain(|b| b.mods > 0 || b.pack.is_some());
    }
    let _ = app.emit("updates", view(&state.updates.lock().unwrap()));
    Ok(instance)
}
