// В релизе не открываем консольное окно рядом с лаунчером.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Окно лаунчера. Здесь только мост между интерфейсом и ядром: команды
//! принимают запрос окна, вызывают ядро и возвращают результат. Решения
//! (что скачать, что запустить, какой цвет допустим) принимает ядро.

use std::sync::Mutex;

use majorlauncher_core::{Accounts, Appearance, Config, ConfigStore, DataDir};
use serde::Serialize;
use tauri::window::Color;
use tauri::{Manager, State, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Флаги WebView2: стандартные флаги Tauri плюс отключение GPU.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu --disable-software-rasterizer";

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

/// Ошибки уходят в окно текстом: ядро уже пишет их понятным языком.
type CmdResult<T> = Result<T, String>;

struct AppState {
    config: Mutex<ConfigStore>,
}

#[tauri::command]
fn core_version() -> &'static str {
    majorlauncher_core::VERSION
}

#[tauri::command]
fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().get().clone()
}

#[tauri::command]
fn set_appearance(
    window: WebviewWindow,
    state: State<AppState>,
    appearance: Appearance,
) -> CmdResult<Config> {
    let mut config = state.config.lock().unwrap();
    config
        .set_appearance(appearance)
        .map_err(|e| e.to_string())?;
    paint(&window, config.get());
    Ok(config.get().clone())
}

/// Покрасить окно в фон темы — он виден, пока интерфейс не нарисован
/// (при старте и на краях при изменении размера окна).
fn paint(window: &WebviewWindow, config: &Config) {
    let system_dark = window.theme().is_ok_and(|t| t == Theme::Dark);
    let [r, g, b] = config.appearance.background_rgb(system_dark);
    let _ = window.set_background_color(Some(Color(r, g, b, 255)));
}

#[derive(Serialize)]
struct AccountInfo {
    name: String,
    kind: &'static str,
}

/// Аккаунт по умолчанию для плашки в шапке. `None` — папка данных ещё
/// не выбрана или аккаунтов нет.
#[tauri::command]
fn current_account(state: State<AppState>) -> CmdResult<Option<AccountInfo>> {
    let Some(dir) = state.config.lock().unwrap().get().data_dir.clone() else {
        return Ok(None);
    };
    let accounts = Accounts::load(&DataDir::new(dir)).map_err(|e| e.to_string())?;
    Ok(accounts.default_account().map(|a| AccountInfo {
        name: a.name.clone(),
        kind: a.kind.label(),
    }))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_version,
            get_config,
            set_appearance,
            current_account
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить окно MajorLauncher");
}
