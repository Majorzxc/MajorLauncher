//! Скины для окна (раздел 5.9 ТЗ): библиотека, скин по нику и ссылке,
//! каталоги Ely.by и MineSkin, «Редактировать» во внешней программе со
//! слежением за файлом.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use majorlauncher_core::config::Config;
use majorlauncher_core::skins::{
    self, CatalogPage, CatalogQuery, Library, NickSource, Skin, SkinSettings, Textures,
};
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;

use crate::builds::blocking;
use crate::{AppState, CmdResult, text};

fn library(state: &State<AppState>) -> CmdResult<Library> {
    Ok(Library::new(&state.data()?))
}

#[tauri::command]
pub fn skins_list(state: State<AppState>) -> CmdResult<Vec<Skin>> {
    Ok(library(&state)?.list())
}

/// Картинка скина из библиотеки — байтами (окно рисует по пикселям).
#[tauri::command]
pub fn skin_png(state: State<AppState>, id: String) -> CmdResult<Response> {
    let path = library(&state)?.path(&id).map_err(text)?;
    Ok(Response::new(std::fs::read(&path).map_err(text)?))
}

#[tauri::command]
pub fn skin_add_file(state: State<AppState>, path: PathBuf) -> CmdResult<Skin> {
    library(&state)?.add_file(&path).map_err(text)
}

/// Скин игрока по нику — посмотреть перед добавлением.
#[tauri::command]
pub async fn skin_lookup(
    state: State<'_, AppState>,
    nick: String,
    from: NickSource,
) -> CmdResult<Textures> {
    skins::textures(&state.http, &nick, from)
        .await
        .map_err(text)
}

/// Картинка скина или плаща с Minecraft или Ely.by (с кэшем) — байтами.
#[tauri::command]
pub async fn skin_texture(state: State<'_, AppState>, url: String) -> CmdResult<Response> {
    let data = state.data()?;
    let bytes = skins::texture(&state.http, &data, &url)
        .await
        .map_err(text)?;
    Ok(Response::new(bytes))
}

/// Добавить в библиотеку картинку с сервиса скинов (ник, каталог).
#[tauri::command]
pub async fn skin_add_texture(
    state: State<'_, AppState>,
    url: String,
    name: String,
    source: String,
    page: Option<String>,
    slim: Option<bool>,
) -> CmdResult<Skin> {
    let data = state.data()?;
    let bytes = skins::texture(&state.http, &data, &url)
        .await
        .map_err(text)?;
    blocking(move || {
        Library::new(&data)
            .add(&bytes, &name, &source, page, slim)
            .map_err(text)
    })
    .await
}

/// Добавить по ссылке на PNG.
#[tauri::command]
pub async fn skin_add_url(state: State<'_, AppState>, url: String) -> CmdResult<Skin> {
    let data = state.data()?;
    let bytes = skins::download(&state.http, &url).await.map_err(text)?;
    let name = url
        .rsplit('/')
        .next()
        .and_then(|n| n.split(['?', '#']).next())
        .map(|n| n.trim_end_matches(".png").to_string())
        .filter(|n| !n.is_empty() && n.len() <= 40)
        .unwrap_or_else(|| "Скин по ссылке".into());
    blocking(move || {
        Library::new(&data)
            .add(&bytes, &name, "ссылка", None, None)
            .map_err(text)
    })
    .await
}

#[tauri::command]
pub fn skin_rename(state: State<AppState>, id: String, name: String) -> CmdResult<Skin> {
    library(&state)?.rename(&id, &name).map_err(text)
}

#[tauri::command]
pub fn skin_set_slim(state: State<AppState>, id: String, slim: bool) -> CmdResult<Skin> {
    library(&state)?.set_slim(&id, slim).map_err(text)
}

#[tauri::command]
pub fn skin_remove(state: State<AppState>, id: String) -> CmdResult<()> {
    library(&state)?.remove(&id).map_err(text)
}

/// «Сохранить как…».
#[tauri::command]
pub fn skin_export(state: State<AppState>, id: String, to: PathBuf) -> CmdResult<()> {
    let from = library(&state)?.path(&id).map_err(text)?;
    std::fs::copy(&from, &to).map(|_| ()).map_err(text)
}

#[tauri::command]
pub async fn skin_catalog(
    state: State<'_, AppState>,
    query: CatalogQuery,
) -> CmdResult<CatalogPage> {
    skins::catalog(&state.http, &query).await.map_err(text)
}

#[tauri::command]
pub fn set_skins(state: State<AppState>, skins: SkinSettings) -> CmdResult<Config> {
    let mut config = state.config.lock().unwrap();
    config.set_skins(skins).map_err(text)?;
    Ok(config.get().clone())
}

/// Файл скина изменился в редакторе; `error` — картинка перестала быть скином.
#[derive(Clone, serde::Serialize)]
struct Changed {
    id: String,
    error: Option<String>,
}

/// Какое слежение за файлом сейчас действует: новое «Редактировать» или
/// уход со вкладки скинов останавливают прежнее.
static WATCH: AtomicU64 = AtomicU64::new(0);

