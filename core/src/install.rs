//! Установка версии игры: описание версии, клиент, библиотеки, ассеты, Java.
//!
//! Три режима:
//! - `Install` — сверить описания с сервером Mojang и проверить все файлы;
//! - `Launch` — быстрый путь перед запуском: локальные описания, ассеты не
//!   проверяются, если версия уже ставилась целиком (бюджет «до 1 с» из ТЗ);
//! - `Verify` — как `Install`, но хеш каждого файла пересчитывается заново.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::{StreamExt, stream};

use crate::download::{Downloader, Task};
use crate::error::{Error, IoContext, Result, json};
use crate::java;
use crate::meta::{
    AssetIndex, LIBRARIES_URL, RESOURCES_URL, VERSION_MANIFEST_URL, VersionJson, VersionManifest,
    maven_path,
};
use crate::paths::DataDir;
use crate::rules::{self, Features};
use crate::verify::{VerifyCache, sha1_file};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Install,
    Launch,
    Verify,
}

/// События для полосы прогресса.
#[derive(Debug, Clone)]
pub enum Event {
    /// Проверяем файлы на диске.
    Checking { files: usize },
    /// Начинаем качать: сколько файлов и байт.
    Downloading { files: usize, bytes: u64 },
    /// Ещё один файл скачан и проверен.
    FileDone { bytes: u64 },
    /// Распаковываем нативные библиотеки.
    Extracting,
}

pub type Progress = Arc<dyn Fn(Event) + Send + Sync>;

/// Всё, что нужно для запуска установленной версии.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub version: VersionJson,
    pub java_home: PathBuf,
    /// Библиотеки и клиент в порядке, в котором их ждёт игра.
    pub classpath: Vec<PathBuf>,
    pub natives_dir: PathBuf,
    pub asset_index: AssetIndex,
    pub log_config: Option<PathBuf>,
}

/// Нативная библиотека старого формата: jar, который надо распаковать.
struct NativeJar {
    path: PathBuf,
    sha1: String,
    exclude: Vec<String>,
}

/// Метка «версия установлена целиком» — после неё быстрый запуск не проверяет ассеты.
fn complete_marker(data: &DataDir, id: &str) -> PathBuf {
    data.version_dir(id).join(".complete")
}

pub async fn prepare(
    data: &DataDir,
    dl: &Downloader,
    version_id: &str,
    mode: Mode,
    progress: Progress,
) -> Result<Prepared> {
    let cache = Arc::new(VerifyCache::load(data.cache().join("verified.json")));
    let version = load_version(data, dl, version_id, mode).await?;
    let id = version.id.clone();
    let complete = complete_marker(data, &id);
    let check_assets = mode != Mode::Launch || !complete.exists();
    let refresh = mode != Mode::Launch;

    let mut tasks = Vec::new();

    // Клиент.
    let client = &version.downloads.client;
    let client_jar = data.client_jar(&id);
    tasks.push(Task {
        url: client.url.clone(),
        path: client_jar.clone(),
        sha1: Some(client.sha1.clone()),
        size: Some(client.size),
    });

    // Библиотеки.
    let (classpath_libs, natives, lib_tasks) = libraries(data, &version)?;
    tasks.extend(lib_tasks);
    let mut classpath = classpath_libs;
    classpath.push(client_jar);

    // Настройка логов: Mojang обновила её после Log4Shell — без неё старые версии уязвимы.
    let log_config = match version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        Some(client) => {
            let path = data.log_config(&client.file.id);
            tasks.push(Task {
                url: client.file.url.clone(),
                path: path.clone(),
                sha1: Some(client.file.sha1.clone()),
                size: Some(client.file.size),
            });
            Some(path)
        }
        None => None,
    };

    // Java.
    let component = version
        .java_version
        .as_ref()
        .map_or(java::DEFAULT_COMPONENT, |j| j.component.as_str());
    let java_files = java::files(data, dl, component, refresh).await?;
    tasks.extend(java_files.tasks);

    // Ассеты: индекс нужен всегда (по нему раскладываются ассеты старых версий),
    // а вот тысячи файлов проверяем только при установке.
    let asset_index = load_asset_index(data, dl, &version, &cache).await?;
    if check_assets {
        let mut seen = HashSet::new();
        for obj in asset_index.objects.values() {
            if seen.insert(obj.hash.clone()) {
                tasks.push(Task {
                    url: format!("{RESOURCES_URL}{}/{}", &obj.hash[..2], obj.hash),
                    path: data.asset_object(&obj.hash),
                    sha1: Some(obj.hash.clone()),
                    size: Some(obj.size),
                });
            }
        }
    }

    let missing = check(tasks, &cache, mode == Mode::Verify, &progress).await;
    if !missing.is_empty() {
        let bytes = missing.iter().filter_map(|t| t.size).sum();
        progress(Event::Downloading {
            files: missing.len(),
            bytes,
        });
        let result = dl
            .download_all(missing, &|task| {
                if let Some(sha1) = &task.sha1 {
                    cache.record(&task.path, sha1);
                }
                progress(Event::FileDone {
                    bytes: task.size.unwrap_or(0),
                });
            })
            .await;
        // Кэш сохраняем и при ошибке: уже скачанное не будем проверять заново.
        cache.save()?;
        result?;
    }

    let natives_dir = data.natives_dir(&id);
    if !natives.is_empty() && !natives_up_to_date(&natives, &natives_dir) {
        progress(Event::Extracting);
        let dir = natives_dir.clone();
        tokio::task::spawn_blocking(move || extract_natives(&natives, &dir))
            .await
            .map_err(|e| Error::Other(format!("распаковка прервана: {e}")))??;
    }

    cache.save()?;
    if check_assets {
        std::fs::write(&complete, b"").at(&complete)?;
    }

    Ok(Prepared {
        version,
        java_home: java_files.home,
        classpath,
        natives_dir,
        asset_index,
        log_config,
    })
}

