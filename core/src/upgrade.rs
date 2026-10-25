//! Перевод сборки на другую версию игры (раздел 5.5 ТЗ). Старая сборка не
//! трогается: создаётся новая на выбранной версии. Моды с Modrinth ставятся в
//! версиях под новую игру (с недостающими зависимостями), моды без такой
//! версии и свои файлы переезжают выключенными — игрок может оставить
//! включёнными те, что, по его мнению, подойдут. Миры — только копией и только
//! выбранные: мир, открытый в новой версии, меняется необратимо.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::builds::{Builds, Instance, LoaderChoice, Patch};
use crate::catalog::{self, Target};
use crate::content::{self, Folder, World};
use crate::download::Downloader;
use crate::error::{Error, IoContext, Result};
use crate::install::{self, Event};
use crate::loaders::{self, Loader};
use crate::modrinth::{self, DependencyType, ProjectType, Version};
use crate::mods;
use crate::paths::DataDir;

/// Что будет с модом в новой сборке.
#[derive(Debug, Clone, Serialize)]
pub struct ModMove {
    /// Файл в старой сборке (с `.disabled`, если выключен).
    pub file: String,
    pub name: String,
    pub enabled: bool,
    /// Версия сейчас.
    pub from: String,
    /// Версия под новую игру; `None` — её нет, мод переедет выключенным.
    pub to: Option<NewVersion>,
    /// Почему версии нет.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewVersion {
    pub version_id: String,
    pub version_number: String,
    pub size: u64,
    /// Та же версия, что сейчас: она подходит и к новой игре.
    pub same: bool,
}

/// Зависимость, которой не было в сборке, а новым версиям она нужна.
#[derive(Debug, Clone, Serialize)]
pub struct Dep {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub from: String,
    pub version: String,
    pub loader: Loader,
    /// Версия загрузчика под новую игру (рекомендованная).
    pub loader_version: Option<String>,
    /// Новая версия игры старше нынешней: миры из нынешней в ней опасны.
    pub older: bool,
    pub mods: Vec<ModMove>,
    pub deps: Vec<Dep>,
    /// Нужные зависимости, которых под новую игру нет.
    pub missing_deps: Vec<String>,
    /// Сколько скачать, байт (примерно: что уже есть в библиотеке — не качается).
    pub download: u64,
    pub worlds: Vec<World>,
    /// Название новой сборки по умолчанию.
    pub name: String,
}

/// Выбор игрока в окне перевода.
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub version: String,
    pub loader_version: Option<String>,
    pub name: String,
    /// Версии модов и зависимостей Modrinth, которые ставить (id из плана).
    pub versions: Vec<String>,
    /// Файлы модов без новой версии, которые оставить включёнными.
    #[serde(default)]
    pub keep_enabled: Vec<String>,
    /// Папки миров, которые скопировать.
    #[serde(default)]
    pub worlds: Vec<String>,
}

/// Глубина зависимостей зависимостей — как у каталога.
const MAX_DEPTH: usize = 4;

