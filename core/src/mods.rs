//! Моды в сборке (разделы 5.4 и 5.6 ТЗ): что это за мод (Modrinth по хешу
//! файла или сведения из самого jar), есть ли обновление и что с ним не так.
//!
//! Всё дорогое кэшируется по SHA-1 файла: хеш (по размеру и дате), сведения
//! из jar и проверка на вредоносный код — в `cache/mods-meta.json`, ответы
//! Modrinth — в `cache/modrinth-files.json`. Повторная проверка сборки — это
//! чтение папки и двух небольших файлов.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::builds::{Builds, Instance, now};
use crate::catalog::{self, Target};
use crate::content::{self, Folder};
use crate::download::Downloader;
use crate::error::{Error, IoContext, Result};
use crate::library::{Library, is_same};
use crate::loaders::Loader;
use crate::modmeta::{self, DepKind, JarMeta, ModLoader};
use crate::modrinth::{self, ProjectType};
use crate::paths::DataDir;
use crate::verify::VerifyCache;

/// Неопознанный файл спрашиваем у Modrinth снова не раньше чем через неделю.
const RECHECK: u64 = 7 * 24 * 3600;

fn release() -> String {
    "release".into()
}

/// Мод, опознанный на Modrinth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Known {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    /// `release`, `beta`, `alpha`.
    #[serde(default = "release")]
    pub version_type: String,
    pub icon_url: Option<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    /// Обязательные зависимости (id проектов).
    pub requires: Vec<String>,
    /// Зависимости ровно этой версии (id версий): такие моды не обновляем
    /// по отдельности — обновление сломало бы этот мод.
    #[serde(default)]
    pub requires_exact: Vec<String>,
    /// Заявленные несовместимости (id проектов).
    pub incompatible: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    pub version_id: String,
    pub version_number: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModInfo {
    /// Имя файла в папке модов (с `.disabled`, если выключен).
    pub file: String,
    /// Название для показа.
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub size: u64,
    pub sha1: String,
    pub loaders: Vec<ModLoader>,
    pub modrinth: Option<Known>,
    /// Игрок сказал «доверять этому файлу».
    pub trusted: bool,
    /// Не обновлять.
    pub pinned: bool,
    pub update: Option<Update>,
    pub malware: Option<String>,
}

/// Уровень: блокирует запуск, предупреждение, совет (раздел 5.6 ТЗ).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Block,
    Warn,
    Tip,
}

/// Что можно сделать с предупреждением.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    /// Выключить мод.
    Disable { file: String },
    /// Удалить файл совсем (вредоносный код).
    Delete { file: String },
    /// Больше не предупреждать про этот файл.
    Trust { sha1: String },
    /// Поставить проект из каталога (недостающая зависимость, Iris для шейдеров).
    Install { project: String, title: String },
    /// Найти в каталоге.
    Search { query: String },
    /// Открыть настройки сборки — поменять загрузчик.
    Loader,
    /// Скачать эту версию заново (файл изменён).
    Redownload { version_id: String, file: String },
    /// Выключить лишние копии мода, оставить подходящую.
    DisableExtra { disable: Vec<String> },
    /// Обновить мод.
    Update { version_id: String, file: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub level: Level,
    pub text: String,
    /// К какому моду (файл в списке); `None` — ко всей сборке.
    pub file: Option<String>,
    pub action: Option<Action>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub mods: Vec<ModInfo>,
    pub issues: Vec<Issue>,
    /// Modrinth ответил: опознание и обновления свежие.
    pub online: bool,
}

impl Report {
    pub fn blocking(&self) -> Vec<&Issue> {
        self.issues
            .iter()
            .filter(|i| i.level == Level::Block)
            .collect()
    }
}

