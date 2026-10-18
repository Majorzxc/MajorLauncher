//! Сборки без повторной настройки (раздел 5.18 ТЗ): общие настройки игры,
//! библиотека ресурспаков и шейдеров, «Быстрая игра», копирование миров.

use std::collections::BTreeMap;
use std::path::PathBuf;

use majorlauncher_core::builds::{QUICK_ID, State as BuildState};
use majorlauncher_core::content::{self, Folder};
use majorlauncher_core::game::{self, RiskyWorld};
use majorlauncher_core::library::{Item, Library};
use majorlauncher_core::options::{self, Group, Profile, SETTINGS, Setting};
use majorlauncher_core::{Builds, DataDir, install};
use serde::Serialize;
use tauri::State;

use crate::builds::blocking;
use crate::{AppState, CmdResult, text};

#[derive(Serialize)]
pub struct OptionsView {
    settings: &'static [Setting],
    values: BTreeMap<String, String>,
    shared: Vec<Group>,
    seeded_from: Option<String>,
    /// Есть ли в профиле клавиши и ресурспаки (их правят в игре).
    keys: usize,
    packs: Option<Vec<String>>,
}

fn view(p: Profile) -> OptionsView {
    OptionsView {
        settings: SETTINGS,
        shared: Group::ALL.into_iter().filter(|g| p.is_shared(*g)).collect(),
        keys: p.keys.len(),
        packs: p.packs,
        seeded_from: p.seeded_from,
        values: p.values,
    }
}

/// Профиль пуст (лаунчер только обновился) — заполнить из последней
/// запущенной сборки: с этими настройками игрок и играет.
fn seed_from_recent(data: &DataDir, profile: &mut Profile) {
    let Ok(list) = Builds::new(data.clone()).list() else {
        return;
    };
    let mut active: Vec<_> = list
        .into_iter()
        .filter(|e| e.state == BuildState::Active && !e.instance.settings.own_options)
        .map(|e| e.instance)
        .collect();
    active.sort_by_key(|b| std::cmp::Reverse(b.last_played.unwrap_or(0)));
    for build in active {
        if options::seed(profile, data, &build).is_ok() {
            return;
        }
    }
}

#[tauri::command]
pub async fn options_get(state: State<'_, AppState>) -> CmdResult<OptionsView> {
    let data = state.data()?;
    blocking(move || {
        let mut profile = Profile::load(&data).map_err(text)?;
        if profile.is_empty() {
            seed_from_recent(&data, &mut profile);
            if !profile.is_empty() {
                profile.save(&data).map_err(text)?;
            }
        }
        Ok(view(profile))
    })
    .await
}

#[tauri::command]
pub fn options_set(state: State<AppState>, key: String, value: String) -> CmdResult<OptionsView> {
    let data = state.data()?;
    let mut profile = Profile::load(&data).map_err(text)?;
    profile.set(&key, &value).map_err(text)?;
    profile.save(&data).map_err(text)?;
    Ok(view(profile))
}

#[tauri::command]
pub fn options_set_shared(
    state: State<AppState>,
    group: Group,
    shared: bool,
) -> CmdResult<OptionsView> {
    let data = state.data()?;
    let mut profile = Profile::load(&data).map_err(text)?;
    profile.set_shared(group, shared);
    profile.save(&data).map_err(text)?;
    Ok(view(profile))
}

/// «Взять настройки из сборки».
#[tauri::command]
pub async fn options_seed(state: State<'_, AppState>, id: String) -> CmdResult<OptionsView> {
    let data = state.data()?;
    blocking(move || {
        let build = Builds::new(data.clone()).get(&id).map_err(text)?;
        let mut profile = Profile::load(&data).map_err(text)?;
        options::seed(&mut profile, &data, &build).map_err(text)?;
        profile.save(&data).map_err(text)?;
        Ok(view(profile))
    })
    .await
}