/// Что получится, если перевести сборку на `version`. Ничего не меняет.
pub async fn plan(dl: &Downloader, data: &DataDir, build: &str, version: &str) -> Result<Plan> {
    let instance = Builds::new(data.clone()).get(build)?;
    if instance.quick {
        return Err(Error::Other(
            "«Быстрая игра» и так запускает любую версию — переводить её не нужно".into(),
        ));
    }
    let version = version.trim();
    if version == instance.version {
        return Err(Error::Other(format!("сборка уже на версии {version}")));
    }
    let manifest = install::fetch_manifest(data, dl).await?;
    let release = |id: &str| {
        manifest
            .versions
            .iter()
            .find(|v| v.id == id)
            .map(|v| v.release_time.clone())
    };
    let new_time = release(version).ok_or_else(|| Error::VersionNotFound(version.into()))?;
    let older = release(&instance.version).is_some_and(|old| new_time < old);

    let loader_version = loader_for(data, dl, instance.loader, version).await?;

    let report = mods::report(dl, data, build, true).await?;
    let loader_names = modrinth::loader_names(instance.loader, version);
    let known: Vec<&mods::ModInfo> = report
        .mods
        .iter()
        .filter(|m| m.modrinth.is_some())
        .collect();
    let hashes: Vec<String> = known.iter().map(|m| m.sha1.clone()).collect();
    let found = if hashes.is_empty() {
        HashMap::new()
    } else {
        modrinth::updates(dl, &hashes, &loader_names, version).await?
    };

    let mut chosen: HashMap<String, Version> = HashMap::new();
    for m in &known {
        let k = m.modrinth.as_ref().expect("отобраны опознанные");
        let Some(v) = found.get(&m.sha1) else {
            continue;
        };
        if v.project_id != k.project_id {
            continue;
        }
        // Новейшая под новую игру — бета, а стоял релиз: ищем релиз.
        let v = if v.version_type != "release" && k.version_type == "release" {
            let list = modrinth::versions(dl, &k.project_id, &loader_names, Some(version)).await?;
            modrinth::best(&list).cloned().unwrap_or_else(|| v.clone())
        } else {
            v.clone()
        };
        chosen.insert(m.sha1.clone(), v);
    }

    // Зависимости новых версий, которых нет среди модов сборки.
    let mut have: HashSet<String> = chosen.values().map(|v| v.project_id.clone()).collect();
    let mut deps: Vec<Version> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut queue: Vec<(modrinth::Dependency, usize)> = chosen
        .values()
        .flat_map(|v| v.dependencies.iter().cloned().map(|d| (d, 0)))
        .collect();
    while let Some((dep, depth)) = queue.pop() {
        if dep.dependency_type != DependencyType::Required {
            continue;
        }
        let Some(project) = dep.project_id.clone() else {
            continue;
        };
        if !have.insert(project.clone()) {
            continue;
        }
        let list = modrinth::versions(dl, &project, &loader_names, Some(version)).await?;
        match modrinth::best(&list) {
            Some(v) => {
                if depth + 1 < MAX_DEPTH {
                    queue.extend(v.dependencies.iter().cloned().map(|d| (d, depth + 1)));
                }
                deps.push(v.clone());
            }
            None => missing.push(project),
        }
    }
    let ids: Vec<&str> = deps
        .iter()
        .map(|v| v.project_id.as_str())
        .chain(missing.iter().map(String::as_str))
        .collect();
    let titles: HashMap<String, String> = modrinth::briefs(dl, &ids)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|b| (b.id, b.title))
        .collect();
    let title = |id: &str| titles.get(id).cloned().unwrap_or_else(|| id.to_string());

    let size = |v: &Version| v.file().map_or(0, |f| f.size);
    let mut download = 0;
    let mut moves: Vec<ModMove> = report
        .mods
        .iter()
        .map(|m| {
            let to = chosen.get(&m.sha1).map(|v| {
                let same = m.modrinth.as_ref().is_some_and(|k| k.version_id == v.id);
                if !same {
                    download += size(v);
                }
                NewVersion {
                    version_id: v.id.clone(),
                    version_number: v.version_number.clone(),
                    size: size(v),
                    same,
                }
            });
            let note = match (&to, &m.modrinth) {
                (Some(_), _) => None,
                (None, Some(_)) => Some(format!("на Modrinth нет версии под {version}")),
                (None, None) => Some("свой файл, не с Modrinth — версию не найти".to_string()),
            };
            ModMove {
                file: m.file.clone(),
                name: m.name.clone(),
                enabled: m.enabled,
                from: m.version.clone(),
                to,
                note,
            }
        })
        .collect();
    moves.sort_by_key(|m| m.name.to_lowercase());
    let deps: Vec<Dep> = deps
        .iter()
        .map(|v| {
            download += size(v);
            Dep {
                project_id: v.project_id.clone(),
                version_id: v.id.clone(),
                title: title(&v.project_id),
                version_number: v.version_number.clone(),
                size: size(v),
            }
        })
        .collect();

    let game_dir = data.instance(build);
    let worlds = tokio::task::spawn_blocking(move || content::worlds(&game_dir))
        .await
        .map_err(|e| Error::Other(e.to_string()))??;
    Ok(Plan {
        from: instance.version.clone(),
        version: version.to_string(),
        loader: instance.loader,
        loader_version,
        older,
        mods: moves,
        deps,
        missing_deps: missing.iter().map(|p| title(p)).collect(),
        download,
        worlds,
        name: format!(
            "{} {version}",
            strip_version(&instance.name, &instance.version)
        ),
    })
}

/// «Выживание 1.21.1» → «Выживание»: новая версия встанет вместо старой.
fn strip_version<'a>(name: &'a str, version: &str) -> &'a str {
    name.strip_suffix(version)
        .map(str::trim_end)
        .filter(|n| !n.is_empty())
        .unwrap_or(name)
}

