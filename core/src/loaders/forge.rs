//! Forge и NeoForge ставятся из своего установщика (jar из их Maven). Окно
//! установщика лаунчер не показывает — делает то же самое сам. Форматов два:
//!
//! - старый (Forge 1.6–1.12.2 до конца 2021): `install_profile.json` с блоками
//!   `install` и `versionInfo`, jar Forge лежит внутри установщика, у библиотек
//!   нет хешей, а часть из них давно переехала в Maven Central;
//! - новый (Forge 1.12.2-14.23.5.2860+, 1.13+, весь NeoForge): описание версии
//!   отдельным файлом, библиотеки с хешами и «процессоры» — небольшие
//!   Java-программы, которые патчат клиент игры. Запускаются Java той же
//!   версии игры, без консольного окна.

use crate::cancel::Cancel;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use futures_util::future::join_all;
use serde::Deserialize;
use serde_json::Value;

use super::{Loader, LoaderVersion, cached_get, compare_versions, game_at_least, save_version};
use crate::download::{Downloader, Task};
use crate::error::{Error, IoContext, Result, json};
use crate::install::{self, Event, Mode, Progress};
use crate::meta::{self, LIBRARIES_URL, maven_path};
use crate::paths::DataDir;
use crate::verify::sha1_file;

const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/";
const NEO_MAVEN: &str = "https://maven.neoforged.net/releases/";
const MAVEN_CENTRAL: &str = "https://repo1.maven.org/maven2/";
const FORGE_PROMOS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEO_API: &str = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/";
/// NeoForge для 1.20.1 ещё назывался `net.neoforged:forge`.
const NEO_LEGACY_GAME: &str = "1.20.1";

/// Сборка Forge из списка Maven: `1.7.10-10.13.4.1614-1.7.10` →
/// игра `1.7.10`, версия `10.13.4.1614`.
#[derive(Debug, Clone, PartialEq)]
struct Build {
    game: String,
    version: String,
    maven: String,
}

fn parse_forge_meta(xml: &str) -> Vec<Build> {
    xml.split("<version>")
        .skip(1)
        .filter_map(|s| s.split("</version>").next())
        .filter_map(|maven| {
            let (game, rest) = maven.split_once('-')?;
            let version = rest.strip_suffix(&format!("-{game}")).unwrap_or(rest);
            Some(Build {
                game: game.to_string(),
                version: version.to_string(),
                maven: maven.to_string(),
            })
        })
        .collect()
}

/// Версия игры по версии NeoForge: `21.1.77` → `1.21.1`, `21.0.5` → `1.21`,
/// `26.1.2.115` → `26.1.2`, `26.1.0.19-beta` → `26.1`.
pub(super) fn neo_game(version: &str) -> Option<String> {
    if version.starts_with("0.") || version.contains('+') {
        return None;
    }
    let nums: Vec<u64> = version
        .split(['.', '-'])
        .map_while(|p| p.parse().ok())
        .collect();
    match nums.as_slice() {
        [a, b, c, _, ..] if *a >= 25 => Some(if *c == 0 {
            format!("{a}.{b}")
        } else {
            format!("{a}.{b}.{c}")
        }),
        [a, b, _, ..] if *a < 25 => Some(if *b == 0 {
            format!("1.{a}")
        } else {
            format!("1.{a}.{b}")
        }),
        _ => None,
    }
}

#[derive(Deserialize)]
struct NeoList {
    versions: Vec<String>,
}

async fn forge_builds(data: &DataDir, dl: &Downloader) -> Result<Vec<Build>> {
    let url = format!("{FORGE_MAVEN}net/minecraftforge/forge/maven-metadata.xml");
    let bytes = cached_get(data, dl, &url, "forge-maven-metadata.xml").await?;
    Ok(parse_forge_meta(&String::from_utf8_lossy(&bytes)))
}