/// Описание версии: локальное или с сервера Mojang (с проверкой хеша).
async fn load_version(
    data: &DataDir,
    dl: &Downloader,
    requested: &str,
    mode: Mode,
) -> Result<VersionJson> {
    let is_alias = matches!(requested, "release" | "latest" | "snapshot");
    let local = data.version_json(requested);

    if mode == Mode::Launch && !is_alias && local.exists() {
        return read_version(&local);
    }

    let manifest = match fetch_manifest(data, dl).await {
        Ok(m) => m,
        Err(e) if e.is_network() => {
            if !is_alias && local.exists() {
                tracing::warn!("нет связи с Mojang, берём сохранённое описание версии: {e}");
                return read_version(&local);
            }
            return Err(Error::NotInstalledOffline(requested.to_string()));
        }
        Err(e) => return Err(e),
    };

    let Some(entry) = manifest.find(requested) else {
        // Версии нет в списке Mojang, но она есть локально (на будущее: свои версии).
        if local.exists() {
            return read_version(&local);
        }
        return Err(Error::VersionNotFound(requested.to_string()));
    };

    let path = data.version_json(&entry.id);
    let up_to_date = sha1_file(&path).is_ok_and(|s| s.eq_ignore_ascii_case(&entry.sha1));
    if !up_to_date {
        let bytes = dl.get_verified(&entry.url, &entry.sha1).await?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
        }
        std::fs::write(&path, bytes).at(&path)?;
    }
    read_version(&path)
}

fn read_version(path: &Path) -> Result<VersionJson> {
    let bytes = std::fs::read(path).at(path)?;
    json(&bytes, &format!("описание версии {}", path.display()))
}

/// Список версий Mojang. Сохраняется в кэш, чтобы `mlcli versions` и псевдонимы
/// работали и без сети.
pub async fn fetch_manifest(data: &DataDir, dl: &Downloader) -> Result<VersionManifest> {
    let path = data.cache().join("version_manifest_v2.json");
    match dl.get_bytes(VERSION_MANIFEST_URL).await {
        Ok(bytes) => {
            let manifest = json(&bytes, "список версий Mojang")?;
            std::fs::create_dir_all(data.cache()).at(&data.cache())?;
            std::fs::write(&path, &bytes).at(&path)?;
            Ok(manifest)
        }
        Err(e) if e.is_network() && path.exists() => {
            tracing::warn!("нет связи с Mojang, берём сохранённый список версий: {e}");
            let bytes = std::fs::read(&path).at(&path)?;
            json(&bytes, "сохранённый список версий")
        }
        Err(e) => Err(e),
    }
}

async fn load_asset_index(
    data: &DataDir,
    dl: &Downloader,
    version: &VersionJson,
    cache: &VerifyCache,
) -> Result<AssetIndex> {
    let r = &version.asset_index;
    let path = data.asset_index(&r.id);
    if !cache.is_valid(&path, Some(&r.sha1), Some(r.size)) {
        let bytes = dl.get_verified(&r.url, &r.sha1).await?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
        }
        std::fs::write(&path, bytes).at(&path)?;
        cache.record(&path, &r.sha1);
    }
    let bytes = std::fs::read(&path).at(&path)?;
    json(&bytes, &format!("индекс ассетов {}", r.id))
}

/// Нативные библиотеки для чужих архитектур: на Windows x64 они не нужны,
/// а качать их — лишние мегабайты.
fn foreign_natives(name: &str) -> bool {
    name.rsplit(':').next().is_some_and(|c| {
        c.starts_with("natives-") && (c.ends_with("-arm64") || c.ends_with("-x86"))
    })
}

type Libraries = (Vec<PathBuf>, Vec<NativeJar>, Vec<Task>);