/// Рекомендованная версия загрузчика под игру; у ванили — нет.
async fn loader_for(
    data: &DataDir,
    dl: &Downloader,
    loader: Loader,
    game: &str,
) -> Result<Option<String>> {
    if loader == Loader::Vanilla {
        return Ok(None);
    }
    let list = loaders::versions(data, dl, loader, game).await?;
    list.iter()
        .find(|v| v.recommended)
        .or_else(|| list.first())
        .map(|v| Some(v.version.clone()))
        .ok_or_else(|| {
            Error::Other(format!(
                "{} для {game} нет — перевести сборку на эту версию нельзя",
                loader.title()
            ))
        })
}

/// Создать новую сборку по выбору игрока. Что-то пошло не так — новая
/// сборка уходит в корзину, старая не тронута. `stage` — подписи шагов.
pub async fn run(
    dl: &Downloader,
    data: &DataDir,
    build: &str,
    choice: &Choice,
    progress: &(dyn Fn(Event) + Sync),
    stage: &(dyn Fn(&str) + Sync),
) -> Result<Instance> {
    let builds = Builds::new(data.clone());
    let old = builds.get(build)?;
    if old.quick {
        return Err(Error::Other("«Быструю игру» переводить не нужно".into()));
    }
    let old_dir = data.instance(build);
    let known_worlds: HashSet<String> = content::worlds(&old_dir)?
        .into_iter()
        .map(|w| w.folder)
        .collect();
    if let Some(w) = choice.worlds.iter().find(|w| !known_worlds.contains(*w)) {
        return Err(Error::Other(format!("мира «{w}» в сборке нет")));
    }
    let loader_version = match old.loader {
        Loader::Vanilla => None,
        _ => match &choice.loader_version {
            Some(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
            _ => loader_for(data, dl, old.loader, &choice.version).await?,
        },
    };
    let new = builds.create(
        &choice.name,
        &choice.version,
        LoaderChoice {
            loader: old.loader,
            version: loader_version.clone(),
        },
        old.group.as_deref(),
    )?;
    let new_dir = data.instance(&new.id);

    let result = async {
        stage("Копируем настройки сборки");
        {
            let (from, to) = (old_dir.clone(), new_dir.clone());
            tokio::task::spawn_blocking(move || copy_settings(&from, &to))
                .await
                .map_err(|e| Error::Other(e.to_string()))??;
        }
        builds.update(
            &new.id,
            Patch {
                settings: Some(old.settings.clone()),
                ..Patch::default()
            },
        )?;
        builds.set_state(
            &new.id,
            &new.version,
            new.loader,
            loader_version.clone(),
            old.pinned_mods.clone(),
        )?;

        let target = Target::Build { id: new.id.clone() };
        if !choice.versions.is_empty() {
            stage("Ставим версии модов под новую игру");
            catalog::install(
                dl,
                data,
                &target,
                ProjectType::Mod,
                &choice.versions,
                progress,
            )
            .await?;
        }

        // Моды без новой версии и свои файлы — выключенными (кроме тех, что
        // игрок оставил); выключенные в старой сборке — выключены и в новой.
        stage("Переносим остальные моды");
        let placed: HashMap<String, String> = catalog::installed(data, &target)?
            .into_iter()
            .map(|i| (i.project_id, i.file))
            .collect();
        let report = mods::report(dl, data, build, false).await?;
        let new_mods = new_dir.join(Folder::Mods.dir());
        std::fs::create_dir_all(&new_mods).at(&new_mods)?;
        for m in &report.mods {
            if let Some(file) = m.modrinth.as_ref().and_then(|k| placed.get(&k.project_id)) {
                if !m.enabled {
                    content::set_enabled(&new_dir, Folder::Mods, file, false)?;
                }
                continue;
            }
            let plain = m.file.trim_end_matches(".disabled");
            let enabled = m.enabled && choice.keep_enabled.contains(&m.file);
            let name = if enabled {
                plain.to_string()
            } else {
                format!("{plain}.disabled")
            };
            let from = old_dir.join(Folder::Mods.dir()).join(&m.file);
            let to = new_mods.join(name);
            if to.exists() {
                continue;
            }
            if std::fs::hard_link(&from, &to).is_err() {
                std::fs::copy(&from, &to).at(&from)?;
            }
        }

        for w in &choice.worlds {
            stage(&format!("Копируем мир «{w}»"));
            let (from, to, w) = (old_dir.clone(), new_dir.clone(), w.clone());
            tokio::task::spawn_blocking(move || content::copy_world(&from, &w, &to))
                .await
                .map_err(|e| Error::Other(e.to_string()))??;
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

/// Папки и файлы сборки, которые новой сборке не нужны: моды ставятся
/// заново, миры — по выбору, остальное — история старой сборки.
const SKIP: &[&str] = &[
    "mods",
    "saves",
    "logs",
    "crash-reports",
    "screenshots",
    "backups",
    "downloads",
    ".majorlauncher.json",
    ".majorlauncher.json.tmp",
    // Новая сборка уже не та версия пакета из каталога.
    ".majorlauncher-pack.json",
];

/// Настройки сборки — в новую: `config/`, `options.txt`, ресурспаки и
/// прочее. Файлы копируются (игра переписывает их на месте — общие ссылки
/// меняли бы и старую сборку), ресурспаки и шейдеры — жёсткими ссылками.
fn copy_settings(from: &Path, to: &Path) -> Result<()> {
    for entry in std::fs::read_dir(from).at(from)?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if SKIP.contains(&name.as_str()) {
            continue;
        }
        let (src, dst) = (entry.path(), to.join(&name));
        let link = name == "resourcepacks" || name == "shaderpacks";
        copy_tree(&src, &dst, link)?;
    }
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path, link: bool) -> Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst).at(dst)?;
        for entry in std::fs::read_dir(src).at(src)?.flatten() {
            copy_tree(&entry.path(), &dst.join(entry.file_name()), link)?;
        }
        return Ok(());
    }
    // Файл уже на месте: новая сборка сама получает ресурспаки «во всех
    // сборках» из библиотеки — это тот же файл, копировать его поверх себя
    // нельзя (Windows ответит «файл занят»).
    if dst.exists() {
        let meta = std::fs::metadata(src).at(src)?;
        if crate::library::is_same(&meta, dst) {
            return Ok(());
        }
        std::fs::remove_file(dst).at(dst)?;
    }
    if link && std::fs::hard_link(src, dst).is_ok() {
        return Ok(());
    }
    std::fs::copy(src, dst).at(src).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_settings() {
        assert_eq!(strip_version("Выживание 1.21.1", "1.21.1"), "Выживание");
        assert_eq!(strip_version("1.21.1", "1.21.1"), "1.21.1");
        assert_eq!(strip_version("Техно", "1.20.1"), "Техно");

        let root = tempfile::tempdir().unwrap();
        let (from, to) = (root.path().join("a"), root.path().join("b"));
        for dir in ["mods", "saves/w", "config/sodium", "resourcepacks", "logs"] {
            std::fs::create_dir_all(from.join(dir)).unwrap();
        }
        std::fs::write(from.join("mods/m.jar"), b"m").unwrap();
        std::fs::write(from.join("saves/w/level.dat"), b"w").unwrap();
        std::fs::write(from.join("config/sodium/o.json"), b"{}").unwrap();
        std::fs::write(from.join("resourcepacks/p.zip"), b"p").unwrap();
        std::fs::write(from.join("options.txt"), b"fov:70").unwrap();
        std::fs::write(from.join(".majorlauncher.json"), b"{}").unwrap();
        std::fs::create_dir_all(&to).unwrap();
        copy_settings(&from, &to).unwrap();
        assert!(to.join("config/sodium/o.json").exists());
        assert!(to.join("resourcepacks/p.zip").exists());
        assert!(to.join("options.txt").exists());
        assert!(!to.join("mods").exists(), "моды ставятся заново");
        assert!(!to.join("saves").exists(), "миры — только выбранные");
        assert!(!to.join("logs").exists());
        assert!(!to.join(".majorlauncher.json").exists());
        // Уже лежащий тот же файл (ресурспак из библиотеки) — не мешает.
        std::fs::remove_dir_all(&to).unwrap();
        std::fs::create_dir_all(to.join("resourcepacks")).unwrap();
        std::fs::hard_link(
            from.join("resourcepacks/p.zip"),
            to.join("resourcepacks/p.zip"),
        )
        .unwrap();
        copy_settings(&from, &to).unwrap();
        assert!(to.join("options.txt").exists());
        // Настройки — копией: игра в новой сборке не меняет старую.
        std::fs::write(to.join("options.txt"), b"fov:90").unwrap();
        assert_eq!(std::fs::read(from.join("options.txt")).unwrap(), b"fov:70");
    }
}