/// Все сборки NeoForge: `(игра, версия)`.
async fn neo_builds(data: &DataDir, dl: &Downloader) -> Result<Vec<(String, String)>> {
    let bytes = cached_get(data, dl, &format!("{NEO_API}neoforge"), "neoforge.json").await?;
    let list: NeoList = json(&bytes, "список версий NeoForge")?;
    let mut out: Vec<(String, String)> = list
        .versions
        .into_iter()
        .filter_map(|v| neo_game(&v).map(|g| (g, v)))
        .collect();
    let bytes = cached_get(data, dl, &format!("{NEO_API}forge"), "neoforge-1.20.1.json").await?;
    let legacy: NeoList = json(&bytes, "список версий NeoForge")?;
    out.extend(legacy.versions.into_iter().filter_map(|v| {
        v.strip_prefix(&format!("{NEO_LEGACY_GAME}-"))
            .map(|v| (NEO_LEGACY_GAME.to_string(), v.to_string()))
    }));
    Ok(out)
}

pub(super) async fn games(data: &DataDir, dl: &Downloader, loader: Loader) -> Result<Vec<String>> {
    let mut games: Vec<String> = match loader {
        Loader::Forge => forge_builds(data, dl)
            .await?
            .into_iter()
            .map(|b| b.game)
            // Установщики Forge есть начиная с 1.6.
            .filter(|g| game_at_least(g, &[1, 6]))
            .collect(),
        _ => neo_builds(data, dl)
            .await?
            .into_iter()
            .map(|(g, _)| g)
            .collect(),
    };
    games.sort();
    games.dedup();
    Ok(games)
}

pub(super) async fn versions(
    data: &DataDir,
    dl: &Downloader,
    loader: Loader,
    game: &str,
) -> Result<Vec<LoaderVersion>> {
    let mut list = match loader {
        Loader::Forge => {
            let builds = forge_builds(data, dl).await?;
            // Рекомендованная сборка — из promotions Forge; без сети — просто без неё.
            let recommended = cached_get(data, dl, FORGE_PROMOS, "forge-promotions.json")
                .await
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .and_then(|p| {
                    p["promos"][format!("{game}-recommended")]
                        .as_str()
                        .map(str::to_string)
                });
            builds
                .into_iter()
                .filter(|b| b.game == game)
                .map(|b| LoaderVersion {
                    recommended: recommended.as_deref() == Some(b.version.as_str()),
                    stable: true,
                    version: b.version,
                })
                .collect::<Vec<_>>()
        }
        _ => neo_builds(data, dl)
            .await?
            .into_iter()
            .filter(|(g, _)| g == game)
            .map(|(_, v)| LoaderVersion {
                stable: !v.contains("beta") && !v.contains("alpha"),
                recommended: false,
                version: v,
            })
            .collect(),
    };
    list.sort_by(|a, b| compare_versions(&b.version, &a.version));
    list.dedup_by(|a, b| a.version == b.version);
    Ok(list)
}

/// Адрес установщика.
async fn installer_url(
    data: &DataDir,
    dl: &Downloader,
    loader: Loader,
    game: &str,
    version: &str,
) -> Result<String> {
    Ok(match loader {
        Loader::Forge => {
            let builds = forge_builds(data, dl).await?;
            let maven = builds
                .into_iter()
                .find(|b| b.game == game && b.version == version)
                .map(|b| b.maven)
                .ok_or_else(|| Error::Other(format!("Forge {version} для {game} не найден")))?;
            format!("{FORGE_MAVEN}net/minecraftforge/forge/{maven}/forge-{maven}-installer.jar")
        }
        _ if game == NEO_LEGACY_GAME => {
            let maven = format!("{game}-{version}");
            format!("{NEO_MAVEN}net/neoforged/forge/{maven}/forge-{maven}-installer.jar")
        }
        _ => {
            format!("{NEO_MAVEN}net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar")
        }
    })
}