fn libraries(data: &DataDir, version: &VersionJson) -> Result<Libraries> {
    let features = Features::default();
    let mut classpath = Vec::new();
    let mut natives = Vec::new();
    let mut tasks = Vec::new();
    let mut seen = HashSet::new();

    for lib in &version.libraries {
        if !rules::allowed(&lib.rules, &features) || foreign_natives(&lib.name) {
            continue;
        }

        let artifact = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref());
        let spec = match artifact {
            Some(a) => {
                let rel = a.path.clone().or_else(|| maven_path(&lib.name));
                rel.map(|rel| (rel, a.url.clone(), a.sha1.clone(), a.size))
            }
            // Библиотеки без блока downloads (так их описывают загрузчики модов):
            // адрес собираем из Maven-координат.
            None if lib.downloads.is_none() => maven_path(&lib.name).map(|rel| {
                let base = lib.url.as_deref().unwrap_or(LIBRARIES_URL);
                let url = format!("{}/{rel}", base.trim_end_matches('/'));
                (rel, url, None, None)
            }),
            None => None,
        };
        if let Some((rel, url, sha1, size)) = spec {
            let path = data.libraries().join(&rel);
            if seen.insert(path.clone()) {
                classpath.push(path.clone());
                tasks.push(Task {
                    url,
                    path,
                    sha1,
                    size,
                });
            }
        }

        // Старый формат нативных библиотек (до 1.19): отдельный jar на каждую ОС.
        if let Some(classifier) = lib.natives.as_ref().and_then(|n| n.get("windows")) {
            let classifier = classifier.replace("${arch}", "64");
            let artifact = lib
                .downloads
                .as_ref()
                .and_then(|d| d.classifiers.as_ref())
                .and_then(|c| c.get(&classifier));
            if let Some(a) = artifact {
                let rel = a
                    .path
                    .clone()
                    .or_else(|| maven_path(&format!("{}:{classifier}", lib.name)))
                    .ok_or_else(|| {
                        Error::Other(format!("непонятный путь библиотеки {}", lib.name))
                    })?;
                let path = data.libraries().join(rel);
                let sha1 = a.sha1.clone().unwrap_or_default();
                if seen.insert(path.clone()) {
                    tasks.push(Task {
                        url: a.url.clone(),
                        path: path.clone(),
                        sha1: a.sha1.clone(),
                        size: a.size,
                    });
                }
                natives.push(NativeJar {
                    path,
                    sha1,
                    exclude: lib.extract.clone().unwrap_or_default().exclude,
                });
            }
        }
    }
    Ok((classpath, natives, tasks))
}

/// Проверить файлы параллельно, вернуть те, что надо скачать.
async fn check(
    tasks: Vec<Task>,
    cache: &Arc<VerifyCache>,
    full: bool,
    progress: &Progress,
) -> Vec<Task> {
    progress(Event::Checking { files: tasks.len() });
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let results: Vec<Option<Task>> = stream::iter(tasks)
        .map(|task| {
            let cache = Arc::clone(cache);
            tokio::task::spawn_blocking(move || {
                let ok = match (&task.sha1, full) {
                    (Some(sha1), true) => cache.is_valid_full(&task.path, sha1),
                    (sha1, _) => cache.is_valid(&task.path, sha1.as_deref(), task.size),
                };
                if ok { None } else { Some(task) }
            })
        })
        .buffer_unordered(threads * 2)
        .map(|r| r.expect("проверка файла не должна паниковать"))
        .collect()
        .await;
    results.into_iter().flatten().collect()
}

/// Метка распаковки — хеши jar-файлов: пока они те же, распаковывать заново не нужно.
fn natives_stamp(jars: &[NativeJar]) -> String {
    jars.iter()
        .map(|j| j.sha1.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn natives_up_to_date(jars: &[NativeJar], dir: &Path) -> bool {
    std::fs::read_to_string(dir.join(".extracted")).is_ok_and(|s| s == natives_stamp(jars))
}

/// Распаковать нативные библиотеки старого формата.
fn extract_natives(jars: &[NativeJar], dir: &Path) -> Result<()> {
    let stamp = natives_stamp(jars);
    let marker = dir.join(".extracted");

    if dir.exists() {
        std::fs::remove_dir_all(dir).at(dir)?;
    }
    std::fs::create_dir_all(dir).at(dir)?;

    for jar in jars {
        let file = std::fs::File::open(&jar.path).at(&jar.path)?;
        let zip_err = |source| Error::Zip {
            path: jar.path.clone(),
            source,
        };
        let mut archive = zip::ZipArchive::new(file).map_err(zip_err)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(zip_err)?;
            let name = entry.name().to_string();
            if entry.is_dir() || jar.exclude.iter().any(|ex| name.starts_with(ex)) {
                continue;
            }
            // enclosed_name защищает от путей вида «../../» внутри архива.
            let Some(rel) = entry.enclosed_name() else {
                continue;
            };
            let out = dir.join(rel);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).at(parent)?;
            }
            let mut target = std::fs::File::create(&out).at(&out)?;
            std::io::copy(&mut entry, &mut target).at(&out)?;
        }
    }
    std::fs::write(&marker, stamp).at(&marker)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_foreign_architectures() {
        assert!(foreign_natives(
            "org.lwjgl:lwjgl:3.3.3:natives-windows-arm64"
        ));
        assert!(foreign_natives("org.lwjgl:lwjgl:3.3.3:natives-windows-x86"));
        assert!(!foreign_natives("org.lwjgl:lwjgl:3.3.3:natives-windows"));
        assert!(!foreign_natives("org.lwjgl:lwjgl:3.3.3"));
    }
}