/// Файл мода, прочитанный с диска.
#[derive(Debug, Clone)]
pub struct Local {
    pub file: String,
    pub enabled: bool,
    pub size: u64,
    pub modified: u64,
    pub sha1: String,
    pub meta: JarMeta,
    pub malware: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct MetaCache {
    #[serde(default)]
    files: HashMap<String, MetaEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
struct MetaEntry {
    meta: JarMeta,
    malware: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct IdentCache {
    /// Формат кэша: поменялся — опознаём заново.
    #[serde(default)]
    version: u32,
    #[serde(default)]
    files: HashMap<String, Ident>,
}

const IDENT_VERSION: u32 = 2;

#[derive(Clone, Serialize, Deserialize)]
struct Ident {
    known: Option<Known>,
    checked: u64,
}

fn read_json<T: Default + serde::de::DeserializeOwned>(path: &Path) -> T {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).at(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(value).expect("кэш сериализуется")).at(&tmp)?;
    std::fs::rename(&tmp, path).at(path)
}

fn trusted_path(data: &DataDir) -> PathBuf {
    data.root().join("trusted-mods.json")
}

fn trusted(data: &DataDir) -> HashSet<String> {
    read_json::<Vec<String>>(&trusted_path(data))
        .into_iter()
        .collect()
}

/// «Доверять этому файлу»: про него больше не предупреждать.
pub fn trust(data: &DataDir, sha1: &str) -> Result<()> {
    let mut list: Vec<String> = read_json(&trusted_path(data));
    if !list.iter().any(|s| s.eq_ignore_ascii_case(sha1)) {
        list.push(sha1.to_lowercase());
    }
    write_json(&trusted_path(data), &list)
}

/// Прочитать моды сборки с диска: хеши, сведения из jar, вредоносный код.
/// Работа с диском — вызывать вне асинхронного потока.
pub fn local(data: &DataDir, build: &str) -> Result<Vec<Local>> {
    let game_dir = Builds::new(data.clone()).game_dir(build)?;
    let files = content::files(&game_dir, Folder::Mods)?;
    let hashes = VerifyCache::load(data.verified());
    let meta_path = data.cache().join("mods-meta.json");
    let mut cache: MetaCache = read_json(&meta_path);
    let mut dirty = false;
    let mut out = Vec::new();
    for f in files.into_iter().filter(|f| !f.is_dir) {
        let path = game_dir.join(Folder::Mods.dir()).join(&f.file);
        let Ok(sha1) = hashes.sha1(&path) else {
            continue;
        };
        let entry = match cache.files.get(&sha1) {
            Some(e) => e.clone(),
            None => {
                let e = MetaEntry {
                    meta: modmeta::read(&path),
                    malware: crate::malware::scan(&path),
                };
                cache.files.insert(sha1.clone(), e.clone());
                dirty = true;
                e
            }
        };
        out.push(Local {
            file: f.file,
            enabled: f.enabled,
            size: f.size,
            modified: f.modified,
            sha1,
            meta: entry.meta,
            malware: entry.malware,
        });
    }
    hashes.save()?;
    if dirty {
        write_json(&meta_path, &cache)?;
    }
    Ok(out)
}

/// Пересчитать хеши модов сборки мимо кэша («Проверить файлы»): порчу, при
/// которой размер и дата файла не менялись, видит только пересчёт. Следующий
/// [`report`] сравнит свежие хеши с каталогом. Работа с диском.
pub fn rehash(data: &DataDir, build: &str) -> Result<()> {
    let game_dir = Builds::new(data.clone()).game_dir(build)?;
    let hashes = VerifyCache::load(data.verified());
    for f in content::files(&game_dir, Folder::Mods)? {
        if !f.is_dir {
            hashes.sha1_full(&game_dir.join(Folder::Mods.dir()).join(&f.file))?;
        }
    }
    hashes.save()
}

/// Что Modrinth сказал о файлах: опознанные, неизвестные ему и ответил ли он.
pub struct Identified {
    pub known: HashMap<String, Known>,
    /// Modrinth проверял эти хеши и не знает их.
    pub unknown: HashSet<String>,
    pub answered: bool,
}

/// Опознать файлы на Modrinth. Без сети (`online = false`) — только кэш.
pub async fn identify(
    dl: &Downloader,
    data: &DataDir,
    hashes: &[String],
    online: bool,
) -> Identified {
    let path = data.cache().join("modrinth-files.json");
    let mut cache: IdentCache = read_json(&path);
    if cache.version != IDENT_VERSION {
        cache = IdentCache {
            version: IDENT_VERSION,
            ..Default::default()
        };
    }
    let stale: Vec<String> = hashes
        .iter()
        .filter(|h| {
            cache
                .files
                .get(*h)
                .is_none_or(|i| i.known.is_none() && now().saturating_sub(i.checked) > RECHECK)
        })
        .cloned()
        .collect();
    let mut answered = !online || stale.is_empty();
    if online
        && !stale.is_empty()
        && let Ok(found) = modrinth::by_hashes(dl, &stale).await
    {
        answered = true;
        let ids: Vec<&str> = found.values().map(|v| v.project_id.as_str()).collect();
        let briefs: HashMap<String, modrinth::Brief> = modrinth::briefs(dl, &ids)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|b| (b.id.clone(), b))
            .collect();
        for h in &stale {
            let known = found.get(h).map(|v| {
                let brief = briefs.get(&v.project_id);
                let deps = |kind| {
                    v.dependencies
                        .iter()
                        .filter(|d| d.dependency_type == kind)
                        .filter_map(|d| d.project_id.clone())
                        .collect()
                };
                Known {
                    project_id: v.project_id.clone(),
                    version_id: v.id.clone(),
                    title: brief.map_or_else(|| v.name.clone(), |b| b.title.clone()),
                    version_number: v.version_number.clone(),
                    version_type: v.version_type.clone(),
                    icon_url: brief.and_then(|b| b.icon_url.clone()),
                    game_versions: v.game_versions.clone(),
                    loaders: v.loaders.clone(),
                    requires: deps(modrinth::DependencyType::Required),
                    requires_exact: v
                        .dependencies
                        .iter()
                        .filter(|d| d.dependency_type == modrinth::DependencyType::Required)
                        .filter_map(|d| d.version_id.clone())
                        .collect(),
                    incompatible: deps(modrinth::DependencyType::Incompatible),
                }
            });
            cache.files.insert(
                h.clone(),
                Ident {
                    known,
                    checked: now(),
                },
            );
        }
        let _ = write_json(&path, &cache);
    }
    let mut known = HashMap::new();
    let mut unknown = HashSet::new();
    for h in hashes {
        match cache.files.get(h).map(|i| &i.known) {
            Some(Some(k)) => {
                known.insert(h.clone(), k.clone());
            }
            Some(None) => {
                unknown.insert(h.clone());
            }
            None => {}
        }
    }
    Identified {
        known,
        unknown,
        answered,
    }
}

/// Новые версии опознанных модов под сборку. Без сети или если Modrinth не
/// ответил — то, что он ответил в прошлый раз (`cache/modrinth-updates.json`).
pub async fn updates(
    dl: &Downloader,
    data: &DataDir,
    instance: &Instance,
    known: &HashMap<String, Known>,
    online: bool,
) -> HashMap<String, Update> {
    let hashes: Vec<String> = known.keys().cloned().collect();
    let loaders = modrinth::loader_names(instance.loader, &instance.version);
    // Обновление зависит от загрузчика и версии игры, не только от файла.
    let key = |h: &str| format!("{}|{}|{h}", loaders.join(","), instance.version);
    let path = data.cache().join("modrinth-updates.json");
    let mut cache: HashMap<String, Update> = read_json(&path);
    let cached = |cache: &HashMap<String, Update>| {
        hashes
            .iter()
            .filter_map(|h| cache.get(&key(h)).map(|u| (h.clone(), u.clone())))
            .collect()
    };
    if !online || hashes.is_empty() {
        return cached(&cache);
    }
    let Ok(found) = modrinth::updates(dl, &hashes, &loaders, &instance.version).await else {
        return cached(&cache);
    };
    let mut out = HashMap::new();
    for (h, v) in found {
        let Some(k) = known.get(&h) else { continue };
        if k.project_id != v.project_id {
            continue;
        }
        // Стоит релиз, а новейшая — бета: ищем последний релиз.
        let v = if v.version_type != "release" && k.version_type == "release" {
            let list = modrinth::versions(dl, &k.project_id, &loaders, Some(&instance.version))
                .await
                .unwrap_or_default();
            match list.into_iter().find(|x| x.version_type == "release") {
                Some(r) => r,
                None => continue,
            }
        } else {
            v
        };
        if v.id != k.version_id {
            out.insert(
                h,
                Update {
                    version_id: v.id,
                    version_number: v.version_number,
                },
            );
        }
    }
    for h in &hashes {
        match out.get(h) {
            Some(u) => cache.insert(key(h), u.clone()),
            None => cache.remove(&key(h)),
        };
    }
    let _ = write_json(&path, &cache);
    out
}

/// Полный отчёт: диск, Modrinth (если `online`), проверки.
pub async fn report(dl: &Downloader, data: &DataDir, build: &str, online: bool) -> Result<Report> {
    let instance = Builds::new(data.clone()).get(build)?;
    let local = {
        let (data, build) = (data.clone(), build.to_string());
        tokio::task::spawn_blocking(move || local(&data, &build))
            .await
            .map_err(|e| Error::Other(format!("проверка модов прервана: {e}")))??
    };
    let hashes: Vec<String> = local.iter().map(|l| l.sha1.clone()).collect();
    let ids = identify(dl, data, &hashes, online).await;
    let updates = updates(dl, data, &instance, &ids.known, online).await;
    let game_dir = Builds::new(data.clone()).game_dir(build)?;
    let shaders = !content::files(&game_dir, Folder::ShaderPacks)?.is_empty();
    let library = library_sha1(data, &game_dir);
    Ok(check(&Context {
        instance: &instance,
        local: &local,
        known: &ids.known,
        unknown: &ids.unknown,
        updates: &updates,
        trusted: &trusted(data),
        library: &library,
        shaders,
        online: ids.answered && online,
    }))
}

/// Моды сборки, которые лежат из библиотеки каталога: файл → (sha1 из каталога, версия).
fn library_sha1(data: &DataDir, game_dir: &Path) -> HashMap<String, (String, String)> {
    let Ok(items) = Library::new(data).list() else {
        return HashMap::new();
    };
    items
        .into_iter()
        .filter(|i| i.folder == Folder::Mods)
        .filter_map(|i| {
            let s = i.source?;
            let here = game_dir.join(Folder::Mods.dir()).join(&i.file);
            here.exists()
                .then(|| (i.file.clone(), (s.sha1, s.version_id)))
        })
        .collect()
}

/// Всё, что нужно проверкам.
pub struct Context<'a> {
    pub instance: &'a Instance,
    pub local: &'a [Local],
    pub known: &'a HashMap<String, Known>,
    /// Modrinth проверял эти хеши и не знает их.
    pub unknown: &'a HashSet<String>,
    pub updates: &'a HashMap<String, Update>,
    pub trusted: &'a HashSet<String>,
    /// Файл → (SHA-1 из каталога, версия): лежит из библиотеки.
    pub library: &'a HashMap<String, (String, String)>,
    /// В сборке есть шейдеры.
    pub shaders: bool,
    pub online: bool,
}