#[derive(Deserialize)]
struct LegacyInstall {
    /// Maven-координаты jar Forge.
    path: String,
    /// Имя этого jar внутри установщика.
    #[serde(rename = "filePath")]
    file_path: String,
}

#[derive(Deserialize)]
struct InstallProfile {
    #[serde(default)]
    data: HashMap<String, Sides>,
    #[serde(default)]
    processors: Vec<Processor>,
    #[serde(default)]
    libraries: Vec<meta::Library>,
    /// Где в установщике описание версии.
    json: String,
}

#[derive(Deserialize)]
struct Sides {
    client: String,
}

#[derive(Deserialize)]
struct Processor {
    sides: Option<Vec<String>>,
    jar: String,
    #[serde(default)]
    classpath: Vec<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    outputs: HashMap<String, String>,
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(path).at(path)?;
    zip::ZipArchive::new(file).map_err(|source| Error::Zip {
        path: path.to_path_buf(),
        source,
    })
}

fn read_entry(
    zip: &mut zip::ZipArchive<std::fs::File>,
    installer: &Path,
    name: &str,
) -> Result<Vec<u8>> {
    let zip_err = |source| Error::Zip {
        path: installer.to_path_buf(),
        source,
    };
    let mut entry = zip.by_name(name.trim_start_matches('/')).map_err(zip_err)?;
    let mut out = Vec::new();
    entry.read_to_end(&mut out).at(installer)?;
    Ok(out)
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).at(dir)?;
    }
    std::fs::write(path, bytes).at(path)
}

