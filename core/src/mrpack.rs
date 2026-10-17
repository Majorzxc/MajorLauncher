//! Сборки из каталога Modrinth и файлы `.mrpack` (раздел 5.5 ТЗ).
//!
//! `.mrpack` — zip с `modrinth.index.json` (версия игры, загрузчик, список
//! файлов с адресами и хешами) и папками `overrides/` и `client-overrides/`
//! (настройки модов, ресурспаки и прочее, что кладётся в сборку как есть).
//! Моды ложатся в библиотеку один раз и в сборку — жёсткой ссылкой, как при
//! установке из каталога. Качать можно только с адресов из белого списка
//! формата и только со сверкой хеша; пути из архива проверяются, чтобы файл
//! не записался мимо папки сборки.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::builds::{Builds, Instance, LoaderChoice};
use crate::content::Folder;
use crate::download::{Downloader, Task};
use crate::error::{Error, IoContext, Result, json};
use crate::install::Event;
use crate::library::{Library, Source};
use crate::loaders::Loader;
use crate::modrinth;
use crate::paths::DataDir;

/// Адреса, с которых формат разрешает качать файлы сборки.
const ALLOWED_HOSTS: &[&str] = &[
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    format_version: u32,
    game: String,
    name: String,
    #[serde(default)]
    files: Vec<PackFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackFile {
    path: String,
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<Env>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: u64,
}

#[derive(Debug, Deserialize)]
struct Env {
    #[serde(default)]
    client: String,
}

/// Что узнали из `.mrpack` до установки.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PackInfo {
    pub name: String,
    pub game: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub files: usize,
    pub size: u64,
}

fn read_index(pack: &Path) -> Result<(Index, PackInfo)> {
    let file = std::fs::File::open(pack).at(pack)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::Other(format!("{} — не архив сборки: {e}", pack.display())))?;
    let mut entry = zip.by_name("modrinth.index.json").map_err(|_| {
        Error::Other("в файле нет modrinth.index.json — это не сборка Modrinth".into())
    })?;
    let mut bytes = Vec::new();
    entry
        .by_ref()
        .take(32 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .at(pack)?;
    let index: Index = json(&bytes, "описание сборки")?;
    if index.format_version != 1 || index.game != "minecraft" {
        return Err(Error::Other(format!(
            "незнакомый формат сборки (версия {}, игра {})",
            index.format_version, index.game
        )));
    }
    let game = index
        .dependencies
        .get("minecraft")
        .cloned()
        .ok_or_else(|| Error::Other("в сборке не указана версия игры".into()))?;
    let (loader, loader_version) = if let Some(v) = index.dependencies.get("fabric-loader") {
        (Loader::Fabric, Some(v.clone()))
    } else if let Some(v) = index.dependencies.get("neoforge") {
        (Loader::NeoForge, Some(v.clone()))
    } else if let Some(v) = index.dependencies.get("forge") {
        (Loader::Forge, Some(v.clone()))
    } else if index.dependencies.contains_key("quilt-loader") {
        return Err(Error::Other(
            "сборка на Quilt — его лаунчер пока не ставит (будет позже)".into(),
        ));
    } else {
        (Loader::Vanilla, None)
    };
    let client: Vec<&PackFile> = index.files.iter().filter(|f| wanted(f)).collect();
    let info = PackInfo {
        name: index.name.clone(),
        game,
        loader,
        loader_version,
        files: client.len(),
        size: client.iter().map(|f| f.file_size).sum(),
    };
    Ok((index, info))
}

/// Нужен ли файл игре (не только серверу).
fn wanted(f: &PackFile) -> bool {
    f.env.as_ref().is_none_or(|e| e.client != "unsupported")
}

/// Путь из архива внутри папки сборки: без `..`, без корня и диска, не в
/// служебные файлы лаунчера.
fn safe_path(root: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.replace('\\', "/");
    let path = Path::new(&rel);
    let ok = !rel.is_empty()
        && path.components().all(|c| matches!(c, Component::Normal(_)))
        && !rel.starts_with(".majorlauncher")
        && !rel.contains(':');
    if !ok {
        return Err(Error::Other(format!("в сборке недопустимый путь «{rel}»")));
    }
    Ok(root.join(path))
}

fn allowed(url: &str) -> bool {
    url.strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|host| ALLOWED_HOSTS.contains(&host))
}

/// `https://cdn.modrinth.com/data/<проект>/versions/<версия>/<файл>` → проект и версия.
fn modrinth_ids(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://cdn.modrinth.com/data/")?;
    let mut parts = rest.split('/');
    let project = parts.next()?;
    (parts.next()? == "versions").then_some(())?;
    let version = parts.next()?;
    Some((project.to_string(), version.to_string()))
}

/// Что лежит в `.mrpack` (для окна подтверждения).
pub fn inspect(pack: &Path) -> Result<PackInfo> {
    read_index(pack).map(|(_, info)| info)
}

