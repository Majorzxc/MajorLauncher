// В релизе не открываем консольное окно рядом с лаунчером.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Окно лаунчера. Здесь только мост между интерфейсом и ядром: команды
//! принимают запрос окна, вызывают ядро и возвращают результат. Решения
//! (что скачать, что запустить, какой цвет допустим) принимает ядро.

mod builds;
mod settings;

use std::sync::Mutex;

use majorlauncher_core::{Config, ConfigStore, DataDir, DownloadOptions, Downloader};
use tauri::window::Color;
use tauri::{Manager, State, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Флаги WebView2: стандартные флаги Tauri плюс отключение GPU.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu --disable-software-rasterizer";

/// Ошибки уходят в окно текстом: ядро уже пишет их понятным языком.
pub type CmdResult<T> = Result<T, String>;

pub struct AppState {
    pub config: Mutex<ConfigStore>,
    pub http: Downloader,
}

impl AppState {
    /// Папка данных или понятная ошибка, если её ещё не выбрали.
    pub fn data(&self) -> CmdResult<DataDir> {
        self.config
            .lock()
            .unwrap()
            .get()
            .data_dir
            .clone()
            .map(DataDir::new)
            .ok_or_else(|| "папка данных ещё не выбрана".to_string())
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("config.json");
            let config = ConfigStore::load(path);

            // Окно создаётся здесь, а не в tauri.conf.json: так оно видимо и
            // покрашено в фон темы сразу, а WebView2 (около секунды на запуск)
            // подключается к нему следом — без ожидания и без белой вспышки.
            let [r, g, b] = config.get().appearance.background_rgb(system_dark());
            WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("MajorLauncher")
                .inner_size(1280.0, 720.0)
                .min_inner_size(1024.0, 640.0)
                .center()
                .background_color(Color(r, g, b, 255))
                // Без GPU: 154 → 97 МБ (замер 03.10.2026). Интерфейсу из плиток
                // и кнопок видеокарта не нужна; WebGL для 3D-скинов — этап 8.
                .additional_browser_args(BROWSER_ARGS)
                .visible(true)
                .build()?;

            app.manage(AppState {
                config: Mutex::new(config),
                http: Downloader::new(DownloadOptions::default())?,
            });

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
            settings::current_account,
            settings::data_dir_suggestion,
            settings::inspect_dir,
            settings::set_data_dir,
            settings::open_data_dir,
            builds::versions_list,
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
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить окно MajorLauncher");
}
