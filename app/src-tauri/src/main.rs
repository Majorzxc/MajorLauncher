// В релизе не открываем консольное окно рядом с лаунчером.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Окно лаунчера. Здесь только мост между интерфейсом и ядром: команды
//! принимают запрос окна, вызывают ядро и возвращают результат. Решения
//! (что скачать, что запустить, какой цвет допустим) принимает ядро.

mod accounts;
mod builds;
mod catalog;
mod content;
mod game_options;
mod mods;
mod play;
mod settings;
mod snapshots;
mod tasks;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use majorlauncher_core::{Config, ConfigStore, DataDir, DownloadOptions, Downloader};
use tauri::window::Color;
use tauri::{
    AppHandle, Manager, RunEvent, State, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

/// Флаги WebView2: стандартные флаги Tauri плюс отключение GPU.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu --disable-software-rasterizer";

/// Ошибки уходят в окно текстом: ядро уже пишет их понятным языком.
pub type CmdResult<T> = Result<T, String>;

pub struct AppState {
    pub config: Mutex<ConfigStore>,
    pub http: Downloader,
    pub play: Mutex<play::PlayState>,
    pub relocation: Mutex<Relocation>,
    pub tasks: Mutex<tasks::Tasks>,
    /// Диск с данными сменил букву при этом запуске — окно скажет об этом один раз.
    pub data_moved: Mutex<Option<(String, String)>>,
}

/// Перенос папки данных: пока он идёт, остальные команды её не трогают.
#[derive(Default)]
pub struct Relocation {
    pub active: bool,
    pub cancel: Arc<AtomicBool>,
    /// Старая папка после копирования на другой диск — её удаляет только игрок.
    pub old: Option<PathBuf>,
}

impl AppState {
    /// Папка данных или понятная ошибка, если её ещё не выбрали.
    pub fn data(&self) -> CmdResult<DataDir> {
        if self.relocating() {
            return Err("идёт перенос папки данных — подождите, пока он закончится".into());
        }
        let data = self
            .config
            .lock()
            .unwrap()
            .get()
            .data_dir
            .clone()
            .map(DataDir::new)
            .ok_or_else(|| "папка данных ещё не выбрана".to_string())?;
        // Диск отключён или сменил букву: не создавать на его месте новых
        // папок (на этой букве может оказаться другой диск).
        if !data.root().is_dir() {
            return Err(format!(
                "папка данных {} не найдена — диск отключён или сменил букву",
                data.root().display()
            ));
        }
        Ok(data)
    }
}

impl AppState {
    pub fn relocating(&self) -> bool {
        self.relocation.lock().unwrap().active
    }
}

/// Перевести ошибку ядра в текст для окна.
pub fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
fn core_version() -> &'static str {
    majorlauncher_core::VERSION
}

/// Тёмная ли тема приложений в Windows — до создания окна (у окна её ещё не
/// спросить). Нет значения в реестре — считаем тёмной.
fn system_dark() -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|key| key.get_value::<u32, _>("AppsUseLightTheme"))
        .map_or(true, |light| light == 0)
}

/// Покрасить окно в фон темы — он виден, пока интерфейс не нарисован
/// (при старте и на краях при изменении размера окна).
pub fn paint(window: &WebviewWindow, config: &Config) {
    let system_dark = window.theme().is_ok_and(|t| t == Theme::Dark);
    let [r, g, b] = config.appearance.background_rgb(system_dark);
    let _ = window.set_background_color(Some(Color(r, g, b, 255)));
}

/// Открыть окно лаунчера или показать уже открытое. Окно закрывается на
/// время игры и создаётся заново после неё.
pub fn open_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(window);
    }
    let state: State<AppState> = app.state();
    let [r, g, b] = state
        .config
        .lock()
        .unwrap()
        .get()
        .appearance
        .background_rgb(system_dark());
    // Окно создаётся здесь, а не в tauri.conf.json: так оно видимо и
    // покрашено в фон темы сразу, а WebView2 (около секунды на запуск)
    // подключается к нему следом — без ожидания и без белой вспышки.
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("MajorLauncher")
        .inner_size(1280.0, 720.0)
        .min_inner_size(1024.0, 640.0)
        .center()
        .background_color(Color(r, g, b, 255))
        // Без GPU: 154 → 97 МБ (замер 03.10.2026). Интерфейсу из плиток
        // и кнопок видеокарта не нужна; WebGL для 3D-скинов — этап 8.
        .additional_browser_args(BROWSER_ARGS)
        .visible(true);
    // Проверочный экземпляр — со своей папкой WebView2: если рядом открыт
    // лаунчер игрока с другими флагами браузера, общая папка не даёт окну
    // подняться.
    let builder = match std::env::var_os("MAJORLAUNCHER_CONFIG_DIR") {
        Some(dir) => builder.data_directory(std::path::PathBuf::from(dir).join("webview")),
        None => builder,
    };
    builder.build()
}