/// id, которые не моды: их «наличие» проверяет сам загрузчик.
const SYSTEM: &[&str] = &[
    "minecraft",
    "java",
    "fabricloader",
    "fabric-loader",
    "quilt_loader",
    "quilt_base",
    "forge",
    "neoforge",
    "fml",
    "javafml",
    "lowcodefml",
    "mcp",
];

/// Подходит ли версия игры. Forge и NeoForge на деле не так строги, как
/// пишут авторы (Iris для 1.21.1 заявляет `[1.21,1.21.1)` и всё равно
/// грузится), поэтому для них хватает той же линейки — 1.21.x.
fn mc_ok(d: &modmeta::Dep, game: &str) -> bool {
    if d.accepts(game) {
        return true;
    }
    if d.style != modmeta::RangeStyle::Maven {
        return false;
    }
    let line =
        |v: &str| -> Vec<u64> { v.split('.').take(2).map_while(|p| p.parse().ok()).collect() };
    d.ranges.iter().any(|r| {
        let lower = r
            .trim_start_matches(['[', '('])
            .split(',')
            .next()
            .unwrap_or_default()
            .trim();
        !lower.is_empty() && line(lower) == line(game)
    })
}

/// Копия мода подходит к сборке по версии игры и загрузчику: из нескольких
/// копий одного мода оставлять надо её.
fn fits(l: &Local, known: Option<&Known>, build: Loader, game: &str) -> bool {
    let mc = l
        .meta
        .mods
        .iter()
        .flat_map(|m| &m.depends)
        .filter(|d| d.kind == DepKind::Required && d.id == "minecraft")
        .all(|d| mc_ok(d, game));
    let mc = mc
        && (!l.meta.mods.is_empty()
            || known.is_none_or(|k| k.game_versions.iter().any(|g| g == game)));
    mc && (l.meta.loaders.is_empty() || loader_ok(build, game, &l.meta.loaders))
}

/// Зависимость требует ровно эту версию или ограничивает её сверху: такой
/// мод не обновляем — новая версия выйдет за границу.
fn caps(d: &modmeta::Dep) -> bool {
    d.ranges.iter().any(|r| match d.style {
        modmeta::RangeStyle::Semver => r
            .split_whitespace()
            .any(|c| c != "*" && !c.starts_with('>')),
        modmeta::RangeStyle::Maven => {
            let r = r.trim();
            r.starts_with(['[', '(']) && !r.ends_with(",)") && !r.ends_with(",]")
        }
    })
}

/// Версия мода, по которой можно судить: не пустая и не заготовка `${version}`.
fn real_version(v: Option<&String>) -> Option<&str> {
    v.map(String::as_str)
        .filter(|v| !v.is_empty() && !v.contains("${"))
}

/// Несовместимость (`breaks`, `conflicts`) касается стоящей версии. Версию не
/// узнать (id приносит другой мод) — судим только по «любая версия».
fn hits(d: &modmeta::Dep, version: Option<&String>) -> bool {
    match real_version(version) {
        Some(v) => d.accepts(v),
        None => d.ranges.iter().all(|r| r.trim() == "*"),
    }
}

