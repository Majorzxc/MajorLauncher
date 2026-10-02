//! Java от Mojang. Каждая версия игры сама называет нужный «компонент»
//! (`jre-legacy` — Java 8, `java-runtime-delta` — Java 21 и т. д.), Mojang
//! раздаёт его пофайлово с хешем у каждого файла. Ставим в `java/<компонент>`,
//! системную Java пользователя не трогаем и не используем.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::download::{Downloader, Task};
use crate::error::{Error, IoContext, Result, json};
use crate::meta::Download;
use crate::paths::DataDir;
use crate::verify::sha1_file;

pub const JAVA_RUNTIMES_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const PLATFORM: &str = "windows-x64";
/// Для версий, в описании которых Java не указана (очень старые), — Java 8.
pub const DEFAULT_COMPONENT: &str = "jre-legacy";

#[derive(Deserialize)]
struct RuntimeEntry {
    manifest: Download,
}

type AllRuntimes = HashMap<String, HashMap<String, Vec<RuntimeEntry>>>;

#[derive(Deserialize)]
struct RuntimeManifest {
    files: BTreeMap<String, RuntimeFile>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RuntimeFile {
    File { downloads: RuntimeDownloads },
    Directory,
    // Ссылки встречаются только в сборках для Linux и macOS.
    Link,
}

#[derive(Deserialize)]
struct RuntimeDownloads {
    raw: Download,
}

/// Файлы Java, которые должны лежать на диске, и папка этой Java.
pub(crate) struct JavaFiles {
    pub home: PathBuf,
    pub tasks: Vec<Task>,
}

/// Собрать список файлов Java. `refresh` — сверить описание с сервером Mojang,
/// иначе взять сохранённое локально (быстро и без интернета).
pub(crate) async fn files(
    data: &DataDir,
    dl: &Downloader,
    component: &str,
    refresh: bool,
) -> Result<JavaFiles> {
    let manifest_path = data.java_manifest(component);
    let have_local = manifest_path.exists();

    if refresh || !have_local {
        match fetch_manifest_ref(dl, component).await {
            Ok(remote) => {
                let up_to_date = have_local
                    && sha1_file(&manifest_path)
                        .is_ok_and(|s| s.eq_ignore_ascii_case(&remote.sha1));
                if !up_to_date {
                    let bytes = dl.get_verified(&remote.url, &remote.sha1).await?;
                    if let Some(dir) = manifest_path.parent() {
                        std::fs::create_dir_all(dir).at(dir)?;
                    }
                    std::fs::write(&manifest_path, bytes).at(&manifest_path)?;
                }
            }
            // Нет сети, но Java уже ставилась — работаем с тем, что есть.
            Err(e) if e.is_network() && have_local => {
                tracing::warn!("не удалось обновить описание Java {component}: {e}");
            }
            Err(e) => return Err(e),
        }
    }

    let bytes = std::fs::read(&manifest_path).at(&manifest_path)?;
    let manifest: RuntimeManifest = json(&bytes, &format!("описание Java {component}"))?;

    let home = data.java_dir(component);
    let mut tasks = Vec::new();
    for (rel, file) in manifest.files {
        let path = home.join(&rel);
        match file {
            RuntimeFile::Directory => std::fs::create_dir_all(&path).at(&path)?,
            RuntimeFile::File { downloads } => tasks.push(Task {
                url: downloads.raw.url,
                path,
                sha1: Some(downloads.raw.sha1),
                size: Some(downloads.raw.size),
            }),
            RuntimeFile::Link => {}
        }
    }
    Ok(JavaFiles { home, tasks })
}

async fn fetch_manifest_ref(dl: &Downloader, component: &str) -> Result<Download> {
    let bytes = dl.get_bytes(JAVA_RUNTIMES_URL).await?;
    let mut all: AllRuntimes = json(&bytes, "список Java от Mojang")?;
    all.get_mut(PLATFORM)
        .and_then(|components| components.remove(component))
        .and_then(|entries| entries.into_iter().next())
        .map(|entry| entry.manifest)
        .ok_or_else(|| Error::JavaUnavailable(component.to_string()))
}

/// Путь к `java.exe` (с консолью) или `javaw.exe` (без окна консоли).
pub fn executable(home: &std::path::Path, console: bool) -> PathBuf {
    home.join("bin")
        .join(if console { "java.exe" } else { "javaw.exe" })
}