/// Поставить сборку из `.mrpack`: создать сборку, скачать файлы, разложить
/// `overrides`. `name` — своё название (иначе из файла). Не вышло — сборка
/// уходит в корзину, чтобы не оставалась недоделанной.
pub async fn install(
    dl: &Downloader,
    data: &DataDir,
    pack: &Path,
    name: Option<&str>,
    progress: &(dyn Fn(Event) + Sync),
) -> Result<Instance> {
    let (index, info) = read_index(pack)?;
    let builds = Builds::new(data.clone());
    let instance = builds.create(
        name.unwrap_or(&info.name),
        &info.game,
        LoaderChoice {
            loader: info.loader,
            version: info.loader_version.clone(),
        },
        None,
    )?;
    match fill(dl, data, pack, &index, &instance, progress).await {
        Ok(()) => Ok(instance),
        Err(e) => {
            let _ = builds.trash(&instance.id);
            Err(e)
        }
    }
}

async fn fill(
    dl: &Downloader,
    data: &DataDir,
    pack: &Path,
    index: &Index,
    instance: &Instance,
    progress: &(dyn Fn(Event) + Sync),
) -> Result<()> {
    let game_dir = Builds::new(data.clone()).game_dir(&instance.id)?;
    let lib = Library::new(data);
    let mut tasks = Vec::new();
    // Моды: (файл в библиотеке, sha1, адрес) — после загрузки записать и разложить.
    let mut mods = Vec::new();
    for f in index.files.iter().filter(|f| wanted(f)) {
        let sha1 = f
            .hashes
            .get("sha1")
            .filter(|h| h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit()))
            .ok_or_else(|| Error::Other(format!("у файла {} нет хеша", f.path)))?
            .to_lowercase();
        let url = f
            .downloads
            .iter()
            .find(|u| allowed(u))
            .ok_or_else(|| {
                Error::Other(format!(
                    "файл {} качается с адреса не из белого списка Modrinth — такую сборку лаунчер не ставит",
                    f.path
                ))
            })?
            .clone();
        let dest = safe_path(&game_dir, &f.path)?;
        let is_mod =
            f.path.starts_with("mods/") && !f.path[5..].contains('/') && f.path.ends_with(".jar");
        if is_mod {
            let file = &f.path[5..];
            let version = modrinth_ids(&url).map_or_else(|| sha1.clone(), |(_, v)| v);
            let stored = lib.store_name(Folder::Mods, file, &sha1, &version);
            let path = lib.store_path(Folder::Mods, &stored);
            let have = crate::verify::sha1_file(&path).is_ok_and(|h| h == sha1);
            if !have {
                tasks.push(Task {
                    url: url.clone(),
                    path,
                    sha1: Some(sha1.clone()),
                    size: Some(f.file_size),
                });
            }
            mods.push((stored, sha1, url));
        } else {
            tasks.push(Task {
                url,
                path: dest,
                sha1: Some(sha1),
                size: Some(f.file_size),
            });
        }
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

    for (file, sha1, url) in mods {
        let (project_id, version_id) = modrinth_ids(&url).unwrap_or_default();
        lib.register(
            Folder::Mods,
            &file,
            Source {
                project_id,
                version_id,
                title: file.trim_end_matches(".jar").to_string(),
                version_number: String::new(),
                sha1,
            },
        )?;
        lib.link(Folder::Mods, &file, &game_dir)?;
    }

    // overrides — для всех, client-overrides — поверх них для игры.
    let (pack, dir) = (pack.to_path_buf(), game_dir.clone());
    tokio::task::spawn_blocking(move || extract_overrides(&pack, &dir))
        .await
        .map_err(|e| Error::Other(format!("распаковка прервана: {e}")))??;
    Ok(())
}

fn extract_overrides(pack: &Path, game_dir: &Path) -> Result<()> {
    let file = std::fs::File::open(pack).at(pack)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| Error::Other(e.to_string()))?;
    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| Error::Other(e.to_string()))?;
            let name = entry.name().replace('\\', "/");
            let Some(rel) = name.strip_prefix(prefix) else {
                continue;
            };
            if rel.is_empty() {
                continue;
            }
            let dest = safe_path(game_dir, rel.trim_end_matches('/'))?;
            if entry.is_dir() {
                std::fs::create_dir_all(&dest).at(&dest)?;
                continue;
            }
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).at(parent)?;
            }
            let mut out = std::fs::File::create(&dest).at(&dest)?;
            std::io::copy(&mut entry, &mut out).at(&dest)?;
        }
    }
    Ok(())
}

