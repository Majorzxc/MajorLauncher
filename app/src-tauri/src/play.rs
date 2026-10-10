//! Запуск игры из окна. Ядро готовит и запускает игру ([`game`]), здесь —
//! только связь с окном: полоса прогресса, окно прячется на время игры и
//! открывается снова после неё (принцип 4 ТЗ).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use majorlauncher_core::builds::QUICK_ID;
use majorlauncher_core::game::{self, Game};
use majorlauncher_core::options::GameVersion;
use majorlauncher_core::playtime::Stats;
use majorlauncher_core::{Builds, DataDir, Event, Instance, OnLaunch, Progress, install};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{AppState, CmdResult, open_window, text};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Idle,
    /// Проверяем файлы и аккаунт.
    Preparing,
    Downloading,
    /// Java запущена, ждём окно игры.
    Starting,
    Running,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PlayStatus {
    pub build: Option<String>,
    pub phase: Phase,
    /// Скачано и всего, байт (во время загрузки).
    pub done: u64,
    pub total: u64,
}

/// Чем закончилась игра — окно показывает это один раз.
#[derive(Debug, Clone, Serialize)]
pub struct Finished {
    build: String,
    name: String,
    seconds: u64,
    code: Option<i32>,
    crashed: bool,
    /// Окно игры так и не появилось — упала при загрузке.
    no_window: bool,
    /// Последние строки вывода игры при краше.
    tail: Vec<String>,
    /// Что пошло не так у самого лаунчера (не удалось записать статистику).
    warning: Option<String>,
}

#[derive(Default)]
pub struct PlayState {
    pub status: PlayStatus,
    /// Итог игры, пока окно было закрыто: заберёт открывшееся окно.
    pub finished: Option<Finished>,
}

impl PlayState {
    /// Идёт ли игра — тогда закрытие окна не завершает лаунчер.
    pub fn game_alive(&self) -> bool {
        matches!(self.status.phase, Phase::Starting | Phase::Running)
    }
}

fn update(app: &AppHandle, f: impl FnOnce(&mut PlayStatus)) {
    let state = app.state::<AppState>();
    let status = {
        let mut play = state.play.lock().unwrap();
        f(&mut play.status);
        play.status.clone()
    };
    let _ = app.emit("play", status);
}

fn reset(app: &AppHandle) {
    update(app, |s| *s = PlayStatus::default());
}

#[derive(Serialize)]
pub struct StatusWithResult {
    status: PlayStatus,
    finished: Option<Finished>,
}

/// Состояние для только что открытого окна. Итог прошлой игры отдаётся один раз.
#[tauri::command]
pub fn play_status(state: State<AppState>) -> StatusWithResult {
    let mut play = state.play.lock().unwrap();
    StatusWithResult {
        status: play.status.clone(),
        finished: play.finished.take(),
    }
}

