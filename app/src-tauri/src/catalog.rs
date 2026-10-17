//! Каталог Modrinth: поиск, страница проекта, установка. Окно передаёт
//! только id проектов и версий — адреса и хеши файлов ядро берёт у Modrinth само.

use majorlauncher_core::catalog::{self, Installed, Plan, SearchRequest, Target, VersionView};
use majorlauncher_core::modrinth::{self, Category, Project, ProjectType, SearchResult};

use majorlauncher_core::Builds;
use majorlauncher_core::paths::DataDir;
use tauri::{AppHandle, State};

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

/// Категории для фильтра каталога (свои у модов, ресурспаков, шейдеров).
#[tauri::command]
pub async fn catalog_categories(
    state: State<'_, AppState>,
    kind: ProjectType,
) -> CmdResult<Vec<Category>> {
    let data = state.data()?;
    catalog::categories(&state.http, &data, kind)
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

/// Сборка из каталога: новая сборка из `.mrpack` выбранной (или новейшей) версии.
#[tauri::command]
pub async fn modpack_install(
    app: AppHandle,
    state: State<'_, AppState>,
    project: String,
    version: Option<String>,
    label: Option<String>,
) -> CmdResult<majorlauncher_core::Instance> {
    let data = state.data()?;
    let task = crate::tasks::start(
        &app,
        label.unwrap_or_else(|| "Сборка из каталога".into()),
        "новая сборка",
    );
    let progress = task.progress();
    let result = majorlauncher_core::mrpack::install_from_catalog(
        &state.http,
        &data,
        &project,
        version.as_deref(),
        &progress,
    )
    .await;
    task.finish(&result);
    result.map_err(text)
}

/// Что внутри файла `.mrpack` — для окна перед импортом.
#[tauri::command]
pub async fn mrpack_inspect(path: String) -> CmdResult<majorlauncher_core::mrpack::PackInfo> {
    crate::builds::blocking(move || {
        majorlauncher_core::mrpack::inspect(std::path::Path::new(&path)).map_err(text)
    })
    .await
}

/// Новая сборка из файла `.mrpack`.
#[tauri::command]
pub async fn mrpack_import(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    name: Option<String>,
) -> CmdResult<majorlauncher_core::Instance> {
    let data = state.data()?;
    let file = std::path::PathBuf::from(&path);
    let title = file.file_stem().map_or_else(
        || "Сборка из файла".into(),
        |s| s.to_string_lossy().into_owned(),
    );
    let task = crate::tasks::start(&app, title, "новая сборка");
    let progress = task.progress();
    let result =
        majorlauncher_core::mrpack::install(&state.http, &data, &file, name.as_deref(), &progress)
            .await;
    task.finish(&result);
    result.map_err(text)
}

/// Куда ставится — для панели «Загрузки».
pub fn place(data: &DataDir, target: &Target) -> String {
    match target {
        Target::Build { id } => Builds::new(data.clone())
            .get(id)
            .map_or_else(|_| "в сборку".into(), |b| format!("в сборку «{}»", b.name)),
        Target::New { .. } => "в новую сборку".into(),
        Target::Library { .. } => "в библиотеку".into(),
    }
}

/// `label` — что ставится, для панели «Загрузки» (название проекта).
#[tauri::command]
pub async fn catalog_install(
    app: AppHandle,
    state: State<'_, AppState>,
    target: Target,
    kind: ProjectType,
    versions: Vec<String>,
    label: Option<String>,
) -> CmdResult<Vec<String>> {
    let data = state.data()?;
    let task = crate::tasks::start(
        &app,
        label.unwrap_or_else(|| "Установка из каталога".into()),
        place(&data, &target),
    );
    let progress = task.progress();
    let result = catalog::install(&state.http, &data, &target, kind, &versions, &progress).await;
    task.finish(&result);
    result.map_err(text)
}