#[allow(clippy::too_many_arguments)] // все — разные входные данные установки
pub(super) async fn install(
    data: &DataDir,
    dl: &Downloader,
    loader: Loader,
    game: &str,
    version: &str,
    id: &str,
    progress: Progress,
    cancel: &Cancel,
) -> Result<Vec<PathBuf>> {
    let url = installer_url(data, dl, loader, game, version).await?;
    let name = url.rsplit('/').next().unwrap_or("installer.jar");
    let installer = data.cache().join("installers").join(name);
    // Хеш установщика лежит рядом в том же Maven — по нему можно качать и с зеркала.
    let sha1 = dl
        .get_bytes(&format!("{url}.sha1"))
        .await
        .ok()
        .map(|b| String::from_utf8_lossy(&b).trim().to_lowercase())
        .filter(|s| s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit()));
    let have = sha1
        .as_ref()
        .is_some_and(|s| sha1_file(&installer).is_ok_and(|h| h == *s));
    if !have {
        dl.download(&Task {
            url: url.clone(),
            path: installer.clone(),
            sha1,
            size: None,
        })
        .await?;
    }

    // Ваниль под загрузчиком: её клиент патчат процессоры, её Java их запускает.
    let vanilla = install::prepare(data, dl, game, Mode::Install, progress.clone()).await?;

    let mut zip = open_zip(&installer)?;
    let profile: Value = json(
        &read_entry(&mut zip, &installer, "install_profile.json")?,
        "описание установщика",
    )?;
    if let Some(info) = profile.get("versionInfo") {
        let install: LegacyInstall =
            serde_json::from_value(profile["install"].clone()).map_err(|source| Error::Json {
                what: "описание установщика".into(),
                source,
            })?;
        let forge_rel = maven_path(&install.path)
            .ok_or_else(|| Error::Other(format!("непонятные координаты {}", install.path)))?;
        let forge_jar = data.libraries().join(&forge_rel);
        write_file(
            &forge_jar,
            &read_entry(&mut zip, &installer, &install.file_path)?,
        )?;
        let version = legacy_libraries(dl, info.clone(), &install.path, &forge_rel).await?;
        save_version(data, id, version)?;
        return Ok(vec![forge_jar]);
    }

    let profile: InstallProfile =
        serde_json::from_value(profile).map_err(|source| Error::Json {
            what: "описание установщика".into(),
            source,
        })?;
    let version: Value = json(
        &read_entry(&mut zip, &installer, &profile.json)?,
        "описание версии",
    )?;
    drop(zip);

    // Файлы из папки maven/ установщика — то, что больше негде взять.
    let libs = data.libraries();
    let bundled = {
        let (installer, libs) = (installer.clone(), libs.clone());
        tokio::task::spawn_blocking(move || extract_maven(&installer, &libs))
            .await
            .map_err(|e| Error::Other(format!("распаковка прервана: {e}")))??
    };

    // Библиотеки для процессоров.
    let tasks: Vec<Task> = profile
        .libraries
        .iter()
        .filter_map(|lib| library_task(&libs, lib))
        .filter(|t| {
            !t.sha1
                .as_ref()
                .is_some_and(|s| sha1_file(&t.path).is_ok_and(|h| h.eq_ignore_ascii_case(s)))
        })
        .collect();
    if !tasks.is_empty() {
        progress(Event::Downloading {
            files: tasks.len(),
            bytes: tasks.iter().filter_map(|t| t.size).sum(),
        });
        let p = progress.clone();
        dl.download_all(tasks, &move |t| {
            p(Event::FileDone {
                bytes: t.size.unwrap_or(0),
            })
        })
        .await?;
    }

    let work = data.cache().join("installers").join(format!("{id}-work"));
    let ctx = Context {
        tokens: HashMap::new(),
        libs: libs.clone(),
        java: crate::java::executable(&vanilla.java_home, true),
        work: work.clone(),
        cancel: cancel.clone(),
    };
    let made = {
        let installer = installer.clone();
        let data_map = profile.data;
        let processors = profile.processors;
        let builtins = [
            ("SIDE", "client".to_string()),
            ("MINECRAFT_JAR", data.client_jar(game).display().to_string()),
            ("MINECRAFT_VERSION", game.to_string()),
            ("ROOT", data.root().display().to_string()),
            ("INSTALLER", installer.display().to_string()),
            ("LIBRARY_DIR", libs.display().to_string()),
        ];
        tokio::task::spawn_blocking(move || {
            let mut ctx = ctx;
            ctx.prepare_tokens(&installer, data_map, &builtins)?;
            ctx.run(&processors, &progress)
        })
        .await
        .map_err(|e| Error::Other(format!("установка прервана: {e}")))??
    };
    let _ = std::fs::remove_dir_all(&work);

    // Библиотеки без адреса делает установщик — они должны уже лежать.
    let local: Vec<PathBuf> = version["libraries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|lib| lib["downloads"]["artifact"]["url"].as_str() == Some(""))
        .filter_map(|lib| {
            let artifact = &lib["downloads"]["artifact"];
            artifact["path"]
                .as_str()
                .map(str::to_string)
                .or_else(|| lib["name"].as_str().and_then(maven_path))
                .map(|rel| libs.join(rel))
        })
        .collect();
    if let Some(missing) = local.iter().find(|p| !p.exists()) {
        return Err(Error::Other(format!(
            "установщик не сделал файл {} — попробуйте другую версию загрузчика",
            missing.display()
        )));
    }
    save_version(data, id, version)?;
    let mut files = local;
    files.extend(made);
    files.extend(
        bundled
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "jar")),
    );
    Ok(files)
}

/// Что скачать для библиотеки из описания установщика; `None` — качать не надо
/// (лежит в установщике или нет адреса).
fn library_task(libs: &Path, lib: &meta::Library) -> Option<Task> {
    let artifact = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref());
    let (rel, url, sha1, size) = match artifact {
        Some(a) => (
            a.path.clone().or_else(|| maven_path(&lib.name))?,
            a.url.clone(),
            a.sha1.clone(),
            a.size,
        ),
        None => {
            let rel = maven_path(&lib.name)?;
            let base = lib.url.as_deref().unwrap_or(LIBRARIES_URL);
            let url = format!("{}/{rel}", base.trim_end_matches('/'));
            (rel, url, lib.sha1.clone(), lib.size)
        }
    };
    if url.is_empty() {
        return None;
    }
    Some(Task {
        url,
        path: libs.join(rel),
        sha1,
        size,
    })
}