/// Сборка из каталога: скачать `.mrpack` выбранной версии (иначе — новейшей
/// стабильной) и поставить. Обложка — иконка проекта.
pub async fn install_from_catalog(
    dl: &Downloader,
    data: &DataDir,
    project: &str,
    version: Option<&str>,
    progress: &(dyn Fn(Event) + Sync),
) -> Result<Instance> {
    let chosen = match version {
        Some(v) => modrinth::versions_by_id(dl, &[v])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| Error::Other("нет такой версии сборки".into()))?,
        None => {
            let list = modrinth::versions(dl, project, &[], None).await?;
            modrinth::best(&list)
                .cloned()
                .ok_or_else(|| Error::Other("у сборки нет версий".into()))?
        }
    };
    let file = chosen
        .file()
        .filter(|f| f.filename.ends_with(".mrpack"))
        .ok_or_else(|| Error::Other("у этой версии нет файла .mrpack".into()))?;
    if !allowed(&file.url) {
        return Err(Error::Other(format!("непонятный адрес файла {}", file.url)));
    }
    let dir = data.cache().join("modpacks");
    std::fs::create_dir_all(&dir).at(&dir)?;
    let path = dir.join(format!("{}.mrpack", chosen.id));
    progress(Event::Downloading {
        files: 1,
        bytes: file.size,
    });
    dl.download_all(
        vec![Task {
            url: file.url.clone(),
            path: path.clone(),
            sha1: Some(file.hashes.sha1.clone()),
            size: Some(file.size),
        }],
        &|t| {
            progress(Event::FileDone {
                bytes: t.size.unwrap_or(0),
            })
        },
    )
    .await?;
    let brief = modrinth::briefs(dl, &[&chosen.project_id])
        .await
        .ok()
        .and_then(|b| b.into_iter().next());
    let name = brief.as_ref().map(|b| b.title.clone());
    let instance = install(dl, data, &path, name.as_deref(), progress).await;
    let _ = std::fs::remove_file(&path);
    let instance = instance?;
    if let Some(icon) = brief.and_then(|b| b.icon_url) {
        let _ = set_icon_cover(dl, data, &instance.id, &icon).await;
    }
    Ok(instance)
}

/// Иконка проекта — обложкой сборки (256×256, PNG).
async fn set_icon_cover(dl: &Downloader, data: &DataDir, id: &str, url: &str) -> Result<()> {
    if !url.starts_with("https://") {
        return Ok(());
    }
    let bytes = dl.get_bytes(url).await?;
    let image = image::load_from_memory(&bytes).map_err(|e| Error::Other(e.to_string()))?;
    let small = image.resize_exact(256, 256, image::imageops::FilterType::Lanczos3);
    let mut png = std::io::Cursor::new(Vec::new());
    small
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| Error::Other(e.to_string()))?;
    Builds::new(data.clone()).set_cover(id, png.get_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn pack(dir: &Path, index: &str, extra: &[(&str, &str)]) -> PathBuf {
        let path = dir.join("test.mrpack");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("modrinth.index.json", opts).unwrap();
        zip.write_all(index.as_bytes()).unwrap();
        for (name, body) in extra {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn reads_index_and_guards_paths() {
        let dir = tempfile::tempdir().unwrap();
        let index = r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"Тест",
            "files":[{"path":"mods/a.jar","hashes":{"sha1":"0000000000000000000000000000000000000000"},
              "downloads":["https://cdn.modrinth.com/data/AANobbMI/versions/abc/a.jar"],"fileSize":10},
              {"path":"mods/server.jar","hashes":{"sha1":"0000000000000000000000000000000000000000"},
              "env":{"client":"unsupported","server":"required"},"downloads":[],"fileSize":5}],
            "dependencies":{"minecraft":"1.21.1","fabric-loader":"0.16.10"}}"#;
        let p = pack(dir.path(), index, &[]);
        let info = inspect(&p).unwrap();
        assert_eq!(info.name, "Тест");
        assert_eq!(info.game, "1.21.1");
        assert_eq!(info.loader, Loader::Fabric);
        assert_eq!(info.files, 1, "серверный файл не нужен");

        let root = Path::new("C:/game");
        assert!(safe_path(root, "config/sodium.json").is_ok());
        assert!(safe_path(root, "../evil.dll").is_err());
        assert!(safe_path(root, "/etc/x").is_err());
        assert!(safe_path(root, "C:/Windows/x").is_err());
        assert!(safe_path(root, "mods/../../x").is_err());
        assert!(safe_path(root, ".majorlauncher.json").is_err());
        assert!(allowed("https://cdn.modrinth.com/data/x/versions/y/z.jar"));
        assert!(!allowed("https://evil.com/cdn.modrinth.com/z.jar"));
        assert!(!allowed("http://cdn.modrinth.com/z.jar"));
        assert_eq!(
            modrinth_ids("https://cdn.modrinth.com/data/AANobbMI/versions/abc/a.jar"),
            Some(("AANobbMI".into(), "abc".into()))
        );
    }

    #[test]
    fn overrides_cannot_escape() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        let p = pack(
            dir.path(),
            "{}",
            &[
                ("overrides/config/a.txt", "a"),
                ("client-overrides/config/a.txt", "b"),
            ],
        );
        extract_overrides(&p, &game).unwrap();
        assert_eq!(
            std::fs::read_to_string(game.join("config/a.txt")).unwrap(),
            "b"
        );

        let bad = pack(dir.path(), "{}", &[("overrides/../escape.txt", "x")]);
        assert!(extract_overrides(&bad, &game).is_err());
        assert!(!dir.path().join("escape.txt").exists());

        let quilt = pack(
            dir.path(),
            r#"{"formatVersion":1,"game":"minecraft","name":"Q","files":[],"dependencies":{"minecraft":"1.21.1","quilt-loader":"0.26"}}"#,
            &[],
        );
        assert!(inspect(&quilt).is_err());
    }
}
