//! Установка из каталога Modrinth (раздел 5.4 ТЗ): версия подбирается под
//! сборку, обязательные зависимости ставятся вместе с модом, заявленные
//! несовместимости показываются до установки. Файл хранится один раз в
//! библиотеке, в сборку попадает жёсткой ссылкой.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::builds::Builds;
use crate::content::Folder;
use crate::download::{Downloader, Task};
use crate::error::{Error, Result};
use crate::install::Event;
use crate::library::{Library, Source, is_same};
use crate::loaders::Loader;
use crate::modrinth::{self, DependencyType, ProjectType, SearchResult, Sort, Version};
use crate::paths::DataDir;

/// Куда ставить из каталога.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Target {
    /// В сборку: версия игры и загрузчик — её.
    Build { id: String },
    /// Сборка, которая создастся при первой установке: в ней пока ничего нет.
    New { game: String, loader: Loader },
    /// Только в библиотеку: фильтры выбраны вручную.
    Library {
        #[serde(default)]
        game: Option<String>,
        #[serde(default)]
        loader: Option<Loader>,
    },
}

/// Фильтр совместимости: версия игры и названия загрузчика у Modrinth.
struct Filter {
    game: Option<String>,
    loaders: Vec<&'static str>,
}

fn folder(kind: ProjectType) -> Result<Folder> {
    match kind {
        ProjectType::Mod => Ok(Folder::Mods),
        ProjectType::ResourcePack => Ok(Folder::ResourcePacks),
        ProjectType::Shader => Ok(Folder::ShaderPacks),
        ProjectType::Modpack => Err(Error::Other(
            "сборки из каталога появятся в части 5.3".into(),
        )),
    }
}

fn filter(data: &DataDir, target: &Target, kind: ProjectType) -> Result<Filter> {
    let (game, loader) = match target {
        Target::Build { id } => {
            let b = Builds::new(data.clone()).get(id)?;
            (Some(b.version), Some(b.loader))
        }
        Target::New { game, loader } => (Some(game.clone()), Some(*loader)),
        Target::Library { game, loader } => (game.clone(), *loader),
    };
    // Загрузчик важен только модам: ресурспаки и шейдеры от него не зависят.
    let loaders = match (kind, loader, &game) {
        (ProjectType::Mod, Some(l), Some(g)) => modrinth::loader_names(l, g),
        (ProjectType::Mod, Some(l), None) => modrinth::loader_names(l, ""),
        _ => Vec::new(),
    };
    Ok(Filter { game, loaders })
}

fn vanilla_mods(data: &DataDir, target: &Target, kind: ProjectType) -> Result<()> {
    if kind != ProjectType::Mod {
        return Ok(());
    }
    let loader = match target {
        Target::Build { id } => Builds::new(data.clone()).get(id)?.loader,
        Target::New { loader, .. } => *loader,
        Target::Library { .. } => return Ok(()),
    };
    if loader == Loader::Vanilla {
        return Err(Error::Other(
            "в ванильной сборке моды не загрузятся — выберите загрузчик в настройках сборки".into(),
        ));
    }
    Ok(())
}

/// Запрос поиска из окна.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchRequest {
    pub target: Target,
    pub kind: ProjectType,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub sort: Sort,
    #[serde(default)]
    pub offset: u32,
    /// Без фильтра совместимости («Показать всё»).
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub environment: Option<modrinth::Environment>,
    #[serde(default)]
    pub open_source: bool,
}

pub async fn search(dl: &Downloader, data: &DataDir, req: &SearchRequest) -> Result<SearchResult> {
    let f = filter(data, &req.target, req.kind)?;
    modrinth::search(
        dl,
        &modrinth::Search {
            query: &req.query,
            kind: Some(req.kind),
            game: if req.all { None } else { f.game.as_deref() },
            loaders: if req.all {
                Vec::new()
            } else {
                f.loaders.clone()
            },
            sort: req.sort,
            offset: req.offset,
            categories: &req.categories,
            environment: req.environment,
            open_source: req.open_source,
        },
    )
    .await
}