/// Какие миры «Быстрой игры» открывали в версии новее `version` — окно
/// предупредит, что перед запуском лаунчер сделает их копию.
#[tauri::command]
pub async fn quick_check(
    state: State<'_, AppState>,
    version: String,
) -> CmdResult<Vec<RiskyWorld>> {
    let data = state.data()?;
    let manifest = install::fetch_manifest(&data, &state.http)
        .await
        .map_err(text)?;
    Ok(game::risky_worlds(
        &data.instance(QUICK_ID),
        &version,
        &manifest,
    ))
}

#[tauri::command]
pub async fn build_copy_world(
    state: State<'_, AppState>,
    id: String,
    folder: String,
    to: String,
) -> CmdResult<String> {
    let builds = Builds::new(state.data()?);
    let from = builds.game_dir(&id).map_err(text)?;
    let target = builds.game_dir(&to).map_err(text)?;
    blocking(move || content::copy_world(&from, &folder, &target).map_err(text)).await
}

/// Библиотека модов: все моды игрока и где они стоят.
#[tauri::command]
pub async fn library_mods(
    state: State<'_, AppState>,
) -> CmdResult<majorlauncher_core::modlib::ModLibrary> {
    let data = state.data()?;
    blocking(move || majorlauncher_core::modlib::list(&data).map_err(text)).await
}

/// Добавить моды в сборку. Возвращает, сколько добавлено.
#[tauri::command]
pub async fn library_mods_add(
    state: State<'_, AppState>,
    keys: Vec<String>,
    build: String,
) -> CmdResult<usize> {
    let data = state.data()?;
    blocking(move || majorlauncher_core::modlib::add_to_build(&data, &keys, &build).map_err(text))
        .await
}

/// Удалить пустующие моды (`keys` — только эти). Возвращает освобождённые байты.
#[tauri::command]
pub async fn library_mods_remove_unused(
    state: State<'_, AppState>,
    keys: Option<Vec<String>>,
) -> CmdResult<u64> {
    let data = state.data()?;
    blocking(move || {
        majorlauncher_core::modlib::remove_unused(&data, keys.as_deref()).map_err(text)
    })
    .await
}

#[tauri::command]
pub fn library_list(state: State<AppState>) -> CmdResult<Vec<Item>> {
    Library::new(&state.data()?).list().map_err(text)
}

/// Добавить файлы, выбранные в диалоге. Возвращает ошибки по файлам, которые
/// не добавились (остальные добавлены).
#[tauri::command]
pub async fn library_add(
    state: State<'_, AppState>,
    folder: Folder,
    paths: Vec<String>,
) -> CmdResult<Vec<String>> {
    let data = state.data()?;
    blocking(move || {
        let lib = Library::new(&data);
        let errors: Vec<String> = paths
            .iter()
            .filter_map(|p| {
                lib.add(folder, &PathBuf::from(p))
                    .err()
                    .map(|e| e.to_string())
            })
            .collect();
        // Новое — сразу во все сборки, не дожидаясь запуска.
        lib.sync(&data).map_err(text)?;
        Ok(errors)
    })
    .await
}

/// «Где использовать» для одного или нескольких файлов; файлы сразу
/// раскладываются по сборкам (и убираются оттуда, где выбор снят).
#[tauri::command]
pub async fn library_set(
    state: State<'_, AppState>,
    folder: Folder,
    files: Vec<String>,
    all_builds: bool,
    builds: Vec<String>,
) -> CmdResult<()> {
    let data = state.data()?;
    blocking(move || {
        let lib = Library::new(&data);
        for file in &files {
            lib.set(folder, file, all_builds, builds.clone())
                .map_err(text)?;
        }
        lib.sync(&data).map_err(text)
    })
    .await
}

#[tauri::command]
pub async fn library_remove(
    state: State<'_, AppState>,
    folder: Folder,
    file: String,
) -> CmdResult<()> {
    let data = state.data()?;
    blocking(move || {
        let dirs: Vec<PathBuf> = Builds::new(data.clone())
            .list()
            .map_err(text)?
            .into_iter()
            .filter(|e| e.state == BuildState::Active)
            .map(|e| data.instance(&e.instance.id))
            .collect();
        Library::new(&data)
            .remove(folder, &file, &dirs)
            .map_err(text)
    })
    .await
}