/// «Редактировать»: открыть скин в выбранной программе (или в программе
/// Windows для PNG) и следить за файлом: каждое сохранение — событие
/// `skin-changed`, окно обновляет 3D. Слежение — проверка даты файла раз в
/// полсекунды, пока открыта вкладка скинов (не дольше 4 часов).
#[tauri::command]
pub fn skin_edit(app: AppHandle, state: State<AppState>, id: String) -> CmdResult<()> {
    let data = state.data()?;
    let path = Library::new(&data).path(&id).map_err(text)?;
    let editor = state.config.lock().unwrap().get().skins.editor.clone();
    match editor {
        Some(exe) => {
            std::process::Command::new(&exe)
                .arg(&path)
                .spawn()
                .map_err(|e| format!("не удалось запустить {}: {e}", exe.display()))?;
        }
        None => app
            .opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(text)?,
    }
    let generation = WATCH.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        let modified = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let mut last: Option<SystemTime> = modified(&path);
        let started = Instant::now();
        while WATCH.load(Ordering::SeqCst) == generation
            && started.elapsed() < Duration::from_secs(4 * 3600)
        {
            std::thread::sleep(Duration::from_millis(500));
            let now = modified(&path);
            if now.is_some() && now != last {
                last = now;
                // Редактор мог ещё дописывать файл — дать ему закончить.
                std::thread::sleep(Duration::from_millis(200));
                let result = Library::new(&data).changed(&id);
                let _ = app.emit(
                    "skin-changed",
                    Changed {
                        id: id.clone(),
                        error: result.err().map(|e| e.to_string()),
                    },
                );
            }
        }
    });
    Ok(())
}

/// Перестать следить за файлом (ушли со вкладки скинов).
#[tauri::command]
pub fn skin_stop_watch() {
    WATCH.fetch_add(1, Ordering::SeqCst);
}

/// Показать файл скина в Проводнике (чтобы перетащить его на сайт).
#[tauri::command]
pub fn skin_reveal(app: AppHandle, state: State<AppState>, id: String) -> CmdResult<()> {
    let path = library(&state)?.path(&id).map_err(text)?;
    app.opener().reveal_item_in_dir(&path).map_err(text)
}

/// Скин, надетый на офлайн-аккаунт.
#[tauri::command]
pub fn skin_worn(state: State<AppState>, account: String) -> CmdResult<Option<String>> {
    Ok(skins::worn(&state.data()?, &account))
}

/// Надеть скин на офлайн-аккаунт (`None` — снять).
#[tauri::command]
pub fn skin_wear(state: State<AppState>, account: String, skin: Option<String>) -> CmdResult<()> {
    skins::set_worn(&state.data()?, &account, skin.as_deref()).map_err(text)
}

/// Сборка и стоит ли в ней CustomSkinLoader.
#[derive(serde::Serialize)]
pub struct LoaderBuild {
    id: String,
    name: String,
    has: bool,
    /// Ванильная сборка: мод туда не поставить.
    vanilla: bool,
}

/// Сборки (кроме быстрой игры) — где офлайн-скин будет виден в игре.
#[tauri::command]
pub async fn skin_loader_builds(state: State<'_, AppState>) -> CmdResult<Vec<LoaderBuild>> {
    let data = state.data()?;
    blocking(move || {
        let builds = majorlauncher_core::builds::Builds::new(data.clone());
        Ok(builds
            .list()
            .map_err(text)?
            .into_iter()
            .filter(|b| {
                !b.instance.quick && matches!(b.state, majorlauncher_core::builds::State::Active)
            })
            .map(|b| LoaderBuild {
                has: skins::has_skin_loader(&data.instance(&b.instance.id)),
                vanilla: b.instance.loader == majorlauncher_core::loaders::Loader::Vanilla,
                id: b.instance.id,
                name: b.instance.name,
            })
            .collect())
    })
    .await
}

/// Плащ офлайн-аккаунта байтами; пусто — плаща нет.
#[tauri::command]
pub fn skin_cape(state: State<AppState>, account: String) -> CmdResult<Response> {
    let path = skins::cape_path(&state.data()?, &account).map_err(text)?;
    Ok(Response::new(std::fs::read(&path).unwrap_or_default()))
}

/// Надеть плащ на офлайн-аккаунт: из файла или с сервиса скинов (плащ
/// найденного по нику игрока). Ничего — снять.
#[tauri::command]
pub async fn skin_cape_set(
    state: State<'_, AppState>,
    account: String,
    file: Option<PathBuf>,
    url: Option<String>,
) -> CmdResult<()> {
    let data = state.data()?;
    let bytes = match (file, url) {
        (Some(f), _) => Some(std::fs::read(&f).map_err(text)?),
        (None, Some(u)) => Some(skins::texture(&state.http, &data, &u).await.map_err(text)?),
        (None, None) => None,
    };
    skins::set_cape(&data, &account, bytes.as_deref()).map_err(text)
}
