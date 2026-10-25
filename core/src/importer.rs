//! Импорт сборок из других лаунчеров (раздел 5.5 ТЗ): Prism и MultiMC,
//! официальный лаунчер и TLauncher (`.minecraft`), CurseForge, Modrinth App и
//! любая папка игры. Файлы чужого лаунчера только читаются и копируются —
//! никаких ссылок и изменений в его папках.
//!
//! Версия игры и загрузчик берутся из описания сборки того лаунчера. Где
//! описания нет (Modrinth App хранит его в своей базе), лаунчер угадывает их
//! по модам: Modrinth знает, под какие версии и загрузчики каждый мод.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::builds::{Builds, Instance, LoaderChoice};
use crate::content;
use crate::download::Downloader;
use crate::error::{Error, IoContext, Result};
use crate::loaders::{self, Loader};
use crate::mods;
use crate::paths::DataDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Prism,
    Official,
    TLauncher,
    Legacy,
    CurseForge,
    Modrinth,
    Folder,
}

impl Source {
    pub fn title(self) -> &'static str {
        match self {
            Source::Prism => "Prism / MultiMC",
            Source::Official => "Официальный лаунчер",
            Source::TLauncher => "TLauncher",
            Source::Legacy => "Legacy Launcher",
            Source::CurseForge => "CurseForge",
            Source::Modrinth => "Modrinth App",
            Source::Folder => "Папка",
        }
    }
}

/// Найденная сборка другого лаунчера.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Found {
    pub source: Source,
    pub source_title: String,
    pub name: String,
    /// Папка игры: где лежат `mods`, `saves`, `config`.
    pub path: String,
    pub version: Option<String>,
    pub loader: Option<Loader>,
    pub loader_version: Option<String>,
    /// Версию и загрузчик угадали по модам — стоит проверить.
    #[serde(default)]
    pub guessed: bool,
    pub mods: usize,
    pub worlds: usize,
    pub worlds_size: u64,
    /// Почему импортировать нельзя (например, Quilt).
    pub problem: Option<String>,
}

/// Что импортировать и как.
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    pub path: String,
    pub name: String,
    pub version: String,
    pub loader: Loader,
    #[serde(default)]
    pub loader_version: Option<String>,
    /// Копировать миры.
    #[serde(default)]
    pub worlds: bool,
}

const QUILT: &str = "сборка на Quilt — его лаунчер пока не ставит (будет позже)";

/// Где обычно лежат сборки других лаунчеров.
fn locations() -> Vec<(Source, PathBuf)> {
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let mut out = Vec::new();
    if let Some(a) = &appdata {
        for dir in ["PrismLauncher", "PolyMC", "MultiMC"] {
            out.push((Source::Prism, a.join(dir).join("instances")));
        }
        out.push((Source::Modrinth, a.join("ModrinthApp").join("profiles")));
        out.push((
            Source::Modrinth,
            a.join("com.modrinth.theseus").join("profiles"),
        ));
        out.push((Source::Official, a.join(".minecraft")));
    }
    // TLauncher и Legacy Launcher могут держать игру где угодно — папку
    // берём из их настроек.
    for dir in launcher_game_dirs() {
        out.push((Source::Official, dir));
    }
    if let Some(h) = &home {
        out.push((
            Source::CurseForge,
            h.join("curseforge").join("minecraft").join("Instances"),
        ));
    }
    out
}

