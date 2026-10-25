//! Запуск игры из окна. Ядро готовит и запускает игру ([`game`]), здесь —
//! только связь с окном: полоса прогресса, окно прячется на время игры и
//! открывается снова после неё (принцип 4 ТЗ).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use majorlauncher_core::builds::QUICK_ID;
use majorlauncher_core::cancel::Cancel;
use majorlauncher_core::game::{self, Game};
use majorlauncher_core::mods::{self, Issue};
use majorlauncher_core::options::GameVersion;
use majorlauncher_core::playtime::LastPlace;
use majorlauncher_core::playtime::Stats;
use majorlauncher_core::snapshots::{self, Hint};
use majorlauncher_core::system;
use majorlauncher_core::{Builds, DataDir, Event, Instance, OnLaunch, Progress, install};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::tasks::LazyTask;
use crate::{AppState, CmdResult, open_window, text};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Idle,
    /// Проверяем файлы и аккаунт.
    Preparing,
    /// «Проверить файлы»: пересчитываем хеши всех файлов сборки.
    Verifying,
    Downloading,
    /// Ставим загрузчик модов: `done` из `total` шагов.
    Installing,
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
    /// Что происходит сейчас, словами: «Проверка модов», «Загрузка модов: 162».
    pub stage: Option<String>,
    /// Когда запущена Java, мс с 1970 года: окно само считает, сколько идёт запуск.
    pub since: Option<u64>,
    /// Сколько сборка грузилась до окна игры в прошлые разы, секунд.
    pub expected: Option<u64>,
    /// «Перезапустить и вернуться»: после выхода игра запустится снова.
    pub restart: bool,
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
    /// Запуск отменили кнопкой «Отменить» — это не краш.
    cancelled: bool,
    /// Сколько раз подряд сборка падает (с этим разом).
    streak: u32,
    /// Падает раз за разом после изменения сборки — снапшот, к которому откатиться.
    rollback: Option<Hint>,
}

#[derive(Default)]
pub struct PlayState {
    pub status: PlayStatus,
    /// Итог игры, пока окно было закрыто: заберёт открывшееся окно.
    pub finished: Option<Finished>,
    /// «Отменить» для текущего запуска.
    pub cancel: Cancel,
    /// Java запущенной игры — её завершает «Отменить», пока нет окна игры.
    pub pid: Option<u32>,
    /// Перезапуск после выхода не начался: моды не дадут игре запуститься.
    pub blocked: Option<Blocked>,
}

/// Проблемы сборки, из-за которых перезапуск не начался.
#[derive(Debug, Clone, Serialize)]
pub struct Blocked {
    id: String,
    issues: Vec<Issue>,
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
    blocked: Option<Blocked>,
}

/// Состояние для только что открытого окна. Итог прошлой игры отдаётся один раз.
#[tauri::command]
pub fn play_status(state: State<AppState>) -> StatusWithResult {
    let mut play = state.play.lock().unwrap();
    StatusWithResult {
        status: play.status.clone(),
        finished: play.finished.take(),
        blocked: play.blocked.take(),
    }
}

/// «Перезапустить и вернуться»: после выхода из игры лаунчер сразу запустит
/// её снова — в тот же мир или на тот же сервер. `close_now` — закрыть игру
/// сейчас, как крестиком окна (она сохранит мир и выйдет).
#[tauri::command]
pub fn play_restart(app: AppHandle, state: State<AppState>, close_now: bool) -> CmdResult<()> {
    {
        let mut play = state.play.lock().unwrap();
        if play.status.phase != Phase::Running {
            return Err("игра не запущена".into());
        }
        if close_now && !play.pid.is_some_and(system::close_window) {
            return Err(
                "окно игры не нашлось — закройте игру сами, лаунчер запустит её снова".into(),
            );
        }
        play.status.restart = true;
    }
    update(&app, |s| s.restart = true);
    Ok(())
}

#[tauri::command]
pub fn play_restart_cancel(app: AppHandle, state: State<AppState>) {
    state.play.lock().unwrap().status.restart = false;
    update(&app, |s| s.restart = false);
}

/// «Отменить» на кнопке «Играть»: подготовка и загрузка обрываются (скачанное
/// остаётся до следующего раза), запущенная Java без окна завершается.
#[tauri::command]
pub fn play_cancel(state: State<AppState>) -> CmdResult<()> {
    let play = state.play.lock().unwrap();
    match play.status.phase {
        Phase::Preparing | Phase::Downloading | Phase::Installing | Phase::Starting => {
            play.cancel.cancel();
            if let Some(pid) = play.pid {
                system::kill(pid);
            }
            Ok(())
        }
        Phase::Running => Err("игра уже открылась — закройте её как обычно".into()),
        Phase::Idle | Phase::Verifying => Err("сейчас нечего отменять".into()),
    }
}