/// Распаковать папку `maven/` установщика в библиотеки. Возвращает пути файлов.
fn extract_maven(installer: &Path, libs: &Path) -> Result<Vec<PathBuf>> {
    let mut zip = open_zip(installer)?;
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|source| Error::Zip {
            path: installer.to_path_buf(),
            source,
        })?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let Ok(rel) = rel.strip_prefix("maven") else {
            continue;
        };
        if entry.is_dir() || rel.as_os_str().is_empty() {
            continue;
        }
        let path = libs.join(rel);
        if std::fs::metadata(&path).is_ok_and(|m| m.len() == entry.size()) {
            out.push(path);
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).at(installer)?;
        write_file(&path, &bytes)?;
        out.push(path);
    }
    Ok(out)
}

/// Старый формат: адреса библиотек проверяем — Maven Forge отдаёт не все,
/// часть давно лежит только в Maven Central. Jar самого Forge — из установщика.
async fn legacy_libraries(
    dl: &Downloader,
    mut version: Value,
    forge_coords: &str,
    forge_rel: &str,
) -> Result<Value> {
    let Some(libs) = version["libraries"].as_array_mut() else {
        return Ok(version);
    };
    let checks = libs.iter().map(|lib| async move {
        let name = lib["name"].as_str().unwrap_or_default();
        let base = lib["url"].as_str()?;
        if name == forge_coords {
            return None;
        }
        let rel = maven_path(name)?;
        let forge = base
            .replace("http://", "https://")
            .replace("https://files.minecraftforge.net/maven/", FORGE_MAVEN);
        for candidate in [forge.as_str(), MAVEN_CENTRAL, LIBRARIES_URL] {
            let url = format!("{}/{rel}", candidate.trim_end_matches('/'));
            if dl.exists(&url).await {
                return Some(candidate.to_string());
            }
        }
        Some(String::new())
    });
    let found = join_all(checks).await;
    for (lib, base) in libs.iter_mut().zip(found) {
        let name = lib["name"].as_str().unwrap_or_default().to_string();
        let Some(obj) = lib.as_object_mut() else {
            continue;
        };
        obj.remove("checksums");
        obj.remove("serverreq");
        if name == forge_coords {
            obj.remove("url");
            obj.insert(
                "downloads".into(),
                serde_json::json!({"artifact": {"path": forge_rel, "url": ""}}),
            );
            continue;
        }
        match base {
            Some(base) if base.is_empty() => {
                return Err(Error::Other(format!(
                    "библиотеку {name} для Forge не нашли ни в одном репозитории"
                )));
            }
            Some(base) => {
                obj.insert("url".into(), Value::String(base));
            }
            None => {}
        }
    }
    Ok(version)
}

/// Подстановки установщика (`{MINECRAFT_JAR}`, `{MAPPINGS}`…) и запуск процессоров.
struct Context {
    tokens: HashMap<String, String>,
    libs: PathBuf,
    java: PathBuf,
    /// Временная папка: файлы из установщика (`/data/client.lzma`).
    work: PathBuf,
    /// «Отменить»: шаг установки останавливается, его процесс Java завершается.
    cancel: Cancel,
}

impl Context {
    fn artifact(&self, coords: &str) -> Result<PathBuf> {
        maven_path(coords.trim_start_matches('[').trim_end_matches(']'))
            .map(|rel| self.libs.join(rel))
            .ok_or_else(|| Error::Other(format!("непонятные координаты {coords}")))
    }