/// Сборки других лаунчеров на этом ПК.
pub fn scan() -> Vec<Found> {
    let mut out = Vec::new();
    let mut roots = std::collections::HashSet::new();
    for (source, place) in locations() {
        match source {
            Source::Official => {
                // Одна папка игры могла найтись дважды (настройки и APPDATA).
                let key = place.display().to_string().to_lowercase();
                if roots.insert(key.trim_end_matches(['\\', '/']).to_string()) {
                    out.extend(minecraft_root(&place));
                }
            }
            _ => {
                let Ok(entries) = std::fs::read_dir(&place) else {
                    continue;
                };
                for e in entries.flatten() {
                    if e.file_type().is_ok_and(|t| t.is_dir())
                        && let Some(f) = detect(&e.path(), Some(source))
                    {
                        out.push(f);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| {
        (&a.source_title, a.name.to_lowercase()).cmp(&(&b.source_title, b.name.to_lowercase()))
    });
    out
}

/// Папка, которую выбрал игрок: сборка любого из лаунчеров или просто папка
/// игры с `mods` и `saves`.
pub fn inspect(path: &Path) -> Result<Vec<Found>> {
    if !path.is_dir() {
        return Err(Error::Other(format!("папки {} нет", path.display())));
    }
    // Выбрали папку самого TLauncher или Legacy Launcher — игра в его настройках.
    let path = &settings_game_dir(path).unwrap_or_else(|| path.to_path_buf());
    if path.join("launcher_profiles.json").exists() || path.join("versions").is_dir() {
        let found = minecraft_root(path);
        if !found.is_empty() {
            return Ok(found);
        }
    }
    detect(path, None).map(|f| vec![f]).ok_or_else(|| {
        Error::Other(
            "в папке не нашлось сборки: нет ни папки mods, ни saves, ни описания сборки другого лаунчера"
                .into(),
        )
    })
}

/// Сборка в папке: по описанию Prism, CurseForge или Modrinth App, иначе —
/// просто папка игры.
fn detect(dir: &Path, hint: Option<Source>) -> Option<Found> {
    if dir.join("instance.cfg").exists() {
        return prism(dir);
    }
    if dir.join("minecraftinstance.json").exists() {
        return curseforge(dir);
    }
    if dir.join("profile.json").exists()
        && let Some(f) = modrinth_profile(dir)
    {
        return Some(f);
    }
    let game = dir.to_path_buf();
    if !game.join("mods").is_dir() && !game.join("saves").is_dir() {
        return None;
    }
    let source = hint.unwrap_or(Source::Folder);
    Some(fill(source, folder_name(dir), &game, None, None, None))
}

fn folder_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.display().to_string())
}

/// Заполнить счётчики модов и миров.
fn fill(
    source: Source,
    name: String,
    game: &Path,
    version: Option<String>,
    loader: Option<Loader>,
    loader_version: Option<String>,
) -> Found {
    let mods = std::fs::read_dir(game.join("mods"))
        .map(|d| {
            d.flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_lowercase();
                    n.ends_with(".jar") || n.ends_with(".jar.disabled")
                })
                .count()
        })
        .unwrap_or(0);
    let worlds = content::worlds(game).unwrap_or_default();
    Found {
        source,
        source_title: source.title().to_string(),
        name,
        path: game.display().to_string(),
        version,
        loader,
        loader_version,
        guessed: false,
        mods,
        worlds: worlds.len(),
        worlds_size: worlds.iter().map(|w| w.size).sum(),
        problem: None,
    }
}

/// Prism, PolyMC, MultiMC: `instance.cfg` и `mmc-pack.json`, игра — в
/// `.minecraft` или `minecraft`.
fn prism(dir: &Path) -> Option<Found> {
    let cfg = std::fs::read_to_string(dir.join("instance.cfg")).unwrap_or_default();
    let name = cfg
        .lines()
        .find_map(|l| l.strip_prefix("name="))
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| folder_name(dir));
    let game = [".minecraft", "minecraft"]
        .iter()
        .map(|d| dir.join(d))
        .find(|p| p.is_dir())
        .unwrap_or_else(|| dir.join(".minecraft"));

    #[derive(Deserialize)]
    struct Pack {
        #[serde(default)]
        components: Vec<Component>,
    }
    #[derive(Deserialize)]
    struct Component {
        uid: String,
        #[serde(default)]
        version: Option<String>,
    }
    let pack: Option<Pack> = std::fs::read(dir.join("mmc-pack.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let mut version = None;
    let mut loader = Some(Loader::Vanilla);
    let mut loader_version = None;
    let mut problem = None;
    for c in pack.map(|p| p.components).unwrap_or_default() {
        match c.uid.as_str() {
            "net.minecraft" => version = c.version,
            "net.fabricmc.fabric-loader" => {
                (loader, loader_version) = (Some(Loader::Fabric), c.version)
            }
            "net.minecraftforge" => (loader, loader_version) = (Some(Loader::Forge), c.version),
            "net.neoforged" => (loader, loader_version) = (Some(Loader::NeoForge), c.version),
            "org.quiltmc.quilt-loader" => problem = Some(QUILT.to_string()),
            _ => {}
        }
    }
    let mut f = fill(Source::Prism, name, &game, version, loader, loader_version);
    f.problem = problem;
    Some(f)
}

/// CurseForge: `minecraftinstance.json` в папке сборки.
fn curseforge(dir: &Path) -> Option<Found> {
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("minecraftinstance.json")).ok()?).ok()?;
    let name = v["name"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| folder_name(dir));
    let version = v["gameVersion"].as_str().map(str::to_string);
    // «forge-47.2.0», «neoforge-21.1.65», «fabric-0.15.11-1.20.1».
    let base = v["baseModLoader"]["name"].as_str().unwrap_or("");
    let (loader, loader_version, problem) = loader_from_name(base);
    let mut f = fill(
        Source::CurseForge,
        name,
        dir,
        version,
        loader,
        loader_version,
    );
    f.problem = problem;
    Some(f)
}

