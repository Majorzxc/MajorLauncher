//! Раздел «Серверы» (раздел 5.10 ТЗ): общий список или свой список сборки,
//! опрос серверов, сборки для «Играть на сервере».

use std::path::PathBuf;

use majorlauncher_core::Builds;
use majorlauncher_core::builds::State as BuildState;
use majorlauncher_core::playtime::Stats;
use majorlauncher_core::servers::fit::{self, Choice, Played};
use majorlauncher_core::servers::ping::{self, Status};
use majorlauncher_core::servers::{self, Server, ServerEdit};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::builds::blocking;
use crate::{AppState, CmdResult, text};

/// Сервер в списке окна: из файла и из статистики.
#[derive(Serialize)]
pub struct Row {
    #[serde(flatten)]
    server: Server,
    played: Option<Played>,
}

/// Список, который можно открыть: общий или свой у сборки.
#[derive(Serialize)]
pub struct ListInfo {
    /// Сборка; `None` — общий список.
    build: Option<String>,
    name: String,
}

#[derive(Serialize)]
pub struct ServersView {
    servers: Vec<Row>,
    /// Сборки со своим списком (у них выключен общий).
    lists: Vec<ListInfo>,
}

fn path(state: &State<AppState>, build: &Option<String>) -> CmdResult<PathBuf> {
    let data = state.data()?;
    Ok(match build {
        Some(id) => servers::build_path(&data, id),
        None => servers::shared_path(&data),
    })
}

fn view(state: &State<AppState>, build: &Option<String>) -> CmdResult<ServersView> {
    let data = state.data()?;
    let list = servers::list(&path(state, build)?).map_err(text)?;
    // Статистика — только время и последняя сборка; без неё список всё равно нужен.
    let played = Stats::new(&data)
        .load()
        .map(|r| fit::played(&r))
        .unwrap_or_default();
    let lists = Builds::new(data)
        .list()
        .map_err(text)?
        .into_iter()
        .filter(|e| e.state == BuildState::Active && e.instance.settings.own_servers)
        .map(|e| ListInfo {
            build: Some(e.instance.id),
            name: e.instance.name,
        })
        .collect();
    Ok(ServersView {
        servers: list
            .into_iter()
            // Скрытые игра запомнила при прямом подключении — в её списке их нет.
            .filter(|s| !s.hidden)
            .map(|server| Row {
                played: played.get(&fit::key(&server.address)).cloned(),
                server,
            })
            .collect(),
        lists,
    })
}

#[tauri::command]
pub async fn servers_list(
    state: State<'_, AppState>,
    build: Option<String>,
) -> CmdResult<ServersView> {
    view(&state, &build)
}

#[tauri::command]
pub fn server_add(
    state: State<AppState>,
    build: Option<String>,
    name: String,
    address: String,
) -> CmdResult<ServersView> {
    servers::add(&path(&state, &build)?, &name, &address).map_err(text)?;
    view(&state, &build)
}

#[tauri::command]
pub fn server_edit(
    state: State<AppState>,
    build: Option<String>,
    index: usize,
    address: String,
    change: ServerEdit,
) -> CmdResult<ServersView> {
    servers::edit(&path(&state, &build)?, index, &address, &change).map_err(text)?;
    view(&state, &build)
}

#[tauri::command]
pub fn server_remove(
    state: State<AppState>,
    build: Option<String>,
    index: usize,
    address: String,
) -> CmdResult<ServersView> {
    servers::remove(&path(&state, &build)?, index, &address).map_err(text)?;
    view(&state, &build)
}

#[tauri::command]
pub fn server_move(
    state: State<AppState>,
    build: Option<String>,
    index: usize,
    address: String,
    to: usize,
) -> CmdResult<ServersView> {
    servers::move_to(&path(&state, &build)?, index, &address, to).map_err(text)?;
    view(&state, &build)
}

/// Ответ одного сервера — событием «server-status», как только пришёл:
/// живые серверы не ждут тех, что не отвечают.
#[derive(Clone, Serialize)]
struct Probe {
    /// Опрос, к которому относится ответ (окно могло начать новый).
    token: u32,
    address: String,
    status: Option<Status>,
    /// Почему не ответил — словами; `kind`: `offline` или `bad` (адрес).
    error: Option<String>,
    kind: Option<&'static str>,
}

/// Опросить серверы. Иконки, которые прислали серверы, записываются в список —
/// как это делает игра.
#[tauri::command]
pub async fn servers_ping(
    app: AppHandle,
    state: State<'_, AppState>,
    build: Option<String>,
    addresses: Vec<String>,
    token: u32,
) -> CmdResult<()> {
    let file = path(&state, &build)?;
    let mut icons = Vec::new();
    ping::ping_each(addresses, |address, r| {
        let probe = match r {
            Ok(status) => {
                icons.push((address.to_string(), status.favicon.clone()));
                Probe {
                    token,
                    address: address.to_string(),
                    status: Some(status.clone()),
                    error: None,
                    kind: None,
                }
            }
            Err(e) => Probe {
                token,
                address: address.to_string(),
                status: None,
                kind: Some(e.kind()),
                error: Some(e.to_string()),
            },
        };
        let _ = app.emit("server-status", probe);
    })
    .await;
    blocking(move || {
        servers::update_icons(&file, &icons)
            .map(|_| ())
            .map_err(text)
    })
    .await
}

/// Сборки для «Играть на сервере», лучшие сверху. `version` и `modded` —
/// из опроса (сервер мог не ответить — тогда без них).
#[tauri::command]
pub async fn server_choices(
    state: State<'_, AppState>,
    address: String,
    version: Option<String>,
    modded: bool,
) -> CmdResult<Vec<Choice>> {
    let data = state.data()?;
    blocking(move || {
        let entries = Builds::new(data.clone()).list().map_err(text)?;
        let played = Stats::new(&data)
            .load()
            .map(|r| fit::played(&r))
            .unwrap_or_default();
        let releases = fit::cached_releases(&data);
        Ok(fit::choices(
            &entries,
            version.as_deref(),
            modded,
            played.get(&fit::key(&address)),
            &releases,
        ))
    })
    .await
}