#[tauri::command]
pub async fn build_play(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    version: Option<String>,
) -> CmdResult<()> {
    let data = state.data()?;
    let builds = Builds::new(data.clone());
    // «Быстрая игра» запускается с версией, выбранной у кнопки.
    let instance = match &version {
        Some(v) if id == QUICK_ID => builds.quick(v),
        _ => builds.get(&id),
    }
    .map_err(text)?;
    let settings = state.config.lock().unwrap().get().game.clone();
    {
        let mut play = state.play.lock().unwrap();
        if play.status.phase != Phase::Idle {
            return Err("игра уже запущена или готовится к запуску".into());
        }
        // Проверка под замком игры: перенос папки проверяет игру тоже под ним.
        if state.relocating() {
            return Err("идёт перенос папки данных — подождите, пока он закончится".into());
        }
        play.status = PlayStatus {
            build: Some(id.clone()),
            phase: Phase::Preparing,
            ..Default::default()
        };
    }
    let _ = app.emit("play", state.play.lock().unwrap().status.clone());

    let prepared = game::prepare(
        &data,
        &state.http,
        &instance,
        &settings,
        progress(app.clone()),
    )
    .await;
    let launch = match prepared {
        Ok(launch) => launch,
        Err(e) => {
            reset(&app);
            return Err(text(e));
        }
    };
    update(&app, |s| s.phase = Phase::Starting);

    // Миры «Быстрой игры» из более новой версии — в копию перед запуском.
    let risky = if instance.quick {
        match install::fetch_manifest(&data, &state.http).await {
            Ok(m) => game::risky_worlds(&data.instance(&instance.id), &instance.version, &m),
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };
    // Общие настройки, серверы, ресурспаки. Ошибка здесь игру не держит —
    // окно покажет её после игры.
    let before = {
        let (data, instance, version) = (data.clone(), instance.clone(), launch.version.clone());
        tauri::async_runtime::spawn_blocking(move || {
            let game_dir = data.instance(&instance.id);
            game::backup_worlds(&game_dir, &risky)
                .map_err(|e| format!("Не удалось сохранить копию миров: {e}"))?;
            game::before_launch(&data, &instance, &version)
                .map_err(|e| format!("Общие настройки игры не применились: {e}"))
        })
        .await
        .map_err(text)?
    };
    // Без копии миров старую версию не запускаем: мир может испортиться.
    if let Err(e) = &before
        && e.starts_with("Не удалось сохранить копию")
    {
        reset(&app);
        return Err(e.clone());
    }
    let note = before.err();

    let game = match game::spawn(&launch.cmd) {
        Ok(game) => game,
        Err(e) => {
            reset(&app);
            return Err(text(e));
        }
    };
    // Сразу отмечаем запуск — сборка поднимается в начало «Недавних».
    let _ = Builds::new(data.clone()).record_play(&id, game.start, 0);
    let on_launch = settings.on_launch;
    let version = launch.version;
    std::thread::Builder::new()
        .name("game-watch".into())
        .spawn(move || watch(app, game, instance, version, on_launch, data, note))
        .map_err(text)?;
    Ok(())
}

/// Полоса прогресса: события ядра → окно, не чаще раза в 100 мс.
fn progress(app: AppHandle) -> Progress {
    let last = Mutex::new(Instant::now() - Duration::from_secs(1));
    Arc::new(move |event| {
        let state = app.state::<AppState>();
        let status = {
            let mut play = state.play.lock().unwrap();
            let s = &mut play.status;
            match event {
                Event::Checking { .. } | Event::Extracting => {}
                Event::Downloading { bytes, .. } => {
                    s.phase = Phase::Downloading;
                    s.total = bytes;
                    s.done = 0;
                }
                Event::FileDone { bytes } => s.done += bytes,
            }
            s.clone()
        };
        let mut last = last.lock().unwrap();
        let finished = status.total > 0 && status.done >= status.total;
        if last.elapsed() >= Duration::from_millis(100) || finished {
            *last = Instant::now();
            let _ = app.emit("play", status);
        }
    })
}

/// Ждать конца игры в отдельном потоке, потом записать время и вернуть окно.
fn watch(
    app: AppHandle,
    game: Game,
    instance: Instance,
    version: GameVersion,
    on_launch: OnLaunch,
    data: DataDir,
    note: Option<String>,
) {
    let outcome = game.wait(|| {
        // Сначала «игра идёт», потом закрываем окно: иначе закрытие
        // последнего окна завершит лаунчер вместе с подсчётом времени.
        update(&app, |s| s.phase = Phase::Running);
        if let Some(window) = app.get_webview_window("main") {
            match on_launch {
                OnLaunch::Close => {
                    let _ = window.destroy();
                }
                OnLaunch::Minimize => {
                    let _ = window.minimize();
                }
                OnLaunch::Keep => {}
            }
        }
    });

    // Изменённые в игре настройки и серверы — в общие.
    let synced = game::after_exit(&data, &instance, &version)
        .err()
        .map(|e| format!("Изменения настроек игры не сохранились в общие: {e}"));
    let finished = match outcome {
        Ok(o) => {
            let stats = Stats::new(&data)
                .append(&o.record(&instance))
                .err()
                .map(|e| format!("Время этой игры не записалось в статистику: {e}"));
            let warning = [note, synced, stats]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            let warning = (!warning.is_empty()).then(|| warning.join(" "));
            let seconds = o.end.saturating_sub(o.start);
            let _ = Builds::new(data).record_play(&instance.id, o.start, seconds);
            let crashed = o.crashed();
            Finished {
                build: instance.id.clone(),
                name: instance.name.clone(),
                seconds,
                code: o.code,
                crashed,
                no_window: !o.had_window,
                // Хвост нужен только при ошибке: последние 40 строк.
                tail: if crashed {
                    o.tail[o.tail.len().saturating_sub(40)..].to_vec()
                } else {
                    Vec::new()
                },
                warning,
            }
        }
        Err(e) => Finished {
            build: instance.id.clone(),
            name: instance.name.clone(),
            seconds: 0,
            code: None,
            crashed: true,
            no_window: false,
            tail: vec![text(e)],
            warning: None,
        },
    };

    reset(&app);
    match app.get_webview_window("main") {
        Some(window) => {
            let _ = window.unminimize();
            let _ = window.set_focus();
            let _ = app.emit("play-finished", finished);
        }
        None => {
            app.state::<AppState>().play.lock().unwrap().finished = Some(finished);
            let _ = open_window(&app);
        }
    }
}