#[tauri::command]
pub async fn build_play(
    app: AppHandle,
    id: String,
    version: Option<String>,
    force: Option<bool>,
) -> CmdResult<Option<Vec<Issue>>> {
    launch(app, id, version, force, None).await
}

/// Запуск игры: кнопка «Играть» и перезапуск после выхода. `quick_play` —
/// сразу войти в мир или на сервер.
pub async fn launch(
    app: AppHandle,
    id: String,
    version: Option<String>,
    force: Option<bool>,
    quick_play: Option<LastPlace>,
) -> CmdResult<Option<Vec<Issue>>> {
    let handle = app.clone();
    let state = handle.state::<AppState>();
    let data = state.data()?;
    let builds = Builds::new(data.clone());
    let check_mods = !force.unwrap_or(false) && id != QUICK_ID;
    let cancel = Cancel::default();
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
            stage: Some(
                if check_mods {
                    "Проверка модов"
                } else {
                    "Подготовка"
                }
                .into(),
            ),
            ..Default::default()
        };
        play.cancel = cancel.clone();
        play.pid = None;
    }
    let _ = app.emit("play", state.play.lock().unwrap().status.clone());
    // Дальше кнопка «Играть» занята: при любом отказе её надо освободить.
    // Отмену окно показывает само — это не ошибка.
    let fail = |e: String| {
        reset(&app);
        if cancel.is_cancelled() {
            Ok(None)
        } else {
            Err(e)
        }
    };

    // Проблемы, которые не дадут игре запуститься (раздел 5.6 ТЗ): окно
    // покажет их и спросит «Всё равно играть?». Только известное, без сети.
    if check_mods {
        let report = match cancel
            .guard(mods::report(&state.http, &data, &id, false))
            .await
        {
            Ok(r) => r,
            Err(e) => return fail(text(e)),
        };
        let mut blocking: Vec<Issue> = report.blocking().into_iter().cloned().collect();
        // Памяти больше, чем в ПК, или нет места на диске — тоже не запустится.
        let enabled = report.mods.iter().filter(|m| m.enabled).count();
        let game = state.config.lock().unwrap().get().game.clone();
        blocking.extend(
            crate::optimize::system_issues(&data, &id, &game, enabled)
                .into_iter()
                .filter(|i| i.level == majorlauncher_core::mods::Level::Block),
        );
        if !blocking.is_empty() {
            reset(&app);
            return Ok(Some(blocking));
        }
    }
    // «Быстрая игра» запускается с версией, выбранной у кнопки.
    let instance = match match &version {
        Some(v) if id == QUICK_ID => builds.quick(v),
        _ => builds.get(&id),
    } {
        Ok(i) => i,
        Err(e) => return fail(text(e)),
    };
    let settings = state.config.lock().unwrap().get().game.clone();
    update(&app, |s| s.stage = Some("Вход и проверка файлов".into()));

    let task = LazyTask::new(
        &app,
        format!("Файлы игры {}", version_title(&instance)),
        format!("для сборки «{}»", instance.name),
    );
    // Сборка уже стартовала с этими версиями: если теперь что-то качается,
    // значит, файлы пропали — скажем, почему (чаще всего антивирус).
    let again = game::was_ready(&data, &instance);
    let prepared = game::prepare(
        &data,
        &state.http,
        &instance,
        &settings,
        progress(app.clone(), Some(task.clone()), again),
        &cancel,
        quick_play,
    )
    .await;
    task.finish(&prepared);
    let launch = match prepared {
        Ok(launch) => launch,
        Err(e) => return fail(text(e)),
    };
    update(&app, |s| {
        s.phase = Phase::Starting;
        s.stage = Some("Применяем настройки игры".into());
    });

    // Миры из более новой версии — в копию перед запуском (окно уже
    // предупредило игрока и получило согласие).
    let risky = match install::fetch_manifest(&data, &state.http).await {
        Ok(m) => game::risky_worlds(&data.instance(&instance.id), &instance.version, &m),
        Err(_) => Vec::new(),
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
        .unwrap_or_else(|e| Err(text(e)))
    };
    // Без копии миров старую версию не запускаем: мир может испортиться.
    if let Err(e) = &before
        && e.starts_with("Не удалось сохранить копию")
    {
        return fail(e.clone());
    }
    let note = before.err();
    if cancel.is_cancelled() {
        return fail(String::new());
    }

    let game = match game::spawn(&launch.cmd) {
        Ok(game) => game,
        Err(e) => return fail(text(e)),
    };
    {
        let mut play = state.play.lock().unwrap();
        play.pid = Some(game.pid());
        // «Отменить» нажали, пока Java запускалась, — завершаем её сразу.
        if cancel.is_cancelled() {
            system::kill(game.pid());
        }
    }
    // Сколько грузилась в прошлый раз — окно покажет проценты запуска.
    let expected = game::load_time(&data, &load_key(&instance));
    update(&app, |s| {
        s.stage = Some("Запуск Java".into());
        s.since = Some(unix_ms());
        s.expected = expected;
    });
    game::mark_ready(&data, &instance);
    // Сразу отмечаем запуск — сборка поднимается в начало «Недавних».
    let _ = Builds::new(data.clone()).record_play(&id, game.start, 0);
    let on_launch = settings.on_launch;
    let version = launch.version;
    std::thread::Builder::new()
        .name("game-watch".into())
        .spawn(move || watch(app, game, instance, version, on_launch, data, note))
        .map_err(text)?;
    Ok(None)
}