    fn prepare_tokens(
        &mut self,
        installer: &Path,
        data: HashMap<String, Sides>,
        builtins: &[(&str, String)],
    ) -> Result<()> {
        let mut zip = open_zip(installer)?;
        for (key, sides) in data {
            let raw = sides.client;
            let value = if raw.starts_with('[') && raw.ends_with(']') {
                self.artifact(&raw)?.display().to_string()
            } else if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
                raw[1..raw.len() - 1].to_string()
            } else if raw.starts_with('/') {
                let path = self.work.join(raw.trim_start_matches('/'));
                write_file(&path, &read_entry(&mut zip, installer, &raw)?)?;
                path.display().to_string()
            } else {
                raw
            };
            self.tokens.insert(key, value);
        }
        for (key, value) in builtins {
            self.tokens.insert(key.to_string(), value.clone());
        }
        Ok(())
    }

    /// `{КЛЮЧ}` в строке → значение. Неизвестный ключ — ошибка: молча
    /// подставить пустоту значит испортить установку.
    fn replace(&self, text: &str) -> Result<String> {
        let mut out = String::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            let end = after
                .find('}')
                .ok_or_else(|| Error::Other(format!("непонятный аргумент установщика: {text}")))?;
            let key = &after[..end];
            let value = self
                .tokens
                .get(key)
                .ok_or_else(|| Error::Other(format!("установщик ждёт неизвестное «{key}»")))?;
            out.push_str(value);
            rest = &after[end + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    fn arg(&self, arg: &str) -> Result<String> {
        if arg.starts_with('[') && arg.ends_with(']') {
            Ok(self.artifact(arg)?.display().to_string())
        } else {
            self.replace(arg)
        }
    }

    fn run(&self, processors: &[Processor], progress: &Progress) -> Result<Vec<PathBuf>> {
        let client: Vec<&Processor> = processors
            .iter()
            .filter(|p| {
                p.sides
                    .as_ref()
                    .is_none_or(|s| s.iter().any(|x| x == "client"))
            })
            .collect();
        let mut made = Vec::new();
        for (i, p) in client.iter().enumerate() {
            self.cancel.check()?;
            progress(Event::Loader {
                step: i + 1,
                total: client.len(),
            });
            let outputs = p
                .outputs
                .iter()
                .map(|(path, sha1)| Ok((PathBuf::from(self.replace(path)?), self.replace(sha1)?)))
                .collect::<Result<Vec<_>>>()?;
            let done = |outputs: &[(PathBuf, String)]| {
                outputs
                    .iter()
                    .all(|(path, sha1)| sha1_file(path).is_ok_and(|h| h.eq_ignore_ascii_case(sha1)))
            };
            // Уже сделано в прошлый раз (установка оборвалась позже) — не повторяем.
            if !outputs.is_empty() && done(&outputs) {
                made.extend(outputs.into_iter().map(|(p, _)| p));
                continue;
            }
            let jar = self.artifact(&p.jar)?;
            let main = main_class(&jar)?;
            let mut classpath = vec![jar];
            for c in &p.classpath {
                classpath.push(self.artifact(c)?);
            }
            let classpath = classpath
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(";");
            let args = p
                .args
                .iter()
                .map(|a| self.arg(a))
                .collect::<Result<Vec<_>>>()?;
            std::fs::create_dir_all(&self.work).at(&self.work)?;
            let mut command = std::process::Command::new(&self.java);
            command
                .arg("-cp")
                .arg(&classpath)
                .arg(&main)
                .args(&args)
                .current_dir(&self.work);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                command.creation_flags(CREATE_NO_WINDOW);
            }
            // Вывод — в файл: так процесс можно ждать по кусочку и завершить
            // по «Отменить», не боясь, что он встанет на переполненном канале.
            let log = self.work.join("processor.log");
            let out = std::fs::File::create(&log).at(&log)?;
            command.stdout(out.try_clone().at(&log)?).stderr(out);
            let mut child = command.spawn().map_err(|e| {
                Error::Other(format!("не удалось запустить Java для установки: {e}"))
            })?;
            let status = loop {
                if let Some(status) = child.try_wait().at(&self.java)? {
                    break status;
                }
                if self.cancel.is_cancelled() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Error::Cancelled);
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            };
            if !status.success() {
                let text = std::fs::read(&log)
                    .map(|b| String::from_utf8_lossy(&b).to_string())
                    .unwrap_or_default();
                let tail: Vec<&str> = text.lines().rev().take(6).collect();
                return Err(Error::Other(format!(
                    "установка: шаг {} из {} ({}) завершился с ошибкой:\n{}",
                    i + 1,
                    client.len(),
                    p.jar,
                    tail.into_iter().rev().collect::<Vec<_>>().join("\n")
                )));
            }
            if !outputs.is_empty() && !done(&outputs) {
                return Err(Error::Other(format!(
                    "установка: шаг {} из {} ({}) сделал не тот файл — хеш не совпал",
                    i + 1,
                    client.len(),
                    p.jar
                )));
            }
            made.extend(outputs.into_iter().map(|(p, _)| p));
        }
        Ok(made)
    }
}