fn main() {
    let mut builder = tauri::Builder::default();
    // Второй запуск лаунчера не открывает вторую копию, а показывает эту —
    // в том числе во время игры, когда окна нет. Проверочный экземпляр со
    // своей папкой настроек (MAJORLAUNCHER_CONFIG_DIR) живёт отдельно.
    if std::env::var_os("MAJORLAUNCHER_CONFIG_DIR").is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Окно создаём из отдельного потока: в обработчике событий
            // WebView2 на Windows может зависнуть.
            let app = app.clone();
            std::thread::spawn(move || {
                let _ = open_window(&app);
            });
        }));
    }
    let app = builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // MAJORLAUNCHER_CONFIG_DIR — другая папка настроек (а с ней и папка
            // данных) для автоматических проверок: они не трогают данные игрока.
            let config_dir = match std::env::var_os("MAJORLAUNCHER_CONFIG_DIR") {
                Some(dir) => std::path::PathBuf::from(dir),
                None => app.path().app_config_dir()?,
            };
            let config = ConfigStore::load(config_dir.join("config.json"));
            app.manage(AppState {
                config: Mutex::new(config),
                http: Downloader::new(DownloadOptions::default())?,
                play: Mutex::new(play::PlayState::default()),
                relocation: Mutex::new(Relocation::default()),
                tasks: Mutex::new(tasks::Tasks::default()),
                data_moved: Mutex::new(None),
            });
            // Диск с данными сменил букву — найти папку до первого окна.
            let state: State<AppState> = app.state();
            let found = settings::locate_data(&state);
            *state.data_moved.lock().unwrap() = found.moved;
            open_window(app.handle())?;

            // Корзина чистится от сборок старше 30 дней при каждом запуске.
            let state: State<AppState> = app.state();
            if let Ok(data) = state.data() {
                let _ = majorlauncher_core::Builds::new(data)
                    .purge_expired(majorlauncher_core::builds::TRASH_DAYS);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_version,
            settings::get_config,
            settings::set_appearance,
            settings::set_game,
            settings::set_catalog,
            catalog::catalog_search,
            catalog::catalog_categories,
            catalog::catalog_translate,
            catalog::modpack_install,
            catalog::mrpack_inspect,
            catalog::mrpack_import,
            catalog::catalog_project,
            catalog::catalog_versions,
            catalog::catalog_installed,
            catalog::catalog_plan,
            catalog::catalog_install,
            settings::system_memory,
            settings::data_dir_suggestion,
            settings::inspect_dir,
            settings::set_data_dir,
            settings::data_dir_check,
            settings::open_data_dir,
            settings::data_size,
            settings::data_move_plan,
            settings::data_move,
            settings::data_move_cancel,
            settings::data_remove_old,
            settings::background_image,
            settings::set_background_image,
            settings::remove_background_image,
            accounts::accounts_list,
            accounts::account_set_default,
            accounts::account_add_offline,
            accounts::account_add_ely,
            accounts::account_remove,
            builds::versions_list,
            builds::loader_games,
            builds::loader_versions,
            builds::builds_list,
            builds::builds_groups,
            builds::build_create,
            builds::build_update,
            builds::build_duplicate,
            builds::build_trash,
            builds::build_archive,
            builds::build_unarchive,
            builds::build_size,
            builds::build_open_folder,
            builds::build_cover,
            builds::build_set_cover,
            builds::build_remove_cover,
            builds::read_image,
            builds::group_add,
            builds::group_delete,
            builds::trash_list,
            builds::trash_restore,
            builds::trash_purge,
            content::build_files,
            content::build_set_enabled,
            content::build_add_files,
            content::build_worlds,
            content::build_logs,
            content::build_thumbnail,
            content::build_screenshot,
            content::build_open,
            content::stats_summary,
            game_options::options_get,
            game_options::options_set,
            game_options::options_set_shared,
            game_options::options_seed,
            game_options::worlds_check,
            game_options::build_copy_world,
            game_options::library_list,
            game_options::library_add,
            game_options::library_set,
            game_options::library_mods,
            game_options::library_mods_add,
            game_options::library_mods_remove_unused,
            game_options::library_remove,
            play::build_play,
            play::build_verify,
            mods::mods_report,
            mods::mods_apply,
            mods::mods_remove,
            mods::mods_update,
            mods::mods_pin,
            play::play_status,
            play::play_cancel,
            snapshots::snapshots_list,
            snapshots::snapshot_take,
            snapshots::snapshot_restore,
            snapshots::snapshot_delete,
            tasks::tasks_list,
            content::build_screenshot_full,
            content::build_trash_screenshot,
            content::build_log_text,
            content::build_reveal,
            tasks::tasks_clear,
        ])
        .build(tauri::generate_context!())
        .expect("не удалось запустить окно MajorLauncher");

    app.run(|app, event| {
        let game_alive = || app.state::<AppState>().play.lock().unwrap().game_alive();
        match event {
            // Окно закрылось, пока идёт игра (его закрыли мы или игрок): лаунчер
            // остаётся маленьким фоновым процессом — досчитать время и открыть
            // окно, когда игра закроется.
            RunEvent::ExitRequested {
                api, code: None, ..
            } if game_alive() => api.prevent_exit(),
            // Подстраховка: окна нет и игры нет — выходим сами. Иначе невидимый
            // процесс перехватывал бы следующие запуски лаунчера.
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::Destroyed,
                ..
            } if label == "main" && !game_alive() => app.exit(0),
            _ => {}
        }
    });
}