fn loader_from_name(name: &str) -> (Option<Loader>, Option<String>, Option<String>) {
    let lower = name.to_lowercase();
    if lower.is_empty() {
        return (Some(Loader::Vanilla), None, None);
    }
    if lower.starts_with("quilt") {
        return (None, None, Some(QUILT.to_string()));
    }
    for (prefix, loader) in [
        ("neoforge-", Loader::NeoForge),
        ("forge-", Loader::Forge),
        ("fabric-", Loader::Fabric),
    ] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            // У Fabric после версии загрузчика — версия игры: её отрезаем.
            let v = match loader {
                Loader::Fabric => rest.split('-').next().unwrap_or(rest),
                _ => rest,
            };
            return (Some(loader), Some(v.to_string()), None);
        }
    }
    (None, None, None)
}

/// Старый формат Modrinth App: `profile.json` рядом с модами.
fn modrinth_profile(dir: &Path) -> Option<Found> {
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("profile.json")).ok()?).ok()?;
    let meta = &v["metadata"];
    let name = meta["name"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| folder_name(dir));
    let version = meta["game_version"].as_str().map(str::to_string);
    let (loader, problem) = match meta["loader"].as_str().unwrap_or("vanilla") {
        "fabric" => (Some(Loader::Fabric), None),
        "forge" => (Some(Loader::Forge), None),
        "neoforge" => (Some(Loader::NeoForge), None),
        "quilt" => (None, Some(QUILT.to_string())),
        _ => (Some(Loader::Vanilla), None),
    };
    let loader_version = meta["loader_version"]["id"].as_str().map(str::to_string);
    let mut f = fill(Source::Modrinth, name, dir, version, loader, loader_version);
    f.problem = problem;
    Some(f)
}

/// Кто хозяин папки игры: TLauncher и Legacy Launcher кладут рядом свои
/// файлы профилей (Windows не различает регистр — сравниваем имена как есть).
fn root_source(root: &Path) -> Source {
    let names: Vec<String> = std::fs::read_dir(root)
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    if names.iter().any(|n| n == "tlauncher_profiles.json") {
        Source::Legacy
    } else if names
        .iter()
        .any(|n| n == "TlauncherProfiles.json" || n == "tlauncher-2.0.properties")
    {
        Source::TLauncher
    } else {
        Source::Official
    }
}