/// Главный класс из `META-INF/MANIFEST.MF` (строки длиннее 72 символов
/// переносятся с пробелом в начале).
fn main_class(jar: &Path) -> Result<String> {
    let mut zip = open_zip(jar)?;
    let bytes = read_entry(&mut zip, jar, "META-INF/MANIFEST.MF")?;
    let text = String::from_utf8_lossy(&bytes)
        .replace("\r\n", "\n")
        .replace("\n ", "");
    text.lines()
        .find_map(|l| l.strip_prefix("Main-Class:"))
        .map(|m| m.trim().to_string())
        .ok_or_else(|| Error::Other(format!("в {} нет главного класса", jar.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_maven_versions() {
        let xml = "<versions><version>1.20.1-47.4.0</version><version>1.7.10-10.13.4.1614-1.7.10</version></versions>";
        let builds = parse_forge_meta(xml);
        assert_eq!(builds[0].game, "1.20.1");
        assert_eq!(builds[0].version, "47.4.0");
        assert_eq!(builds[1].version, "10.13.4.1614");
        assert_eq!(builds[1].maven, "1.7.10-10.13.4.1614-1.7.10");
    }

    #[test]
    fn neoforge_versions_map_to_games() {
        assert_eq!(neo_game("21.1.77").as_deref(), Some("1.21.1"));
        assert_eq!(neo_game("21.0.5-beta").as_deref(), Some("1.21"));
        assert_eq!(neo_game("20.2.3-beta").as_deref(), Some("1.20.2"));
        assert_eq!(neo_game("26.1.2.115").as_deref(), Some("26.1.2"));
        assert_eq!(neo_game("26.1.0.19-beta").as_deref(), Some("26.1"));
        assert_eq!(neo_game("0.25w14craftmine.3-beta"), None);
        assert_eq!(neo_game("26.1.0.0-alpha.1+snapshot-1"), None);
    }

    #[test]
    fn tokens_and_artifacts() {
        let ctx = Context {
            tokens: HashMap::from([
                ("SIDE".to_string(), "client".to_string()),
                ("ROOT".to_string(), "D:/data".to_string()),
            ]),
            libs: PathBuf::from("L"),
            java: PathBuf::new(),
            work: PathBuf::new(),
            cancel: Cancel::default(),
        };
        assert_eq!(ctx.arg("--side={SIDE}").unwrap(), "--side=client");
        assert_eq!(ctx.arg("{ROOT}/libraries/").unwrap(), "D:/data/libraries/");
        assert!(ctx.arg("{NOPE}").is_err());
        assert_eq!(
            PathBuf::from(ctx.arg("[net.minecraft:client:1.20.1:slim]").unwrap()),
            PathBuf::from("L").join("net/minecraft/client/1.20.1/client-1.20.1-slim.jar")
        );
    }
}
