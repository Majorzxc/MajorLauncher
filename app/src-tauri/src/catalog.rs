//! Каталог Modrinth: поиск, страница проекта, установка. Окно передаёт
//! только id проектов и версий — адреса и хеши файлов ядро берёт у Modrinth само.

use majorlauncher_core::catalog::{self, Installed, Plan, SearchRequest, Target, VersionView};
use majorlauncher_core::modrinth::{self, Project, ProjectType, SearchResult};

use tauri::State;

use crate::{AppState, CmdResult, text};

#[tauri::command]
pub async fn catalog_search(
    state: State<'_, AppState>,
    req: SearchRequest,
) -> CmdResult<SearchResult> {
    let data = state.data()?;
    catalog::search(&state.http, &data, &req)
        .await
        .map_err(text)
}

#[tauri::command]
pub async fn catalog_project(state: State<'_, AppState>, id: String) -> CmdResult<Project> {
    modrinth::project(&state.http, &id).await.map_err(text)
}

#[tauri::command]
pub async fn catalog_versions(
    state: State<'_, AppState>,
    target: Target,
    kind: ProjectType,
    project: String,
    all: bool,
) -> CmdResult<Vec<VersionView>> {
    let data = state.data()?;
    catalog::versions(&state.http, &data, &target, kind, &project, all)
        .await
        .map_err(text)
}

#[tauri::command]
pub fn catalog_installed(state: State<AppState>, target: Target) -> CmdResult<Vec<Installed>> {
    catalog::installed(&state.data()?, &target).map_err(text)
}

#[tauri::command]
pub async fn catalog_plan(
    state: State<'_, AppState>,
    target: Target,
    kind: ProjectType,
    project: String,
    version: Option<String>,
) -> CmdResult<Plan> {
    let data = state.data()?;
    catalog::plan(
        &state.http,
        &data,
        &target,
        kind,
        &project,
        version.as_deref(),
    )
    .await
    .map_err(text)
}

#[tauri::command]
pub async fn catalog_install(
    state: State<'_, AppState>,
    target: Target,
    kind: ProjectType,
    versions: Vec<String>,
) -> CmdResult<Vec<String>> {
    let data = state.data()?;
    catalog::install(&state.http, &data, &target, kind, &versions, &|_| {})
        .await
        .map_err(text)
}