/// `.minecraft` официального лаунчера, TLauncher или Legacy Launcher:
/// профили из `launcher_profiles.json`, версия — из `versions/<id>/<id>.json`.
/// У TLauncher и Legacy профилей обычно нет — сборкой считается каждая
/// установленная версия (папка игры у них общая).
fn minecraft_root(root: &Path) -> Vec<Found> {
    let v: serde_json::Value = std::fs::read(root.join("launcher_profiles.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let source = root_source(root);
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if source != Source::Official {
        for id in installed_versions(root) {
            seen.insert((id.clone(), root.to_path_buf()));
            let (version, loader, loader_version, problem) = resolve_version(root, &id);
            let mut f = fill(source, id, root, version, loader, loader_version);
            f.problem = problem;
            vanilla_shared(&mut f);
            out.push(f);
        }
    }
    for (_, p) in v["profiles"].as_object().into_iter().flatten() {
        let Some(id) = p["lastVersionId"].as_str() else {
            continue;
        };
        // «Последний выпуск» без своей папки — это просто ванилла, не сборка.
        let game = p["gameDir"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| root.to_path_buf());
        if p["type"].as_str().is_some_and(|t| t.starts_with("latest")) && p["gameDir"].is_null() {
            continue;
        }
        if !seen.insert((id.to_string(), game.clone())) || !game.is_dir() {
            continue;
        }
        let (version, loader, loader_version, problem) = resolve_version(root, id);
        let name = p["name"]
            .as_str()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or(id)
            .to_string();
        let mut f = fill(source, name, &game, version, loader, loader_version);
        f.problem = problem;
        if game == root {
            vanilla_shared(&mut f);
        }
        out.push(f);
    }
    out
}

/// Ванильная версия в общей папке: моды там — от версии с загрузчиком, в
/// ванильную сборку их не переносим (и не показываем).
fn vanilla_shared(f: &mut Found) {
    if f.loader == Some(Loader::Vanilla) {
        f.mods = 0;
    }
}

/// Установленные версии лаунчера: папки `versions/<id>` с `<id>.json` и jar.
fn installed_versions(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(root.join("versions"))
        .map(|d| {
            d.flatten()
                .filter_map(|e| {
                    let id = e.file_name().to_string_lossy().to_string();
                    let dir = e.path();
                    (dir.join(format!("{id}.json")).exists()).then_some(id)
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Папки игры TLauncher и Legacy Launcher из их настроек: установленные
/// программы (реестр Windows) и `%APPDATA%\.tlauncher`.
fn launcher_game_dirs() -> Vec<PathBuf> {
    let mut places: Vec<PathBuf> = installed_launchers();
    if let Some(a) = std::env::var_os("APPDATA") {
        places.push(PathBuf::from(a).join(".tlauncher"));
    }
    places.iter().filter_map(|p| settings_game_dir(p)).collect()
}

/// Папка игры из настроек лаунчера в папке `dir` (`tl.properties`,
/// `tlauncher-2.0.properties`, `legacy*.properties`): `minecraft.gamedir`.
/// Нет настройки — у Legacy Launcher игра по умолчанию в `game` рядом.
fn settings_game_dir(dir: &Path) -> Option<PathBuf> {
    let files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            n.ends_with(".properties") && (n.starts_with("tl") || n.starts_with("legacy"))
        })
        .collect();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        if let Some(raw) = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("minecraft.gamedir="))
        {
            let path = PathBuf::from(unescape_properties(raw.trim()));
            if path.is_dir() {
                return Some(path);
            }
        }
    }
    let game = dir.join("game");
    (!files.is_empty() && game.join("versions").is_dir()).then_some(game)
}

/// Значение из Java `.properties`: `D\:\\Minecraft\\game` → `D:\Minecraft\game`.
fn unescape_properties(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Папки установленных TLauncher и Legacy Launcher — из записей об
/// установке программ в реестре Windows.
#[cfg(windows)]
fn installed_launchers() -> Vec<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    let mut out = Vec::new();
    let roots = [
        (
            HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];
    for (hive, path) in roots {
        let Ok(list) = RegKey::predef(hive).open_subkey_with_flags(path, KEY_READ) else {
            continue;
        };
        for name in list.enum_keys().flatten() {
            let Ok(app) = list.open_subkey_with_flags(&name, KEY_READ) else {
                continue;
            };
            let title: String = app.get_value("DisplayName").unwrap_or_default();
            let lower = title.to_lowercase();
            if !(lower.contains("legacy launcher") || lower.contains("tlauncher")) {
                continue;
            }
            let place: String = app.get_value("InstallLocation").unwrap_or_default();
            let place = place.trim().trim_matches('"');
            if !place.is_empty() {
                out.push(PathBuf::from(place));
            }
        }
    }
    out
}

#[cfg(not(windows))]
fn installed_launchers() -> Vec<PathBuf> {
    Vec::new()
}

type Resolved = (
    Option<String>,
    Option<Loader>,
    Option<String>,
    Option<String>,
);

/// Версия игры и загрузчик по id версии лаунчера: из её json (поле
/// `inheritsFrom` и библиотеки загрузчика) или по виду id.
fn resolve_version(root: &Path, id: &str) -> Resolved {
    let path = root.join("versions").join(id).join(format!("{id}.json"));
    if let Some(v) = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
    {
        // Версия игры: от какой унаследована, или чей jar (так пишут TLauncher
        // и Legacy Launcher: «Fabric 1.21.10» с `"jar": "1.21.10"`), иначе сама.
        let game = v["inheritsFrom"]
            .as_str()
            .or_else(|| v["jar"].as_str())
            .or_else(|| v["id"].as_str())
            .map(str::to_string);
        for lib in v["libraries"].as_array().into_iter().flatten() {
            let name = lib["name"].as_str().unwrap_or("");
            let mut parts = name.split(':');
            let (group, artifact, version) = (parts.next(), parts.next(), parts.next());
            match (group, artifact, version) {
                (Some("net.fabricmc"), Some("fabric-loader"), Some(v)) => {
                    return (game, Some(Loader::Fabric), Some(v.to_string()), None);
                }
                (Some("org.quiltmc"), Some("quilt-loader"), _) => {
                    return (game, None, None, Some(QUILT.to_string()));
                }
                (Some("net.neoforged"), Some("neoforge"), Some(v)) => {
                    return (game, Some(Loader::NeoForge), Some(v.to_string()), None);
                }
                (Some("net.minecraftforge"), Some("forge"), Some(v)) => {
                    // «1.20.1-47.2.0» → «47.2.0».
                    let v = v.rsplit_once('-').map_or(v, |(_, x)| x);
                    return (game, Some(Loader::Forge), Some(v.to_string()), None);
                }
                _ => {}
            }
        }
        if v["inheritsFrom"].is_null() && v["jar"].is_null() {
            return (game, Some(Loader::Vanilla), None, None);
        }
    }
    if let Some((loader, game, v)) = loaders::parse_version_id(id) {
        return (Some(game), Some(loader), Some(v), None);
    }
    if let Some(v) = id.strip_prefix("neoforge-") {
        return (
            loaders::neoforge_game(v),
            Some(Loader::NeoForge),
            Some(v.to_string()),
            None,
        );
    }
    // Похоже на обычную версию игры — ванилла; иначе версию угадаем по модам.
    let plain = id.chars().next().is_some_and(|c| c.is_ascii_digit()) && !id.contains(' ');
    if plain {
        (Some(id.to_string()), Some(Loader::Vanilla), None, None)
    } else {
        (None, None, None, None)
    }
}

/// Угадать версию игры и загрузчик по модам: Modrinth знает, под какие
/// версии и загрузчики каждый мод. Берётся версия, которую поддерживает
/// больше всего модов (при равенстве — новее), и загрузчик большинства.
pub async fn guess(dl: &Downloader, data: &DataDir, found: &mut Found) -> Result<()> {
    if found.version.is_some() && found.loader.is_some() {
        return Ok(());
    }
    let mods_dir = Path::new(&found.path).join("mods");
    let files: Vec<PathBuf> = std::fs::read_dir(&mods_dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jar")))
                .collect()
        })
        .unwrap_or_default();
    if files.is_empty() {
        if found.loader.is_none() && found.problem.is_none() {
            found.loader = Some(Loader::Vanilla);
        }
        return Ok(());
    }
    let hashes = tokio::task::spawn_blocking(move || {
        files
            .iter()
            .filter_map(|p| crate::verify::sha1_file(p).ok())
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?;
    let ident = mods::identify(dl, data, &hashes, true).await;
    let manifest = crate::install::fetch_manifest(data, dl).await.ok();
    // Порядок версий в списке Mojang: чем меньше индекс, тем новее.
    let order: HashMap<&str, usize> = manifest
        .as_ref()
        .map(|m| {
            m.versions
                .iter()
                .enumerate()
                .map(|(i, v)| (v.id.as_str(), i))
                .collect()
        })
        .unwrap_or_default();
    let mut votes: HashMap<String, usize> = HashMap::new();
    let mut loader_votes: HashMap<Loader, f64> = HashMap::new();
    for k in ident.known.values() {
        for gv in &k.game_versions {
            if order.is_empty() || order.contains_key(gv.as_str()) {
                *votes.entry(gv.clone()).or_default() += 1;
            }
        }
        let ls: Vec<Loader> = k
            .loaders
            .iter()
            .filter_map(|l| match l.as_str() {
                "fabric" => Some(Loader::Fabric),
                "forge" => Some(Loader::Forge),
                "neoforge" => Some(Loader::NeoForge),
                _ => None,
            })
            .collect();
        for l in &ls {
            *loader_votes.entry(*l).or_default() += 1.0 / ls.len() as f64;
        }
    }
    if found.version.is_none() {
        found.version = votes
            .iter()
            .max_by(|(a, x), (b, y)| {
                x.cmp(y).then_with(|| {
                    // Новее — меньший индекс в списке Mojang.
                    let ia = order.get(a.as_str()).copied().unwrap_or(usize::MAX);
                    let ib = order.get(b.as_str()).copied().unwrap_or(usize::MAX);
                    ib.cmp(&ia)
                })
            })
            .map(|(v, _)| v.clone());
        found.guessed = found.version.is_some();
    }
    if found.loader.is_none() && found.problem.is_none() {
        found.loader = loader_votes
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(l, _)| l);
        found.guessed |= found.loader.is_some();
    }
    Ok(())
}

/// Папки и файлы, которые не переносятся: журналы, кэши, служебные файлы
/// других лаунчеров. Из корня `.minecraft` — ещё и файлы самой игры.
const SKIP: &[&str] = &[
    "logs",
    "crash-reports",
    "screenshots",
    ".cache",
    "cache",
    "webcache",
    "webcache2",
    ".fabric",
    ".mixin.out",
    "downloads",
    "debug",
    "natives",
    ".majorlauncher.json",
    "profile.json",
    "minecraftinstance.json",
];
const SKIP_ROOT: &[&str] = &[
    "versions",
    "libraries",
    "assets",
    "runtime",
    "bin",
    "resources",
    "home",
    "server-resource-packs",
    "treatment_tags.json",
    // Профили TLauncher и Legacy хранят токены входа — их не переносим.
    "tlauncherprofiles.json",
    "tlauncher_profiles.json",
    "tlauncher-2.0.properties",
    "usercache.json",
    "usernamecache.json",
];

/// Создать сборку из сборки другого лаунчера: копия модов, настроек и (по
/// выбору) миров. Не вышло — новая сборка уходит в корзину.
pub async fn import(
    dl: &Downloader,
    data: &DataDir,
    req: &Request,
    stage: &(dyn Fn(&str) + Sync),
) -> Result<Instance> {
    let game = PathBuf::from(&req.path);
    if !game.is_dir() {
        return Err(Error::Other(format!("папки {} больше нет", game.display())));
    }
    // Свою же папку данных «импортировать» незачем: это уже сборки лаунчера.
    if game.starts_with(data.root()) {
        return Err(Error::Other("это папка самого MajorLauncher".into()));
    }
    let loader_version = match req.loader {
        Loader::Vanilla => None,
        _ => match req.loader_version.as_deref().map(str::trim) {
            Some(v) if !v.is_empty() => Some(v.to_string()),
            _ => {
                let list = loaders::versions(data, dl, req.loader, &req.version).await?;
                let v = list
                    .iter()
                    .find(|v| v.recommended)
                    .or_else(|| list.first())
                    .ok_or_else(|| {
                        Error::Other(format!("{} для {} нет", req.loader.title(), req.version))
                    })?;
                Some(v.version.clone())
            }
        },
    };
    let builds = Builds::new(data.clone());
    let new = builds.create(
        &req.name,
        &req.version,
        LoaderChoice {
            loader: req.loader,
            version: loader_version,
        },
        None,
    )?;
    let to = data.instance(&new.id);
    // Общая папка игры лаунчера (в ней ещё версии, библиотеки, ассеты).
    let is_root = game.join("launcher_profiles.json").exists()
        || game.join("versions").is_dir()
        || game.join("libraries").is_dir();
    let result = async {
        let entries: Vec<_> = std::fs::read_dir(&game).at(&game)?.flatten().collect();
        for e in entries {
            let name = e.file_name().to_string_lossy().to_string();
            if skipped(&name, is_root, req.worlds, req.loader) {
                continue;
            }
            stage(&format!("Копируем {name}"));
            let (src, dst) = (e.path(), to.join(&name));
            tokio::task::spawn_blocking(move || copy_tree(&src, &dst))
                .await
                .map_err(|e| Error::Other(e.to_string()))??;
        }
        // Замки открытых миров копировать незачем: игра создаст свои.
        if let Ok(worlds) = std::fs::read_dir(to.join("saves")) {
            for w in worlds.flatten() {
                let _ = std::fs::remove_file(w.path().join("session.lock"));
            }
        }
        Ok::<_, Error>(())
    }
    .await;
    match result {
        Ok(()) => builds.get(&new.id),
        Err(e) => {
            let _ = builds.trash(&new.id);
            Err(e)
        }
    }
}

/// Не переносить: журналы, кэши, служебные файлы и токены чужого лаунчера;
/// миры — если игрок не просил; моды — в ванильную сборку из общей папки.
fn skipped(name: &str, is_root: bool, worlds: bool, loader: Loader) -> bool {
    let lower = name.to_lowercase();
    SKIP.contains(&lower.as_str())
        // Служебные файлы официального лаунчера (аккаунты, настройки,
        // журналы) — все начинаются с «launcher_».
        || (is_root && (SKIP_ROOT.contains(&lower.as_str()) || lower.starts_with("launcher_")))
        || lower.ends_with(".log")
        || (lower == "saves" && !worlds)
        || (lower == "mods" && loader == Loader::Vanilla && is_root)
}

/// Копия дерева. Уже лежащий тот же файл (ресурспак из библиотеки) не трогаем.
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst).at(dst)?;
        for e in std::fs::read_dir(src).at(src)?.flatten() {
            copy_tree(&e.path(), &dst.join(e.file_name()))?;
        }
        return Ok(());
    }
    if dst.exists() {
        return Ok(());
    }
    std::fs::copy(src, dst).at(src).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_other_launchers() {
        let root = tempfile::tempdir().unwrap();

        // Prism.
        let p = root.path().join("prism");
        std::fs::create_dir_all(p.join(".minecraft/mods")).unwrap();
        std::fs::write(p.join(".minecraft/mods/sodium.jar"), b"x").unwrap();
        std::fs::write(
            p.join("instance.cfg"),
            "InstanceType=OneSix\nname=Мой Prism\n",
        )
        .unwrap();
        std::fs::write(
            p.join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.10"}]}"#,
        )
        .unwrap();
        let f = detect(&p, None).unwrap();
        assert_eq!(f.source, Source::Prism);
        assert_eq!(f.name, "Мой Prism");
        assert_eq!(f.version.as_deref(), Some("1.21.1"));
        assert_eq!(f.loader, Some(Loader::Fabric));
        assert_eq!(f.loader_version.as_deref(), Some("0.16.10"));
        assert_eq!(f.mods, 1);
        assert!(f.path.ends_with(".minecraft"));

        // CurseForge.
        let c = root.path().join("cf");
        std::fs::create_dir_all(c.join("mods")).unwrap();
        std::fs::write(
            c.join("minecraftinstance.json"),
            r#"{"name":"All the Mods","gameVersion":"1.20.1","baseModLoader":{"name":"forge-47.2.0","minecraftVersion":"1.20.1"}}"#,
        )
        .unwrap();
        let f = detect(&c, None).unwrap();
        assert_eq!(f.source, Source::CurseForge);
        assert_eq!(
            (f.loader, f.loader_version.as_deref()),
            (Some(Loader::Forge), Some("47.2.0"))
        );
        assert_eq!(
            loader_from_name("fabric-0.15.11-1.20.1"),
            (Some(Loader::Fabric), Some("0.15.11".into()), None)
        );
        assert!(loader_from_name("quilt-0.20").2.is_some());

        // Официальный лаунчер: профиль Fabric со своей папкой и «последний выпуск».
        let m = root.path().join(".minecraft");
        let fab = "fabric-loader-0.16.10-1.21.1";
        std::fs::create_dir_all(m.join("versions").join(fab)).unwrap();
        std::fs::create_dir_all(m.join("mods")).unwrap();
        std::fs::write(
            m.join("versions").join(fab).join(format!("{fab}.json")),
            r#"{"id":"fabric-loader-0.16.10-1.21.1","inheritsFrom":"1.21.1","libraries":[{"name":"net.fabricmc:fabric-loader:0.16.10"}]}"#,
        )
        .unwrap();
        std::fs::write(
            m.join("launcher_profiles.json"),
            format!(
                r#"{{"profiles":{{"a":{{"name":"Фабрик","type":"custom","lastVersionId":"{fab}"}},"b":{{"name":"","type":"latest-release","lastVersionId":"latest-release"}}}}}}"#
            ),
        )
        .unwrap();
        let list = inspect(&m).unwrap();
        assert_eq!(list.len(), 1, "«последний выпуск» без папки — не сборка");
        assert_eq!(list[0].source, Source::Official);
        assert_eq!(list[0].version.as_deref(), Some("1.21.1"));
        assert_eq!(list[0].loader, Some(Loader::Fabric));

        // Forge по id без json.
        let r = resolve_version(&m, "1.20.1-forge-47.2.0");
        assert_eq!(
            (r.0.as_deref(), r.1, r.2.as_deref()),
            (Some("1.20.1"), Some(Loader::Forge), Some("47.2.0"))
        );
        let r = resolve_version(&m, "neoforge-21.1.65");
        assert_eq!(
            (r.0.as_deref(), r.1),
            (Some("1.21.1"), Some(Loader::NeoForge))
        );
        assert_eq!(resolve_version(&m, "1.12.2").1, Some(Loader::Vanilla));
        assert_eq!(resolve_version(&m, "Мой профиль").1, None);

        // Legacy Launcher: папка игры в настройках, версии без профилей,
        // у Fabric-версии вместо inheritsFrom — поле jar.
        let ll = root.path().join("LL");
        let game = ll.join("game");
        let fab = "Fabric 1.21.10";
        std::fs::create_dir_all(game.join("versions").join(fab)).unwrap();
        std::fs::create_dir_all(game.join("versions/1.21.4")).unwrap();
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("mods/sodium.jar"), b"x").unwrap();
        std::fs::write(
            game.join("versions").join(fab).join(format!("{fab}.json")),
            r#"{"id":"Fabric 1.21.10","jar":"1.21.10","libraries":[{"name":"net.fabricmc:fabric-loader:0.19.5"}]}"#,
        )
        .unwrap();
        std::fs::write(
            game.join("versions/1.21.4/1.21.4.json"),
            r#"{"id":"1.21.4"}"#,
        )
        .unwrap();
        std::fs::write(game.join("launcher_profiles.json"), r#"{"profiles":{}}"#).unwrap();
        std::fs::write(game.join("tlauncher_profiles.json"), "{}").unwrap();
        std::fs::write(
            ll.join("tl.properties"),
            format!(
                "minecraft.gamedir={}\n",
                game.display()
                    .to_string()
                    .replace('\\', "\\\\")
                    .replace(':', "\\:")
            ),
        )
        .unwrap();
        assert_eq!(settings_game_dir(&ll).as_deref(), Some(game.as_path()));
        let list = inspect(&ll).unwrap();
        assert_eq!(list.len(), 2, "{list:?}");
        let fabric = list.iter().find(|f| f.name == fab).unwrap();
        assert_eq!(fabric.source, Source::Legacy);
        assert_eq!(fabric.version.as_deref(), Some("1.21.10"));
        assert_eq!(
            (fabric.loader, fabric.loader_version.as_deref()),
            (Some(Loader::Fabric), Some("0.19.5"))
        );
        assert_eq!(fabric.mods, 1);
        let vanilla = list.iter().find(|f| f.name == "1.21.4").unwrap();
        assert_eq!((vanilla.loader, vanilla.mods), (Some(Loader::Vanilla), 0));
        assert_eq!(
            unescape_properties(r"D\:\\Minecraft\\game"),
            r"D:\Minecraft\game"
        );

        // Просто папка: нет ни mods, ни saves — не сборка.
        assert!(inspect(root.path()).is_err());
    }

    #[test]
    fn never_copies_launcher_secrets() {
        let f = Loader::Fabric;
        for name in [
            "tlauncher_profiles.json",
            "TlauncherProfiles.json",
            "launcher_accounts.json",
            "launcher_msa_credentials.bin",
            "versions",
            "Libraries",
            "logs",
            "latest.log",
        ] {
            assert!(skipped(name, true, true, f), "{name}");
        }
        for name in [
            "mods",
            "config",
            "saves",
            "options.txt",
            "servers.dat",
            "schematics",
        ] {
            assert!(!skipped(name, true, true, f), "{name}");
        }
        assert!(skipped("saves", true, false, f), "миры — по галочке");
        assert!(
            skipped("mods", true, true, Loader::Vanilla),
            "ванилле моды из общей папки не нужны"
        );
        assert!(
            !skipped("mods", false, true, Loader::Vanilla),
            "папку сборки переносим целиком"
        );
    }

    #[test]
    fn copies_without_launcher_junk() {
        let root = tempfile::tempdir().unwrap();
        let (src, dst) = (root.path().join("a"), root.path().join("b"));
        std::fs::create_dir_all(src.join("config")).unwrap();
        std::fs::write(src.join("config/x.toml"), b"1").unwrap();
        copy_tree(&src, &dst).unwrap();
        assert!(dst.join("config/x.toml").exists());
        // Повторно — уже лежащий файл не трогается.
        copy_tree(&src, &dst).unwrap();
    }
}