fn loader_ok(build: Loader, game: &str, mod_loaders: &[ModLoader]) -> bool {
    mod_loaders.iter().any(|l| match (build, l) {
        (Loader::Fabric, ModLoader::Fabric) => true,
        (Loader::Forge, ModLoader::Forge) => true,
        (Loader::NeoForge, ModLoader::NeoForge) => true,
        (Loader::NeoForge, ModLoader::Forge) => game == "1.20.1",
        _ => false,
    })
}

fn loader_names(list: &[ModLoader]) -> String {
    list.iter()
        .map(|l| match l {
            ModLoader::Fabric => "Fabric",
            ModLoader::Quilt => "Quilt",
            ModLoader::Forge => "Forge",
            ModLoader::NeoForge => "NeoForge",
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

/// Проверки (раздел 5.6 ТЗ, «Моды»). Без диска и сети — легко проверять.
pub fn check(cx: &Context) -> Report {
    let inst = cx.instance;
    let game = inst.version.as_str();
    let mut issues = Vec::new();
    let mut mods = Vec::new();

    // Один мод несколькими копиями. Оставить надо подходящую к сборке (а из
    // них — новую); остальные лишние. Fabric и Quilt сами грузят подходящую
    // копию, Forge и NeoForge на копиях падают.
    let mut groups: HashMap<String, Vec<&Local>> = HashMap::new();
    for l in cx.local.iter().filter(|l| l.enabled) {
        let key = l
            .meta
            .mods
            .first()
            .map(|m| m.id.clone())
            .or_else(|| cx.known.get(&l.sha1).map(|k| k.project_id.clone()));
        if let Some(key) = key {
            groups.entry(key).or_default().push(l);
        }
    }
    let version_of = |l: &Local| {
        cx.known
            .get(&l.sha1)
            .map(|k| k.version_number.clone())
            .or_else(|| l.meta.mods.first().map(|m| m.version.clone()))
            .unwrap_or_default()
    };
    let mut copies: Vec<(&Local, Vec<&Local>)> = Vec::new();
    for list in groups.into_values().filter(|l| l.len() > 1) {
        let fit: Vec<&Local> = list
            .iter()
            .copied()
            .filter(|l| fits(l, cx.known.get(&l.sha1), inst.loader, game))
            .collect();
        let pool = if fit.is_empty() { &list } else { &fit };
        let keep = *pool
            .iter()
            .max_by(|a, b| crate::loaders::compare_versions(&version_of(a), &version_of(b)))
            .expect("в группе есть моды");
        let extra = list.into_iter().filter(|l| l.file != keep.file).collect();
        copies.push((keep, extra));
    }
    let spare: HashSet<&str> = copies
        .iter()
        .flat_map(|(_, extra)| extra.iter().map(|l| l.file.as_str()))
        .collect();

    let enabled: Vec<&Local> = cx
        .local
        .iter()
        .filter(|l| l.enabled && !spare.contains(l.file.as_str()))
        .collect();
    // Что стоит: id из jar (с версиями), то, что моды приносят с собой, и проекты Modrinth.
    let mut present: HashSet<String> = HashSet::new();
    let mut versions: HashMap<String, String> = HashMap::new();
    let mut projects: HashSet<String> = HashSet::new();
    // Версии, которые другой мод требует ровно такими: их не обновляем.
    let mut held: HashSet<String> = HashSet::new();
    // То же по сведениям из jar: id модов, версию которых другой мод держит.
    let mut held_ids: HashSet<String> = HashSet::new();
    for l in &enabled {
        for m in &l.meta.mods {
            present.insert(m.id.clone());
            versions.insert(m.id.clone(), m.version.clone());
            for d in &m.depends {
                if d.kind == DepKind::Required && d.id != m.id && caps(d) {
                    held_ids.insert(d.id.clone());
                }
            }
        }
        present.extend(l.meta.provides.iter().cloned());
        if let Some(k) = cx.known.get(&l.sha1) {
            projects.insert(k.project_id.clone());
            held.extend(k.requires_exact.iter().cloned());
        }
    }

    if inst.loader == Loader::Vanilla && !enabled.is_empty() {
        issues.push(Issue {
            level: Level::Block,
            text:
                "У сборки нет загрузчика — моды не загрузятся. Выберите Fabric, Forge или NeoForge."
                    .into(),
            file: None,
            action: Some(Action::Loader),
        });
    }

    for l in cx.local {
        let known = cx.known.get(&l.sha1);
        let first = l.meta.mods.first();
        let name = known
            .map(|k| k.title.clone())
            .or_else(|| first.map(|m| m.name.clone()))
            .unwrap_or_else(|| {
                l.file
                    .trim_end_matches(".disabled")
                    .trim_end_matches(".jar")
                    .to_string()
            });
        let version = known
            .map(|k| k.version_number.clone())
            .or_else(|| first.map(|m| m.version.clone()))
            .unwrap_or_default();
        let pinned = known.is_some_and(|k| inst.pinned_mods.contains(&k.project_id));
        // Версию держит другой мод (требует ровно её) — обновлять нельзя.
        let held_here = known.is_some_and(|k| held.contains(&k.version_id))
            || first.is_some_and(|m| held_ids.contains(&m.id));
        let update = cx
            .updates
            .get(&l.sha1)
            .cloned()
            .filter(|_| !held_here && !spare.contains(l.file.as_str()));
        let trusted = cx.trusted.contains(&l.sha1);
        let file = Some(l.file.clone());
        let issue = |level, text: String, action| Issue {
            level,
            text,
            file: file.clone(),
            action,
        };

        if let Some(what) = &l.malware {
            issues.push(issue(
                Level::Block,
                format!("«{name}»: найден известный вредоносный код ({what}). Удалите файл и смените пароли, если играли с ним."),
                Some(Action::Delete { file: l.file.clone() }),
            ));
        } else if l.enabled && !spare.contains(l.file.as_str()) {
            let disable = || {
                Some(Action::Disable {
                    file: l.file.clone(),
                })
            };
            // Загрузчик.
            let mod_loaders: Vec<ModLoader> = if l.meta.loaders.is_empty() {
                known
                    .map(|k| {
                        k.loaders
                            .iter()
                            .filter_map(|x| match x.as_str() {
                                "fabric" => Some(ModLoader::Fabric),
                                "quilt" => Some(ModLoader::Quilt),
                                "forge" => Some(ModLoader::Forge),
                                "neoforge" => Some(ModLoader::NeoForge),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                l.meta.loaders.clone()
            };
            if inst.loader != Loader::Vanilla
                && !mod_loaders.is_empty()
                && !loader_ok(inst.loader, game, &mod_loaders)
            {
                issues.push(issue(
                    Level::Block,
                    format!(
                        "«{name}» — мод для {}, а сборка на {}.",
                        loader_names(&mod_loaders),
                        inst.loader.title()
                    ),
                    disable(),
                ));
            }
            // Зависимости и несовместимости из jar.
            for m in &l.meta.mods {
                for d in &m.depends {
                    match d.kind {
                        DepKind::Required if d.id == "minecraft" => {
                            if !mc_ok(d, game) {
                                issues.push(issue(
                                    Level::Block,
                                    format!(
                                        "«{name}» не для Minecraft {game} (нужна {}).",
                                        d.ranges.join(" или ")
                                    ),
                                    disable(),
                                ));
                            }
                        }
                        DepKind::Required
                            if matches!(
                                (inst.loader, d.id.as_str()),
                                (Loader::Fabric, "fabricloader")
                                    | (Loader::Forge, "forge")
                                    | (Loader::NeoForge, "neoforge")
                            ) =>
                        {
                            if let Some(lv) = &inst.loader_version
                                && !d.accepts(lv)
                            {
                                issues.push(issue(
                                    Level::Block,
                                    format!(
                                        "«{name}» нужен {} {}, а стоит {lv}.",
                                        inst.loader.title(),
                                        d.ranges.join(" или ")
                                    ),
                                    Some(Action::Loader),
                                ));
                            }
                        }
                        DepKind::Required if SYSTEM.contains(&d.id.as_str()) => {}
                        // Нужный мод стоит, но не той версии.
                        DepKind::Required
                            if versions.get(&d.id).is_some_and(|v| {
                                !v.is_empty() && !v.contains("${") && !d.accepts(v)
                            }) =>
                        {
                            issues.push(issue(
                                Level::Block,
                                format!(
                                    "«{name}» нужен «{}» версии {}, а стоит {}.{}",
                                    d.id,
                                    d.ranges.join(" или "),
                                    versions[&d.id],
                                    if update.is_some() {
                                        " Новая версия этого мода, скорее всего, подходит."
                                    } else {
                                        ""
                                    }
                                ),
                                // Свежая версия мода обычно собрана под свежую
                                // зависимость (XaeroPlus под новый Xaero's World Map).
                                Some(match &update {
                                    Some(u) => Action::Update {
                                        version_id: u.version_id.clone(),
                                        file: l.file.clone(),
                                    },
                                    None => Action::Search {
                                        query: d.id.clone(),
                                    },
                                }),
                            ));
                        }
                        DepKind::Required if !present.contains(&d.id) => {
                            // Если Modrinth знает, какого проекта не хватает, — его и предложим.
                            let missing_project = known
                                .and_then(|k| k.requires.iter().find(|p| !projects.contains(*p)))
                                .cloned();
                            let action = match missing_project {
                                Some(project) => Some(Action::Install {
                                    project,
                                    title: d.id.clone(),
                                }),
                                None => Some(Action::Search {
                                    query: d.id.clone(),
                                }),
                            };
                            issues.push(issue(
                                Level::Block,
                                format!("Для «{name}» нужен мод «{}» — его нет в сборке.", d.id),
                                action,
                            ));
                        }
                        DepKind::Breaks
                            if present.contains(&d.id)
                                && first.is_none_or(|f| f.id != d.id)
                                && hits(d, versions.get(&d.id)) =>
                        {
                            issues.push(issue(
                                Level::Block,
                                format!(
                                    "«{name}» несовместим с «{}»: вместе игра не запустится.",
                                    d.id
                                ),
                                disable(),
                            ));
                        }
                        DepKind::Conflicts
                            if present.contains(&d.id) && hits(d, versions.get(&d.id)) =>
                        {
                            issues.push(issue(
                                Level::Warn,
                                format!(
                                    "Автор «{name}» предупреждает о проблемах вместе с «{}».",
                                    d.id
                                ),
                                None,
                            ));
                        }
                        _ => {}
                    }
                }
            }
            if let Some(k) = known {
                if l.meta.mods.is_empty() {
                    for p in k.requires.iter().filter(|p| !projects.contains(*p)) {
                        issues.push(issue(
                            Level::Block,
                            format!(
                                "Для «{name}» нужен ещё один мод с Modrinth — его нет в сборке."
                            ),
                            Some(Action::Install {
                                project: p.clone(),
                                title: p.clone(),
                            }),
                        ));
                    }
                }
                if let Some(p) = k.incompatible.iter().find(|p| projects.contains(*p)) {
                    let other = cx
                        .known
                        .values()
                        .find(|o| &o.project_id == p)
                        .map_or_else(|| p.clone(), |o| o.title.clone());
                    issues.push(issue(
                        Level::Block,
                        format!("«{name}» несовместим с «{other}»."),
                        disable(),
                    ));
                }
                if !k.game_versions.iter().any(|g| g == game) {
                    issues.push(issue(
                        Level::Warn,
                        format!(
                            "«{name}» на Modrinth не отмечен для {game} — может не заработать."
                        ),
                        None,
                    ));
                }
            }
            // Файл из каталога изменён.
            let changed = cx
                .library
                .get(&l.file)
                .filter(|(sha1, _)| !sha1.is_empty() && !sha1.eq_ignore_ascii_case(&l.sha1));
            if let Some((_, version_id)) = changed {
                issues.push(issue(
                    Level::Block,
                    format!("Файл «{name}» изменён или повреждён — не совпадает с каталогом."),
                    Some(Action::Redownload {
                        version_id: version_id.clone(),
                        file: l.file.clone(),
                    }),
                ));
            }
            if cx.unknown.contains(&l.sha1) && !trusted && changed.is_none() {
                // Две копии одного мода: по названию не понять, о каком файле речь.
                let twins = cx
                    .local
                    .iter()
                    .filter(|o| o.meta.mods.first().map(|m| &m.id) == first.map(|m| &m.id))
                    .count();
                let what = if first.is_some() && twins > 1 {
                    format!("«{name}» ({})", l.file)
                } else {
                    format!("«{name}»")
                };
                issues.push(issue(
                    Level::Warn,
                    format!("{what} не из каталога Modrinth и не опознан. Если файл ваш или вы ему доверяете — отметьте."),
                    Some(Action::Trust { sha1: l.sha1.clone() }),
                ));
            }
            if let Some(u) = &update
                && !pinned
            {
                issues.push(issue(
                    Level::Tip,
                    format!(
                        "Есть обновление «{name}»: {version} → {}.",
                        u.version_number
                    ),
                    Some(Action::Update {
                        version_id: u.version_id.clone(),
                        file: l.file.clone(),
                    }),
                ));
            }
        }

        mods.push(ModInfo {
            file: l.file.clone(),
            name,
            version,
            enabled: l.enabled,
            size: l.size,
            sha1: l.sha1.clone(),
            loaders: l.meta.loaders.clone(),
            modrinth: known.cloned(),
            trusted,
            pinned,
            update,
            malware: l.malware.clone(),
        });
    }

    // Один мод несколькими копиями.
    let fabric_like = inst.loader == Loader::Fabric;
    for (keep, extra) in &copies {
        let info = |l: &Local| {
            mods.iter()
                .find(|m| m.file == l.file)
                .expect("мод в списке")
        };
        let kept = info(keep);
        let others = extra
            .iter()
            .map(|l| info(l).version.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let count = extra.len() + 1;
        let (level, text) = if fabric_like {
            (
                Level::Warn,
                format!(
                    "«{}» стоит {count} раза. {} загрузит подходящую копию ({}), лишние ({others}) лучше выключить.",
                    kept.name,
                    inst.loader.title(),
                    kept.version
                ),
            )
        } else {
            (
                Level::Block,
                format!(
                    "«{}» стоит {count} раза — с копиями игра не запустится. Оставьте {}, выключите {others}.",
                    kept.name, kept.version
                ),
            )
        };
        issues.push(Issue {
            level,
            text,
            file: Some(keep.file.clone()),
            action: Some(Action::DisableExtra {
                disable: extra.iter().map(|l| l.file.clone()).collect(),
            }),
        });
    }

    // Шейдеры без мода шейдеров.
    let shader_mod = enabled.iter().any(|l| {
        l.meta
            .mods
            .iter()
            .any(|m| matches!(m.id.as_str(), "iris" | "oculus" | "optifine" | "optifabric"))
            || l.file.to_lowercase().contains("optifine")
    });
    if cx.shaders && !shader_mod && inst.loader != Loader::Vanilla {
        let (project, title) = if inst.loader == Loader::Forge {
            ("oculus", "Oculus")
        } else {
            ("iris", "Iris")
        };
        issues.push(Issue {
            level: Level::Warn,
            text: format!("В сборке есть шейдеры, но нет мода {title} — без него они не работают."),
            file: None,
            action: Some(Action::Install {
                project: project.into(),
                title: title.into(),
            }),
        });
    }

    issues.sort_by_key(|i| i.level);
    Report {
        mods,
        issues,
        online: cx.online,
    }
}

/// Убрать мод из сборки. Свой файл (не из библиотеки) сначала переезжает в
/// библиотеку — его можно будет вернуть; файл с вредоносным кодом удаляется
/// совсем.
pub fn remove(data: &DataDir, build: &str, file: &str) -> Result<()> {
    let game_dir = Builds::new(data.clone()).game_dir(build)?;
    let path = content::inside(&game_dir, Folder::Mods, file)?;
    let lib = Library::new(data);
    let plain = file.trim_end_matches(".disabled");
    let stored = lib.store_path(Folder::Mods, plain);
    let from_library = std::fs::metadata(&stored).is_ok_and(|m| is_same(&m, &path));
    let infected = crate::malware::scan(&path).is_some();
    if infected && from_library {
        lib.forget(Folder::Mods, plain)?;
    }
    if from_library || infected {
        std::fs::remove_file(&path).at(&path)
    } else {
        lib.adopt(Folder::Mods, &path).map(|_| ())
    }
}

/// Выполнить простое действие (выключить, удалить, доверять, оставить новую).
/// Установка и обновление идут через каталог.
pub fn apply(data: &DataDir, build: &str, action: &Action) -> Result<()> {
    let game_dir = Builds::new(data.clone()).game_dir(build)?;
    match action {
        Action::Disable { file } => {
            content::set_enabled(&game_dir, Folder::Mods, file, false).map(|_| ())
        }
        Action::Delete { file } => remove(data, build, file),
        Action::Trust { sha1 } => trust(data, sha1),
        Action::DisableExtra { disable } => {
            for file in disable {
                content::set_enabled(&game_dir, Folder::Mods, file, false)?;
            }
            Ok(())
        }
        _ => Err(Error::Other(
            "это действие выполняется через каталог".into(),
        )),
    }
}

/// Обновить моды: новые версии из каталога, старые файлы — из сборки (свои
/// старые файлы уезжают в библиотеку). `only` — конкретные версии; пусто —
/// все обновления, кроме закреплённых модов.
pub async fn update(
    dl: &Downloader,
    data: &DataDir,
    build: &str,
    only: &[(String, String)],
) -> Result<Vec<String>> {
    let pairs: Vec<(String, String)> = if only.is_empty() {
        let r = report(dl, data, build, true).await?;
        r.mods
            .into_iter()
            .filter(|m| m.enabled && !m.pinned)
            .filter_map(|m| m.update.map(|u| (u.version_id, m.file)))
            .collect()
    } else {
        only.to_vec()
    };
    if pairs.is_empty() {
        return Ok(Vec::new());
    }
    let versions: Vec<String> = pairs.iter().map(|(v, _)| v.clone()).collect();
    let titles = catalog::install(
        dl,
        data,
        &Target::Build { id: build.into() },
        ProjectType::Mod,
        &versions,
        &|_| {},
    )
    .await?;
    // Новые файлы легли; старые (если имя другое и файл ещё на месте) убираем.
    let game_dir = Builds::new(data.clone()).game_dir(build)?;
    let lib = Library::new(data);
    let new_files: HashSet<String> = lib
        .list()?
        .into_iter()
        .filter(|i| {
            i.source
                .as_ref()
                .is_some_and(|s| versions.contains(&s.version_id))
        })
        .map(|i| i.file)
        .collect();
    for (_, old) in &pairs {
        if !new_files.contains(old.trim_end_matches(".disabled"))
            && game_dir.join(Folder::Mods.dir()).join(old).exists()
        {
            remove(data, build, old)?;
        }
    }
    Ok(titles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builds::BuildSettings;
    use crate::modmeta::{Dep, ModEntry, RangeStyle};

    fn instance(loader: Loader, version: &str) -> Instance {
        Instance {
            id: "b".into(),
            name: "b".into(),
            version: "1.21.1".into(),
            loader,
            loader_version: Some(version.into()),
            group: None,
            pinned: false,
            created: 0,
            last_played: None,
            playtime: 0,
            settings: BuildSettings::default(),
            quick: false,
            pinned_mods: Vec::new(),
        }
    }

    fn fabric_mod(file: &str, id: &str, version: &str, depends: Vec<Dep>) -> Local {
        Local {
            file: file.into(),
            enabled: true,
            size: 1,
            modified: 0,
            sha1: format!("sha-{file}"),
            meta: JarMeta {
                loaders: vec![ModLoader::Fabric],
                mods: vec![ModEntry {
                    id: id.into(),
                    name: id.into(),
                    version: version.into(),
                    depends,
                }],
                provides: Vec::new(),
            },
            malware: None,
        }
    }

    fn dep(id: &str, kind: DepKind, range: &str) -> Dep {
        Dep {
            id: id.into(),
            kind,
            ranges: if range.is_empty() {
                vec![]
            } else {
                vec![range.into()]
            },
            style: RangeStyle::Semver,
        }
    }

    fn run(inst: &Instance, local: &[Local], shaders: bool) -> Report {
        check(&Context {
            instance: inst,
            local,
            known: &HashMap::new(),
            unknown: &HashSet::new(),
            updates: &HashMap::new(),
            trusted: &HashSet::new(),
            library: &HashMap::new(),
            shaders,
            online: false,
        })
    }

    #[test]
    fn finds_problems() {
        let inst = instance(Loader::Fabric, "0.16.10");
        let mods = vec![
            fabric_mod(
                "iris.jar",
                "iris",
                "1.8",
                vec![
                    dep("fabricloader", DepKind::Required, ">=0.17"),
                    dep("minecraft", DepKind::Required, "1.21.1"),
                    dep("sodium", DepKind::Required, ""),
                ],
            ),
            fabric_mod("a1.jar", "dup", "1.0", vec![]),
            fabric_mod("a2.jar", "dup", "1.2", vec![]),
        ];
        let r = run(&inst, &mods, false);
        let texts: Vec<&str> = r.issues.iter().map(|i| i.text.as_str()).collect();
        // Forge-стиль: «[1.21,1.21.1)» на 1.21.1 — та же линейка, не ошибка.
        let iris_mc = Dep {
            id: "minecraft".into(),
            kind: DepKind::Required,
            ranges: vec!["[1.21,1.21.1)".into()],
            style: RangeStyle::Maven,
        };
        assert!(mc_ok(&iris_mc, "1.21.1"));
        assert!(!mc_ok(&iris_mc, "1.20.1"));
        assert!(
            texts.iter().any(|t| t.contains("Fabric >=0.17")),
            "{texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.contains("нужен мод «sodium»")),
            "{texts:?}"
        );
        assert!(
            !texts.iter().any(|t| t.contains("не для Minecraft")),
            "1.21.1 подходит"
        );
        let dup = r
            .issues
            .iter()
            .find(|i| i.text.contains("стоит 2 раза"))
            .unwrap();
        assert_eq!(
            dup.action,
            Some(Action::DisableExtra {
                disable: vec!["a1.jar".into()]
            })
        );
        // Fabric сам выбирает подходящую копию: копии — предупреждение.
        assert_eq!(dup.level, Level::Warn);
        assert!(
            r.issues
                .iter()
                .filter(|i| i.file.as_deref() != Some("a2.jar"))
                .all(|i| i.level == Level::Block)
        );

        // Sodium поставили — зависимость закрыта; мод Fabric в Forge-сборке — ошибка.
        let mut fixed = mods.clone();
        fixed.push(fabric_mod("sodium.jar", "sodium", "0.6", vec![]));
        // Нужна версия новее, чем стоит.
        let mut needs_new = fixed.clone();
        needs_new.push(fabric_mod(
            "addon.jar",
            "addon",
            "1",
            vec![dep("sodium", DepKind::Required, ">=0.7")],
        ));
        let r = run(&inst, &needs_new, false);
        assert!(
            r.issues
                .iter()
                .any(|i| i.text.contains("версии >=0.7, а стоит 0.6"))
        );
        let r = run(&inst, &fixed, true);
        assert!(!r.issues.iter().any(|i| i.text.contains("sodium»")));
        assert!(
            !r.issues.iter().any(|i| i.text.contains("шейдеры")),
            "Iris стоит"
        );

        let forge = instance(Loader::Forge, "47.4.10");
        let r = run(&forge, &fixed[3..], true);
        assert!(r.issues.iter().any(|i| i.text.contains("мод для Fabric")));
        assert!(r.issues.iter().any(|i| i.text.contains("нет мода Oculus")));
    }

    #[test]
    fn malware_blocks_and_vanilla_needs_loader() {
        let inst = instance(Loader::Vanilla, "");
        let mut bad = fabric_mod("bad.jar", "bad", "1", vec![]);
        bad.malware = Some("fractureiser".into());
        let r = run(&inst, &[bad], false);
        assert_eq!(r.issues.len(), 2);
        assert!(
            r.issues
                .iter()
                .any(|i| matches!(i.action, Some(Action::Delete { .. })))
        );
        assert!(
            r.issues
                .iter()
                .any(|i| matches!(i.action, Some(Action::Loader)))
        );
    }

    /// Случаи из настоящей сборки автора (Fabric 1.21.10, 17.10.2026): игра
    /// с ними запускается, лаунчер не должен ничего блокировать.
    #[test]
    fn real_fabric_build_has_no_false_alarms() {
        let inst = instance(Loader::Fabric, "0.19.5");
        let mut inst = inst;
        inst.version = "1.21.10".into();
        let breaks = |id: &str, range: &str| Dep {
            id: id.into(),
            kind: DepKind::Breaks,
            ranges: vec![range.into()],
            style: RangeStyle::Semver,
        };
        let mc = |range: &str| dep("minecraft", DepKind::Required, range);
        let mods = vec![
            fabric_mod(
                "appleskin.jar",
                "appleskin",
                "3.0.7+mc1.21.9",
                vec![breaks("roughlyenoughitems", "<6.0.306-alpha")],
            ),
            fabric_mod("rei.jar", "roughlyenoughitems", "21.9.812", vec![]),
            fabric_mod(
                "cullleaves.jar",
                "cullleaves",
                "4.1.0",
                vec![breaks("sodium", "<0.6.0")],
            ),
            fabric_mod("sodium.jar", "sodium", "0.7.3+mc1.21.10", vec![]),
            fabric_mod(
                "malilib.jar",
                "malilib",
                "0.26.6",
                vec![breaks("iris", "<1.9.4-")],
            ),
            fabric_mod(
                "iris.jar",
                "iris",
                "1.9.6+mc1.21.9",
                vec![breaks("worldeditcui", "<1.18.1")],
            ),
            fabric_mod(
                "itemscroller.jar",
                "itemscroller",
                "0.29.2",
                vec![breaks("malilib", "<0.26.3-")],
            ),
            fabric_mod(
                "wecui.jar",
                "worldeditcui",
                "1.21.10+01",
                vec![Dep {
                    id: "worldedit".into(),
                    kind: DepKind::Breaks,
                    ranges: vec!["7.3.1".into(), "7.3.16".into()],
                    style: RangeStyle::Semver,
                }],
            ),
            fabric_mod("worldedit.jar", "worldedit", "7.3.17+7262-c7fbe08", vec![]),
            fabric_mod(
                "esb-21.10.jar",
                "easyshulkerboxes",
                "21.10.1",
                vec![mc(">=1.21.10- <1.21.11-")],
            ),
            fabric_mod(
                "esb-21.11.jar",
                "easyshulkerboxes",
                "21.11.0",
                vec![
                    mc(">=1.21.11- <1.21.12-"),
                    dep("puzzleslib", DepKind::Required, ">=21.11.2"),
                ],
            ),
            fabric_mod("puzzleslib.jar", "puzzleslib", "21.10.8", vec![]),
        ];
        let r = run(&inst, &mods, false);
        let blocking: Vec<&str> = r.blocking().iter().map(|i| i.text.as_str()).collect();
        assert!(blocking.is_empty(), "{blocking:?}");
        // Лишняя копия — под 1.21.11; выключить надо её, а не подходящую.
        let dup = r
            .issues
            .iter()
            .find(|i| matches!(i.action, Some(Action::DisableExtra { .. })))
            .expect("копии найдены");
        assert_eq!(
            dup.action,
            Some(Action::DisableExtra {
                disable: vec!["esb-21.11.jar".into()]
            })
        );

        // А попавшая в диапазон версия — несовместимость.
        let mut old = mods.clone();
        old[1] = fabric_mod("rei.jar", "roughlyenoughitems", "6.0.100", vec![]);
        assert!(
            run(&inst, &old, false)
                .blocking()
                .iter()
                .any(|i| i.text.contains("roughlyenoughitems"))
        );
    }

    /// XaeroPlus требует World Map ровно 1.39.17: обновление не предлагаем.
    #[test]
    fn exact_dependency_holds_update() {
        let inst = instance(Loader::Fabric, "0.19.5");
        let wm = fabric_mod("wm.jar", "xaeroworldmap", "1.39.17", vec![]);
        let plus = fabric_mod(
            "plus.jar",
            "xaeroplus",
            "2.29.1",
            vec![dep("xaeroworldmap", DepKind::Required, "1.39.17")],
        );
        let updates = HashMap::from([(
            wm.sha1.clone(),
            Update {
                version_id: "new".into(),
                version_number: "1.47.0".into(),
            },
        )]);
        let ask = |list: &[Local]| {
            check(&Context {
                instance: &inst,
                local: list,
                known: &HashMap::new(),
                unknown: &HashSet::new(),
                updates: &updates,
                trusted: &HashSet::new(),
                library: &HashMap::new(),
                shaders: false,
                online: true,
            })
        };
        let r = ask(&[wm.clone(), plus]);
        assert!(
            r.mods.iter().all(|m| m.update.is_none()),
            "держит XaeroPlus"
        );
        let r = ask(&[wm]);
        assert!(r.mods[0].update.is_some(), "без XaeroPlus обновлять можно");
        assert!(caps(&dep("x", DepKind::Required, "<0.27")));
        assert!(!caps(&dep("x", DepKind::Required, ">=0.134.0")));
        assert!(!caps(&dep("x", DepKind::Required, "*")));
    }

    #[test]
    fn damaged_catalog_file_offers_redownload_not_trust() {
        let inst = instance(Loader::Fabric, "0.16.10");
        let sodium = fabric_mod("sodium.jar", "sodium", "0.6", vec![]);
        let library = HashMap::from([(
            "sodium.jar".to_string(),
            ("other-sha".to_string(), "v1".to_string()),
        )]);
        let r = check(&Context {
            instance: &inst,
            local: &[sodium],
            known: &HashMap::new(),
            unknown: &HashSet::from(["sha-sodium.jar".to_string()]),
            updates: &HashMap::new(),
            trusted: &HashSet::new(),
            library: &library,
            shaders: false,
            online: true,
        });
        assert!(
            r.issues
                .iter()
                .any(|i| matches!(i.action, Some(Action::Redownload { .. })))
        );
        assert!(
            !r.issues
                .iter()
                .any(|i| matches!(i.action, Some(Action::Trust { .. }))),
            "испорченному файлу из каталога доверять не предлагаем"
        );
    }
}