/// Под каким именем запоминать время загрузки: у «Быстрой игры» оно своё для
/// каждой версии.
fn load_key(instance: &Instance) -> String {
    if instance.quick {
        format!("{QUICK_ID}/{}", instance.version)
    } else {
        instance.id.clone()
    }
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// «Проверить файлы» (страница сборки → «Настройки сборки»). Занимает кнопку
/// «Играть» этой сборки, как подготовка к запуску.
#[tauri::command]
pub async fn build_verify(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<game::Checked> {
    let data = state.data()?;
    let instance = Builds::new(data.clone()).get(&id).map_err(text)?;
    {
        let mut play = state.play.lock().unwrap();
        if play.status.phase != Phase::Idle {
            return Err("игра запущена или готовится к запуску — проверьте файлы после неё".into());
        }
        if state.relocating() {
            return Err("идёт перенос папки данных — подождите, пока он закончится".into());
        }
        play.status = PlayStatus {
            build: Some(id.clone()),
            phase: Phase::Verifying,
            ..Default::default()
        };
    }
    let _ = app.emit("play", state.play.lock().unwrap().status.clone());
    let task = crate::tasks::start(
        &app,
        format!("Проверка файлов {}", version_title(&instance)),
        format!("сборка «{}»", instance.name),
    );
    task.stage("Пересчитываем хеши файлов");
    let forward = {
        let task = task.clone();
        move |e: Event| task.event(&e)
    };
    let result = game::verify(
        &data,
        &state.http,
        &instance,
        progress_with(app.clone(), Arc::new(forward), false),
    )
    .await;
    task.finish(&result);
    reset(&app);
    result.map_err(text)
}

/// «1.21.1 · NeoForge 21.1.257» — для панели «Загрузки».
fn version_title(instance: &Instance) -> String {
    match &instance.loader_version {
        Some(v) if instance.loader != majorlauncher_core::loaders::Loader::Vanilla => {
            format!("{} · {} {v}", instance.version, instance.loader.title())
        }
        _ => instance.version.clone(),
    }
}

/// Полоса прогресса и панель «Загрузки» (если задача есть).
fn progress(app: AppHandle, task: Option<Arc<LazyTask>>, again: bool) -> Progress {
    progress_with(
        app,
        Arc::new(move |e: Event| {
            if let Some(t) = &task {
                t.event(&e);
            }
        }),
        again,
    )
}

/// Полоса прогресса: события ядра → окно, не чаще раза в 100 мс; `also` —
/// кому ещё передать событие.
fn progress_with(app: AppHandle, also: Arc<dyn Fn(Event) + Send + Sync>, again: bool) -> Progress {
    let last = Mutex::new(Instant::now() - Duration::from_secs(1));
    Arc::new(move |event| {
        also(event.clone());
        let state = app.state::<AppState>();
        let status = {
            let mut play = state.play.lock().unwrap();
            let s = &mut play.status;
            match event {
                // Во время «Проверить файлы» подпись своя.
                Event::Checking { files } if s.phase != Phase::Verifying => {
                    s.stage = Some(format!("Проверка файлов игры: {files}"));
                }
                Event::Checking { .. } => {}
                Event::Extracting => s.stage = Some("Распаковка библиотек".into()),
                Event::Downloading { bytes, files } => {
                    s.phase = Phase::Downloading;
                    s.total = bytes;
                    s.done = 0;
                    s.stage = Some(if again {
                        format!(
                            "Пропали файлы игры ({files}) — возможно, их удалил антивирус. Качаю заново"
                        )
                    } else {
                        format!("Загрузка файлов игры: {files}")
                    });
                }
                Event::FileDone { bytes } => s.done += bytes,
                Event::Loader { step, total } => {
                    s.phase = Phase::Installing;
                    s.done = step as u64;
                    s.total = total as u64;
                    s.stage = Some("Установка загрузчика модов".into());
                }
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
    let started = Instant::now();
    let key = load_key(&instance);
    let cancel = app.state::<AppState>().play.lock().unwrap().cancel.clone();
    let on_window = || {
        let _ = game::record_load_time(&data, &key, started.elapsed().as_secs());
        // Сначала «игра идёт», потом закрываем окно: иначе закрытие
        // последнего окна завершит лаунчер вместе с подсчётом времени.
        update(&app, |s| {
            s.phase = Phase::Running;
            s.stage = None;
        });
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
    };
    let outcome = game.wait(on_window, |stage| update(&app, |s| s.stage = Some(stage)));

    // Изменённые в игре настройки и серверы — в общие.
    let synced = game::after_exit(&data, &instance, &version)
        .err()
        .map(|e| format!("Изменения настроек игры не сохранились в общие: {e}"));
    let cancelled = cancel.is_cancelled();
    let last_place = outcome.as_ref().ok().and_then(|o| o.last_place.clone());
    let finished = match outcome {
        // Отменили до окна игры: ни статистики, ни краша.
        _ if cancelled => Finished {
            build: instance.id.clone(),
            name: instance.name.clone(),
            seconds: 0,
            code: None,
            crashed: false,
            no_window: true,
            tail: Vec::new(),
            warning: None,
            cancelled: true,
            streak: 0,
            rollback: None,
        },
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
            let _ = Builds::new(data.clone()).record_play(&instance.id, o.start, seconds);
            let crashed = o.crashed();
            let streak = game::record_result(&data, &instance.id, crashed, o.end);
            // Падает второй раз подряд — предложить откатить последнее изменение.
            let rollback = (crashed && streak.crashes >= 2)
                .then(|| snapshots::rollback_hint(&data, &instance.id, streak.last_ok))
                .flatten();
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
                cancelled: false,
                streak: streak.crashes,
                rollback,
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
            cancelled: false,
            streak: 0,
            rollback: None,
        },
    };

    // «Перезапустить и вернуться»: игра вышла сама (не упала) — сразу снова,
    // в тот же мир или на сервер. Окно лаунчера не показываем: оно закроется
    // при старте игры, как обычно.
    let restart = std::mem::take(&mut app.state::<AppState>().play.lock().unwrap().status.restart);
    reset(&app);
    if restart && !finished.crashed && !finished.cancelled {
        let version = instance.quick.then(|| instance.version.clone());
        let id = instance.id.clone();
        tauri::async_runtime::spawn(async move {
            let result = launch(app.clone(), id.clone(), version, None, last_place).await;
            let state = app.state::<AppState>();
            match result {
                Ok(None) => {}
                Ok(Some(issues)) => {
                    state.play.lock().unwrap().blocked = Some(Blocked { id, issues });
                    show(&app);
                }
                Err(e) => {
                    state.play.lock().unwrap().finished = Some(Finished {
                        build: id,
                        name: instance.name.clone(),
                        seconds: 0,
                        code: None,
                        crashed: false,
                        no_window: true,
                        tail: Vec::new(),
                        warning: Some(format!("Перезапустить игру не удалось: {e}")),
                        cancelled: true,
                        streak: 0,
                        rollback: None,
                    });
                    show(&app);
                }
            }
        });
        return;
    }
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

/// Показать окно лаунчера с тем, что накопилось для него (итог игры, проблемы
/// сборки): уже открытое получает события, новое заберёт их в `play_status`.
fn show(app: &AppHandle) {
    match app.get_webview_window("main") {
        Some(window) => {
            let _ = window.unminimize();
            let _ = window.set_focus();
            let state = app.state::<AppState>();
            let mut play = state.play.lock().unwrap();
            if let Some(f) = play.finished.take() {
                let _ = app.emit("play-finished", f);
            }
            if let Some(b) = play.blocked.take() {
                let _ = app.emit("play-blocked", b);
            }
        }
        None => {
            let _ = open_window(app);
        }
    }
}
