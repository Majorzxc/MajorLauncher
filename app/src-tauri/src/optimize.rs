//! «Оптимизировать» и система: железо ПК, системные предупреждения у сборки,
//! сама оптимизация по настройкам игрока (раздел 5.7 ТЗ).

use majorlauncher_core::health::{self, Hardware};
use majorlauncher_core::mods::Issue;
use majorlauncher_core::optimize::{self, OptimizeSettings, Report};
use majorlauncher_core::{Builds, Config, DataDir, GameSettings};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::builds::blocking;
use crate::play::Phase;
use crate::{AppState, CmdResult, text};

/// Железо ПК — для раздела «Оптимизация» в настройках.
#[tauri::command]
pub async fn hardware_info() -> CmdResult<Hardware> {
    blocking(|| Ok(health::hardware())).await
}

#[tauri::command]
pub fn set_optimize(state: State<AppState>, optimize: OptimizeSettings) -> CmdResult<Config> {
    let mut config = state.config.lock().unwrap();
    config.set_optimize(optimize).map_err(text)?;
    Ok(config.get().clone())
}

/// Есть ли интернет: соединение с Modrinth за 3 секунды. Без него можно
/// играть в скачанные сборки, а каталог, обновления и вход Ely.by недоступны.
#[tauri::command]
pub async fn net_online() -> bool {
    blocking(|| {
        use std::net::{TcpStream, ToSocketAddrs};
        let ok = ("api.modrinth.com", 443)
            .to_socket_addrs()
            .is_ok_and(|mut addrs| {
                addrs.any(|a| {
                    TcpStream::connect_timeout(&a, std::time::Duration::from_secs(3)).is_ok()
                })
            });
        Ok(ok)
    })
    .await
    .unwrap_or(true)
}

/// Настройки питания Windows (из предупреждения про «Экономию энергии»).
#[tauri::command]
pub fn open_power_settings(app: AppHandle) -> CmdResult<()> {
    app.opener()
        .open_url("ms-settings:powersleep", None::<&str>)
        .map_err(text)
}

/// Системные предупреждения сборки: место, память, видеокарта, питание.
pub fn system_issues(data: &DataDir, id: &str, game: &GameSettings, mods: usize) -> Vec<Issue> {
    let Ok(instance) = Builds::new(data.clone()).get(id) else {
        return Vec::new();
    };
    health::issues(data, &instance, game, mods, &health::hardware())
}

/// Оптимизировать сборку. Прогресс — в «Загрузках», итог — отчёт по шагам.
#[tauri::command]
pub async fn optimize_build(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<Report> {
    let data = state.data()?;
    {
        let play = state.play.lock().unwrap();
        if play.status.phase != Phase::Idle && play.status.build.as_deref() == Some(id.as_str()) {
            return Err("сборка запущена — оптимизируйте её после игры".into());
        }
    }
    let (opts, game) = {
        let config = state.config.lock().unwrap();
        (config.get().optimize.clone(), config.get().game.clone())
    };
    let name = Builds::new(data.clone()).get(&id).map_err(text)?.name;
    let task = crate::tasks::start(&app, "Оптимизация", format!("сборка «{name}»"));
    let progress = task.progress();
    let stage = {
        let task = task.clone();
        move |s: &str| task.stage(s)
    };
    let result = optimize::run(&state.http, &data, &id, &opts, &game, &progress, &stage).await;
    task.finish(&result);
    result.map_err(text)
}
