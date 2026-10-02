// В релизе не открываем консольное окно рядом с лаунчером.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Версия ядра — пока единственное, что окно спрашивает у ядра.
#[tauri::command]
fn core_version() -> &'static str {
    majorlauncher_core::VERSION
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![core_version])
        .run(tauri::generate_context!())
        .expect("не удалось запустить окно MajorLauncher");
}
