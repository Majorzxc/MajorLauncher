//! Библиотека модов (раздел 5.4 ТЗ): все моды игрока — и из библиотеки, и
//! свои файлы в сборках, — где каждый стоит, какие пустуют.
//!
//! Мод из библиотеки лежит в сборке жёсткой ссылкой: «стоит в сборке» — это
//! файл с тем же именем, размером и датой. Свои файлы (не из каталога)
//! узнаются по имени и размеру; в другую сборку они добавляются той же
//! жёсткой ссылкой, без копии.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::builds::{Builds, State};
use crate::content::{self, Folder};
use crate::error::{Error, IoContext, Result};
use crate::library::{Library, Source, is_same};
use crate::modmeta::ModLoader;
use crate::mods::Describer;
use crate::paths::DataDir;

/// Где стоит мод.
#[derive(Debug, Clone, Serialize)]
pub struct Use {
    pub build: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibMod {
    /// Как окно называет мод в действиях: `lib:<файл>` или `own:<сборка>/<файл>`.
    pub key: String,
    pub file: String,
    pub title: String,
    pub version: String,
    pub size: u64,
    pub in_library: bool,
    pub source: Option<Source>,
    pub icon_url: Option<String>,
    pub loaders: Vec<ModLoader>,
    pub builds: Vec<Use>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModLibrary {
    pub mods: Vec<LibMod>,
    /// Сколько занимают все моды (каждый файл один раз).
    pub total: u64,
    /// Сколько занимают пустующие моды библиотеки.
    pub unused: u64,
}

/// Мод для действий окна.
#[derive(Debug, Clone, Deserialize)]
pub struct Key(pub String);

enum Parsed {
    Library(String),
    Own { build: String, file: String },
}

fn parse(key: &str) -> Result<Parsed> {
    if let Some(file) = key.strip_prefix("lib:") {
        return Ok(Parsed::Library(file.to_string()));
    }
    if let Some(rest) = key.strip_prefix("own:")
        && let Some((build, file)) = rest.split_once('/')
    {
        return Ok(Parsed::Own {
            build: build.to_string(),
            file: file.to_string(),
        });
    }
    Err(Error::Other(format!("непонятный мод «{key}»")))
}

fn plain(file: &str) -> &str {
    file.trim_end_matches(".disabled")
}

/// Все моды игрока: библиотека и свои файлы в сборках.
pub fn list(data: &DataDir) -> Result<ModLibrary> {
    let lib = Library::new(data);
    let builds: Vec<_> = Builds::new(data.clone())
        .list()?
        .into_iter()
        .filter(|b| b.state == State::Active && !b.instance.quick)
        .map(|b| b.instance)
        .collect();
    let items: Vec<_> = lib
        .list()?
        .into_iter()
        .filter(|i| i.folder == Folder::Mods)
        .collect();
    let lib_meta: HashMap<String, std::fs::Metadata> = items
        .iter()
        .filter_map(|i| {
            std::fs::metadata(lib.store_path(Folder::Mods, &i.file))
                .ok()
                .map(|m| (i.file.clone(), m))
        })
        .collect();

    let mut uses: HashMap<String, Vec<Use>> = HashMap::new();
    // Свои файлы: (имя, размер) → где стоят и путь к первому.
    let mut own: Vec<(String, u64, PathBuf, Vec<Use>)> = Vec::new();
    for b in &builds {
        let Ok(dir) = Builds::new(data.clone()).game_dir(&b.id) else {
            continue;
        };
        let Ok(files) = content::files(&dir, Folder::Mods) else {
            continue;
        };
        for f in files.into_iter().filter(|f| !f.is_dir) {
            let path = dir.join(Folder::Mods.dir()).join(&f.file);
            let name = plain(&f.file).to_string();
            let here = Use {
                build: b.id.clone(),
                name: b.name.clone(),
                enabled: f.enabled,
            };
            if lib_meta.get(&name).is_some_and(|m| is_same(m, &path)) {
                uses.entry(name).or_default().push(here);
            } else if let Some(o) = own.iter_mut().find(|o| o.0 == name && o.1 == f.size) {
                o.3.push(here);
            } else {
                own.push((name, f.size, path, vec![here]));
            }
        }
    }

    let mut describer = Describer::new(data);
    let mut mods = Vec::new();
    let mut describe = |path: &Path, fallback: &str| {
        let (title, version, icon, loaders) = match describer.describe(path) {
            Some((_, meta, known)) => {
                let first = meta.mods.first();
                (
                    known
                        .as_ref()
                        .map(|k| k.title.clone())
                        .or_else(|| first.map(|m| m.name.clone())),
                    known
                        .as_ref()
                        .map(|k| k.version_number.clone())
                        .or_else(|| first.map(|m| m.version.clone())),
                    known.and_then(|k| k.icon_url),
                    meta.loaders,
                )
            }
            None => (None, None, None, Vec::new()),
        };
        (
            title.unwrap_or_else(|| fallback.trim_end_matches(".jar").to_string()),
            version.unwrap_or_default(),
            icon,
            loaders,
        )
    };
    for i in &items {
        let path = lib.store_path(Folder::Mods, &i.file);
        if !path.exists() {
            continue;
        }
        let (title, version, icon_url, loaders) = describe(&path, &i.file);
        mods.push(LibMod {
            key: format!("lib:{}", i.file),
            file: i.file.clone(),
            title: i.source.as_ref().map_or(title, |s| s.title.clone()),
            version,
            size: lib_meta.get(&i.file).map_or(i.size, |m| m.len()),
            in_library: true,
            source: i.source.clone(),
            icon_url,
            loaders,
            builds: uses.remove(&i.file).unwrap_or_default(),
        });
    }
    for (name, size, path, builds) in own {
        let (title, version, icon_url, loaders) = describe(&path, &name);
        mods.push(LibMod {
            key: format!(
                "own:{}/{}",
                builds[0].build,
                path.file_name()
                    .map_or(name.clone(), |n| n.to_string_lossy().into_owned())
            ),
            file: name,
            title,
            version,
            size,
            in_library: false,
            source: None,
            icon_url,
            loaders,
            builds,
        });
    }
    describer.save()?;
    mods.sort_by_key(|m| m.title.to_lowercase());
    let total = mods.iter().map(|m| m.size).sum();
    let unused = mods
        .iter()
        .filter(|m| m.in_library && m.builds.is_empty())
        .map(|m| m.size)
        .sum();
    Ok(ModLibrary {
        mods,
        total,
        unused,
    })
}

/// Добавить моды в сборку (жёсткой ссылкой). Возвращает, сколько добавлено;
/// уже стоящие пропускаются.
pub fn add_to_build(data: &DataDir, keys: &[String], build: &str) -> Result<usize> {
    let builds = Builds::new(data.clone());
    let target = builds.game_dir(build)?;
    let mods_dir = target.join(Folder::Mods.dir());
    std::fs::create_dir_all(&mods_dir).at(&mods_dir)?;
    let lib = Library::new(data);
    let mut added = 0;
    for key in keys {
        let from = match parse(key)? {
            Parsed::Library(file) => lib.store_path(Folder::Mods, &file),
            Parsed::Own { build, file } => {
                content::inside(&builds.game_dir(&build)?, Folder::Mods, &file)?
            }
        };
        let name = from
            .file_name()
            .map(|n| plain(&n.to_string_lossy()).to_string())
            .ok_or_else(|| Error::Other("непонятное имя файла".into()))?;
        let to = mods_dir.join(&name);
        if to.exists() || mods_dir.join(format!("{name}.disabled")).exists() {
            continue;
        }
        if std::fs::hard_link(&from, &to).is_err() {
            std::fs::copy(&from, &to).at(&to)?;
        }
        added += 1;
    }
    Ok(added)
}

/// Удалить из библиотеки моды, которые не стоят ни в одной сборке. `only` —
/// только эти (иначе все пустующие). Возвращает освобождённое место.
pub fn remove_unused(data: &DataDir, only: Option<&[String]>) -> Result<u64> {
    let all = list(data)?;
    let lib = Library::new(data);
    let mut freed = 0;
    for m in all.mods {
        if !m.in_library || !m.builds.is_empty() {
            continue;
        }
        if only.is_some_and(|keys| !keys.contains(&m.key)) {
            continue;
        }
        lib.forget(Folder::Mods, &m.file)?;
        freed += m.size;
    }
    Ok(freed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builds::LoaderChoice;
    use crate::loaders::Loader;

    #[test]
    fn finds_uses_and_unused() {
        let root = tempfile::tempdir().unwrap();
        let data = DataDir::new(root.path().join("data"));
        let builds = Builds::new(data.clone());
        let fabric = || LoaderChoice {
            loader: Loader::Fabric,
            version: Some("0.16.10".into()),
        };
        let a = builds.create("A", "1.21.1", fabric(), None).unwrap();
        let b = builds.create("B", "1.21.1", fabric(), None).unwrap();
        let lib = Library::new(&data);
        let store = |file: &str, body: &str| {
            let p = lib.store_path(Folder::Mods, file);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, body).unwrap();
            lib.register(
                Folder::Mods,
                file,
                Source {
                    project_id: "p".into(),
                    version_id: file.into(),
                    title: file.into(),
                    version_number: "1".into(),
                    sha1: String::new(),
                },
            )
            .unwrap();
        };
        store("used.jar", "used");
        store("idle.jar", "idle-file");
        let dir_a = builds.game_dir(&a.id).unwrap();
        lib.link(Folder::Mods, "used.jar", &dir_a).unwrap();
        std::fs::write(dir_a.join("mods/mine.jar"), "свой").unwrap();

        let l = list(&data).unwrap();
        let used = l.mods.iter().find(|m| m.file == "used.jar").unwrap();
        assert_eq!(used.builds.len(), 1);
        let idle = l.mods.iter().find(|m| m.file == "idle.jar").unwrap();
        assert!(idle.builds.is_empty());
        assert_eq!(l.unused, idle.size);
        let mine = l.mods.iter().find(|m| m.file == "mine.jar").unwrap();
        assert!(!mine.in_library);

        // Свой файл и мод из библиотеки — в сборку B.
        let n = add_to_build(&data, &[mine.key.clone(), idle.key.clone()], &b.id).unwrap();
        assert_eq!(n, 2);
        let again = add_to_build(&data, std::slice::from_ref(&idle.key), &b.id).unwrap();
        assert_eq!(again, 0, "уже стоит");
        let l = list(&data).unwrap();
        assert_eq!(l.unused, 0);
        let mine = l.mods.iter().find(|m| m.file == "mine.jar").unwrap();
        assert_eq!(mine.builds.len(), 2, "одна запись на обе сборки");

        // Пустующий удаляется, используемый — нет.
        std::fs::remove_file(builds.game_dir(&b.id).unwrap().join("mods/idle.jar")).unwrap();
        assert_eq!(
            remove_unused(&data, None).unwrap(),
            "idle-file".len() as u64
        );
        assert!(!lib.store_path(Folder::Mods, "idle.jar").exists());
        assert!(lib.store_path(Folder::Mods, "used.jar").exists());
        assert!(parse("../x").is_err());
    }
}