/// Категории для фильтра каталога: только этого вида проектов. Список
/// берётся у Modrinth раз в неделю (`cache/modrinth-categories.json`).
pub async fn categories(
    dl: &Downloader,
    data: &DataDir,
    kind: ProjectType,
) -> Result<Vec<modrinth::Category>> {
    let path = data.cache().join("modrinth-categories.json");
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .is_ok_and(|t| t.elapsed().is_ok_and(|e| e.as_secs() < 7 * 24 * 3600));
    let cached: Option<Vec<modrinth::Category>> = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let all = match cached {
        Some(list) if fresh => list,
        cached => match modrinth::categories(dl).await {
            Ok(list) => {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(&path, serde_json::to_vec(&list).unwrap_or_default());
                list
            }
            // Нет сети — старый список лучше пустого.
            Err(e) => cached.ok_or(e)?,
        },
    };
    let kind = kind.facet();
    Ok(all.into_iter().filter(|c| c.project_type == kind).collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionView {
    #[serde(flatten)]
    pub version: Version,
    /// Подходит к выбранной сборке.
    pub compatible: bool,
}

/// Версии проекта, новые первыми; `all` — и несовместимые (с пометкой).
pub async fn versions(
    dl: &Downloader,
    data: &DataDir,
    target: &Target,
    kind: ProjectType,
    project: &str,
    all: bool,
) -> Result<Vec<VersionView>> {
    let f = filter(data, target, kind)?;
    let list = if all {
        modrinth::versions(dl, project, &[], None).await?
    } else {
        modrinth::versions(dl, project, &f.loaders, f.game.as_deref()).await?
    };
    Ok(list
        .into_iter()
        .map(|v| {
            let game_ok = f.game.as_ref().is_none_or(|g| v.game_versions.contains(g));
            let loader_ok =
                f.loaders.is_empty() || v.loaders.iter().any(|l| f.loaders.contains(&l.as_str()));
            VersionView {
                compatible: game_ok && loader_ok,
                version: v,
            }
        })
        .collect())
}

/// Что из каталога уже стоит: проект → файл в сборке (или в библиотеке).
#[derive(Debug, Clone, Serialize)]
pub struct Installed {
    pub project_id: String,
    pub version_id: String,
    pub file: String,
    pub enabled: bool,
}

/// Проекты каталога в сборке (по файлам библиотеки, которые в ней лежат) или
/// в самой библиотеке.
pub fn installed(data: &DataDir, target: &Target) -> Result<Vec<Installed>> {
    let lib = Library::new(data);
    let items = lib.list()?;
    let mut out = Vec::new();
    let game_dir = match target {
        Target::Build { id } => Some(Builds::new(data.clone()).game_dir(id)?),
        Target::New { .. } => return Ok(out),
        Target::Library { .. } => None,
    };
    for item in items {
        let Some(source) = &item.source else {
            continue;
        };
        let installed = |file: String, enabled| Installed {
            project_id: source.project_id.clone(),
            version_id: source.version_id.clone(),
            file,
            enabled,
        };
        let Some(dir) = &game_dir else {
            out.push(installed(item.file.clone(), true));
            continue;
        };
        let Target::Build { id } = target else {
            continue;
        };
        let stored = lib.store_path(item.folder, &item.file);
        let Ok(meta) = std::fs::metadata(&stored) else {
            continue;
        };
        let here = dir.join(item.folder.dir()).join(&item.file);
        let off = dir
            .join(item.folder.dir())
            .join(format!("{}.disabled", item.file));
        if is_same(&meta, &here) {
            out.push(installed(item.file.clone(), true));
        } else if is_same(&meta, &off) {
            out.push(installed(format!("{}.disabled", item.file), false));
        } else if item.folder != Folder::Mods && (item.all_builds || item.builds.contains(id)) {
            // Ресурспак или шейдер ляжет в сборку при запуске.
            out.push(installed(item.file.clone(), true));
        }
    }
    Ok(out)
}

/// Что поставится.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Plan {
    pub items: Vec<PlanItem>,
    /// Нужные зависимости, которые уже стоят.
    pub present: Vec<String>,
    /// Нужные зависимости без версии под эту сборку — мод может не запуститься.
    pub missing: Vec<String>,
    /// Уже стоящие проекты, с которыми мод заявлен несовместимым.
    pub conflicts: Vec<String>,
    /// Необязательные дополнения (не ставятся сами).
    pub optional: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItem {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub version_number: String,
    pub size: u64,
    /// Ставится как зависимость другого.
    pub dependency: bool,
    /// Заменит уже стоящую версию этого проекта.
    pub replaces: Option<String>,
}

/// Глубже зависимостей зависимостей не идём: у модов так не бывает, а
/// зацикленный список не повесит лаунчер.
const MAX_DEPTH: usize = 4;

pub async fn plan(
    dl: &Downloader,
    data: &DataDir,
    target: &Target,
    kind: ProjectType,
    project: &str,
    version: Option<&str>,
) -> Result<Plan> {
    vanilla_mods(data, target, kind)?;
    let f = filter(data, target, kind)?;
    let installed: HashMap<String, Installed> = installed(data, target)?
        .into_iter()
        .map(|i| (i.project_id.clone(), i))
        .collect();

    let main = match version {
        Some(v) => modrinth::versions_by_id(dl, &[v]).await?.into_iter().next(),
        None => {
            let list = modrinth::versions(dl, project, &f.loaders, f.game.as_deref()).await?;
            modrinth::best(&list).cloned()
        }
    }
    .ok_or_else(|| Error::Other("у этого проекта нет версии под выбранную сборку".into()))?;

    let mut plan = Plan::default();
    let mut ids: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::from([main.project_id.clone()]);
    let mut queue: Vec<(modrinth::Dependency, usize)> =
        main.dependencies.iter().cloned().map(|d| (d, 0)).collect();
    let replaces = installed.get(&main.project_id);
    if replaces.is_some_and(|i| i.version_id == main.id) {
        // Эта версия уже стоит — ставить нечего.
        plan.present.push(main.project_id.clone());
    } else {
        plan.items
            .push(item(&main, false, replaces.map(|i| i.file.clone())));
    }
    ids.push(main.project_id.clone());

    while let Some((dep, depth)) = queue.pop() {
        let project = match (&dep.project_id, &dep.version_id) {
            (Some(p), _) => p.clone(),
            (None, Some(v)) => match modrinth::versions_by_id(dl, &[v]).await?.into_iter().next() {
                Some(found) => found.project_id,
                None => continue,
            },
            (None, None) => continue,
        };
        match dep.dependency_type {
            DependencyType::Required => {
                if !seen.insert(project.clone()) {
                    continue;
                }
                ids.push(project.clone());
                if installed.contains_key(&project) {
                    plan.present.push(project);
                    continue;
                }
                let chosen = match &dep.version_id {
                    Some(v) => modrinth::versions_by_id(dl, &[v]).await?.into_iter().next(),
                    None => {
                        let list =
                            modrinth::versions(dl, &project, &f.loaders, f.game.as_deref()).await?;
                        modrinth::best(&list).cloned()
                    }
                };
                match chosen {
                    Some(v) => {
                        if depth + 1 < MAX_DEPTH {
                            queue.extend(v.dependencies.iter().cloned().map(|d| (d, depth + 1)));
                        }
                        plan.items.push(item(&v, true, None));
                    }
                    None => plan.missing.push(project),
                }
            }
            DependencyType::Optional if depth == 0 => {
                if !installed.contains_key(&project) && seen.insert(project.clone()) {
                    ids.push(project.clone());
                    plan.optional.push(project);
                }
            }
            DependencyType::Incompatible if installed.contains_key(&project) => {
                ids.push(project.clone());
                plan.conflicts.push(project);
            }
            _ => {}
        }
    }

    // Названия и иконки — одним запросом.
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let briefs: HashMap<String, modrinth::Brief> = modrinth::briefs(dl, &refs)
        .await?
        .into_iter()
        .map(|b| (b.id.clone(), b))
        .collect();
    let title = |id: &String| {
        briefs
            .get(id)
            .map_or_else(|| id.clone(), |b| b.title.clone())
    };
    for it in &mut plan.items {
        if let Some(b) = briefs.get(&it.project_id) {
            it.title = b.title.clone();
            it.icon_url = b.icon_url.clone();
        }
    }
    for list in [
        &mut plan.present,
        &mut plan.missing,
        &mut plan.conflicts,
        &mut plan.optional,
    ] {
        *list = list.iter().map(title).collect();
    }
    Ok(plan)
}

fn item(v: &Version, dependency: bool, replaces: Option<String>) -> PlanItem {
    PlanItem {
        project_id: v.project_id.clone(),
        version_id: v.id.clone(),
        title: v.name.clone(),
        icon_url: None,
        version_number: v.version_number.clone(),
        size: v.file().map_or(0, |f| f.size),
        dependency,
        replaces,
    }
}

/// Поставить выбранные версии (их id окно берёт из плана). Возвращает названия.
pub async fn install(
    dl: &Downloader,
    data: &DataDir,
    target: &Target,
    kind: ProjectType,
    version_ids: &[String],
    progress: &(dyn Fn(Event) + Sync),
) -> Result<Vec<String>> {
    vanilla_mods(data, target, kind)?;
    let folder = folder(kind)?;
    let lib = Library::new(data);
    let refs: Vec<&str> = version_ids.iter().map(String::as_str).collect();
    let versions = modrinth::versions_by_id(dl, &refs).await?;
    let installed: HashMap<String, Installed> = installed(data, target)?
        .into_iter()
        .map(|i| (i.project_id.clone(), i))
        .collect();

    // Сначала всё скачать в библиотеку, потом раскладывать: оборвалась
    // загрузка — в сборке ничего не поменялось.
    let mut tasks = Vec::new();
    let mut placed = Vec::new();
    for v in &versions {
        let file = v
            .file()
            .ok_or_else(|| Error::Other(format!("у версии {} нет файла", v.name)))?;
        if !file.url.starts_with("https://") {
            return Err(Error::Other(format!("непонятный адрес файла {}", file.url)));
        }
        let name = lib.store_name(folder, &file.filename, &file.hashes.sha1, &v.id);
        let path = lib.store_path(folder, &name);
        let have = crate::verify::sha1_file(&path)
            .is_ok_and(|h| h.eq_ignore_ascii_case(&file.hashes.sha1));
        if !have {
            tasks.push(Task {
                url: file.url.clone(),
                path,
                sha1: Some(file.hashes.sha1.clone()),
                size: Some(file.size),
            });
        }
        placed.push((v, name, file.hashes.sha1.clone()));
    }
    if !tasks.is_empty() {
        progress(Event::Downloading {
            files: tasks.len(),
            bytes: tasks.iter().filter_map(|t| t.size).sum(),
        });
    }
    dl.download_all(tasks, &|t| {
        progress(Event::FileDone {
            bytes: t.size.unwrap_or(0),
        })
    })
    .await?;

    let briefs: HashMap<String, String> = {
        let ids: Vec<&str> = versions.iter().map(|v| v.project_id.as_str()).collect();
        modrinth::briefs(dl, &ids)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|b| (b.id, b.title))
            .collect()
    };
    let mut titles = Vec::new();
    for (v, name, sha1) in placed {
        let title = briefs
            .get(&v.project_id)
            .cloned()
            .unwrap_or_else(|| v.name.clone());
        lib.register(
            folder,
            &name,
            Source {
                project_id: v.project_id.clone(),
                version_id: v.id.clone(),
                title: title.clone(),
                version_number: v.version_number.clone(),
                sha1,
            },
        )?;
        if let Target::Build { id } = target {
            let game_dir = Builds::new(data.clone()).game_dir(id)?;
            // Старая версия этого же проекта уходит из сборки (в библиотеке остаётся).
            if let Some(old) = installed.get(&v.project_id)
                && old.file != name
            {
                let _ = std::fs::remove_file(game_dir.join(folder.dir()).join(&old.file));
            }
            lib.link(folder, &name, &game_dir)?;
            if folder != Folder::Mods {
                lib.use_in(folder, &name, id)?;
            }
        }
        titles.push(title);
    }
    Ok(titles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builds::LoaderChoice;

    #[test]
    fn filters_follow_the_build() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        let builds = Builds::new(data.clone());
        let fabric = builds
            .create(
                "Моды",
                "1.21.1",
                LoaderChoice {
                    loader: Loader::Fabric,
                    version: Some("0.19.5".into()),
                },
                None,
            )
            .unwrap();
        let vanilla = builds
            .create("Чистая", "1.21.1", LoaderChoice::default(), None)
            .unwrap();

        let f = filter(
            &data,
            &Target::Build {
                id: fabric.id.clone(),
            },
            ProjectType::Mod,
        )
        .unwrap();
        assert_eq!(
            (f.game.as_deref(), f.loaders),
            (Some("1.21.1"), vec!["fabric"])
        );
        let f = filter(
            &data,
            &Target::Build {
                id: fabric.id.clone(),
            },
            ProjectType::ResourcePack,
        )
        .unwrap();
        assert!(f.loaders.is_empty(), "ресурспакам загрузчик не важен");

        assert!(
            vanilla_mods(
                &data,
                &Target::Build {
                    id: vanilla.id.clone()
                },
                ProjectType::Mod
            )
            .is_err()
        );
        assert!(
            vanilla_mods(
                &data,
                &Target::Build { id: vanilla.id },
                ProjectType::ResourcePack
            )
            .is_ok()
        );
        assert!(folder(ProjectType::Modpack).is_err());
    }

    #[test]
    fn installed_follows_links_in_the_build() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        let builds = Builds::new(data.clone());
        let b = builds
            .create(
                "Моды",
                "1.21.1",
                LoaderChoice {
                    loader: Loader::Fabric,
                    version: Some("0.19.5".into()),
                },
                None,
            )
            .unwrap();
        let lib = Library::new(&data);
        let path = lib.store_path(Folder::Mods, "sodium.jar");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"jar").unwrap();
        let source = Source {
            project_id: "AANobbMI".into(),
            version_id: "v1".into(),
            title: "Sodium".into(),
            version_number: "0.8".into(),
            sha1: String::new(),
        };
        lib.register(Folder::Mods, "sodium.jar", source).unwrap();
        let target = Target::Build { id: b.id.clone() };
        assert!(
            installed(&data, &target).unwrap().is_empty(),
            "в сборке ещё нет"
        );

        let game = builds.game_dir(&b.id).unwrap();
        lib.link(Folder::Mods, "sodium.jar", &game).unwrap();
        let found = installed(&data, &target).unwrap();
        assert_eq!(
            (found[0].project_id.as_str(), found[0].enabled),
            ("AANobbMI", true)
        );

        // Выключенный мод тоже считается стоящим.
        crate::content::set_enabled(&game, Folder::Mods, "sodium.jar", false).unwrap();
        let found = installed(&data, &target).unwrap();
        assert_eq!(
            (found[0].file.as_str(), found[0].enabled),
            ("sodium.jar.disabled", false)
        );

        // Перед запуском библиотека моды не раскладывает — выключенный не вернётся.
        lib.link_into(&b.id, &game).unwrap();
        assert!(!game.join("mods/sodium.jar").exists());
    }
}
