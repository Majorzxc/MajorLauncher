//! Экспорт сборки в `.mrpack` (раздел 5.5 ТЗ): стандартный формат Modrinth,
//! его понимают Prism, Modrinth App и сам MajorLauncher. Моды, ресурспаки и
//! шейдеры с Modrinth попадают в файл ссылкой с хешами — сборка весит
//! килобайты и воспроизводится один в один. Свои файлы (не с Modrinth),
//! настройки модов и прочее — в `overrides/` как есть.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::builds::{Builds, Instance};
use crate::download::Downloader;
use crate::error::{Error, IoContext, Result};
use crate::loaders::Loader;
use crate::modrinth;
use crate::mods;
use crate::paths::DataDir;

/// Что положить в файл, кроме включённых модов.
#[derive(Debug, Clone, Deserialize)]
pub struct Options {
    pub name: String,
    /// Версия самой сборки: «1.0».
    pub version: String,
    #[serde(default)]
    pub summary: String,
    /// Настройки модов: `config/`, `defaultconfigs/`.
    #[serde(default)]
    pub configs: bool,
    /// Ресурспаки и шейдеры сборки.
    #[serde(default)]
    pub packs: bool,
    /// Настройки игры `options.txt` (клавиши, звук, графика).
    #[serde(default)]
    pub options: bool,
    /// Выключенные моды — останутся выключенными.
    #[serde(default)]
    pub disabled: bool,
}

/// Что получится — для окна до экспорта.
#[derive(Debug, Clone, Serialize)]
pub struct Preview {
    pub name: String,
    pub game: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    /// Включённые моды с Modrinth — ссылкой.
    pub modrinth: usize,
    /// Свои файлы модов — целиком в файл.
    pub own: Vec<String>,
    pub disabled: usize,
    pub packs: usize,
    pub config_size: u64,
    pub has_options: bool,
    /// Modrinth не ответил — моды, которые он не опознал по кэшу, уйдут файлами.
    pub offline: bool,
}

/// Итог экспорта.
#[derive(Debug, Clone, Serialize)]
pub struct Exported {
    pub path: String,
    pub size: u64,
    pub linked: usize,
    pub embedded: usize,
}

/// Папки сборки с настройками модов.
const CONFIG_DIRS: &[&str] = &["config", "defaultconfigs"];

struct Item {
    /// Путь в сборке через «/»: `mods/sodium.jar`.
    rel: String,
    abs: PathBuf,
}

fn files_in(game: &Path, dir: &str, deep: bool) -> Vec<Item> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::from(dir)];
    while let Some(rel) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(game.join(&rel)) else {
            continue;
        };
        for e in entries.flatten() {
            let path = rel.join(e.file_name());
            match e.file_type() {
                Ok(t) if t.is_dir() => {
                    if deep {
                        stack.push(path);
                    }
                }
                Ok(_) => out.push(Item {
                    rel: path.to_string_lossy().replace('\\', "/"),
                    abs: game.join(&path),
                }),
                Err(_) => {}
            }
        }
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

fn dir_size(items: &[Item]) -> u64 {
    items
        .iter()
        .filter_map(|i| std::fs::metadata(&i.abs).ok())
        .map(|m| m.len())
        .sum()
}

/// Что попадёт в файл: моды, паки, настройки — по разделам.
struct Gathered {
    instance: Instance,
    mods: Vec<Item>,
    disabled: Vec<Item>,
    packs: Vec<Item>,
    configs: Vec<Item>,
    options: Option<Item>,
}

fn gather(data: &DataDir, build: &str) -> Result<Gathered> {
    let instance = Builds::new(data.clone()).get(build)?;
    if instance.quick {
        return Err(Error::Other(
            "«Быструю игру» экспортировать нельзя: это не сборка, а общая папка для любой версии"
                .into(),
        ));
    }
    let game = data.instance(build);
    let (disabled, mods): (Vec<Item>, Vec<Item>) = files_in(&game, "mods", false)
        .into_iter()
        .filter(|i| i.rel.ends_with(".jar") || i.rel.ends_with(".jar.disabled"))
        .partition(|i| i.rel.ends_with(".disabled"));
    let packs = ["resourcepacks", "shaderpacks"]
        .iter()
        .flat_map(|d| files_in(&game, d, false))
        .filter(|i| i.rel.ends_with(".zip"))
        .collect();
    let configs = CONFIG_DIRS
        .iter()
        .flat_map(|d| files_in(&game, d, true))
        .collect();
    let options = game.join("options.txt");
    Ok(Gathered {
        instance,
        mods,
        disabled,
        packs,
        configs,
        options: options.exists().then(|| Item {
            rel: "options.txt".into(),
            abs: options,
        }),
    })
}

/// Хеши файлов и что о них знает Modrinth.
async fn known(
    dl: &Downloader,
    data: &DataDir,
    items: &[&Item],
    online: bool,
) -> Result<(HashMap<String, String>, mods::Identified)> {
    let paths: Vec<PathBuf> = items.iter().map(|i| i.abs.clone()).collect();
    let hashes = tokio::task::spawn_blocking(move || {
        paths
            .iter()
            .map(|p| crate::verify::sha1_file(p).map(|h| (p.display().to_string(), h)))
            .collect::<Result<HashMap<_, _>>>()
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))??;
    let list: Vec<String> = hashes.values().cloned().collect();
    let ident = mods::identify(dl, data, &list, online).await;
    Ok((hashes, ident))
}

/// Что получится при экспорте. Спрашивает Modrinth, если он доступен.
pub async fn preview(dl: &Downloader, data: &DataDir, build: &str) -> Result<Preview> {
    let g = gather(data, build)?;
    let mods: Vec<&Item> = g.mods.iter().collect();
    let (hashes, ident) = known(dl, data, &mods, true).await?;
    let own = g
        .mods
        .iter()
        .filter(|i| {
            hashes
                .get(&i.abs.display().to_string())
                .is_none_or(|h| !ident.known.contains_key(h))
        })
        .map(|i| i.rel.trim_start_matches("mods/").to_string())
        .collect::<Vec<_>>();
    Ok(Preview {
        name: g.instance.name.clone(),
        game: g.instance.version.clone(),
        loader: g.instance.loader,
        loader_version: g.instance.loader_version.clone(),
        modrinth: g.mods.len() - own.len(),
        own,
        disabled: g.disabled.len(),
        packs: g.packs.len(),
        config_size: dir_size(&g.configs),
        has_options: g.options.is_some(),
        offline: !ident.answered,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexOut<'a> {
    format_version: u32,
    game: &'a str,
    version_id: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    summary: &'a str,
    files: Vec<FileOut>,
    dependencies: HashMap<&'a str, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileOut {
    path: String,
    hashes: HashMap<&'static str, String>,
    downloads: Vec<String>,
    file_size: u64,
}

/// Записать сборку в `.mrpack` по пути `out` (его выбрал игрок).
pub async fn export(
    dl: &Downloader,
    data: &DataDir,
    build: &str,
    opts: &Options,
    out: &Path,
) -> Result<Exported> {
    if !out.is_absolute() {
        return Err(Error::Other("укажите полный путь к файлу".into()));
    }
    let name = opts.name.trim();
    if name.is_empty() {
        return Err(Error::Other("у сборки должно быть название".into()));
    }
    let g = gather(data, build)?;

    // С Modrinth — только включённые моды и паки (выключенные кладём файлом,
    // чтобы они остались выключенными: формат не умеет «выключено»).
    let mut linkable: Vec<&Item> = g.mods.iter().collect();
    if opts.packs {
        linkable.extend(g.packs.iter());
    }
    let (hashes, ident) = known(dl, data, &linkable, true).await?;
    let ids: Vec<&str> = linkable
        .iter()
        .filter_map(|i| hashes.get(&i.abs.display().to_string()))
        .filter_map(|h| ident.known.get(h))
        .map(|k| k.version_id.as_str())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let versions = if ids.is_empty() {
        Vec::new()
    } else {
        // Адрес и SHA-512 файла — из Modrinth; без сети экспорт с модами не собрать.
        let mut all = Vec::new();
        for chunk in ids.chunks(100) {
            all.extend(modrinth::versions_by_id(dl, chunk).await?);
        }
        all
    };
    let by_sha1: HashMap<String, &modrinth::VersionFile> = versions
        .iter()
        .flat_map(|v| v.files.iter())
        .map(|f| (f.hashes.sha1.to_lowercase(), f))
        .collect();

    let mut files = Vec::new();
    let mut overrides: Vec<&Item> = Vec::new();
    for item in &linkable {
        let link = hashes
            .get(&item.abs.display().to_string())
            .and_then(|h| by_sha1.get(&h.to_lowercase()))
            .filter(|f| {
                f.url.starts_with("https://cdn.modrinth.com/") && !f.hashes.sha512.is_empty()
            });
        match link {
            Some(f) => files.push(FileOut {
                path: item.rel.clone(),
                hashes: HashMap::from([
                    ("sha1", f.hashes.sha1.to_lowercase()),
                    ("sha512", f.hashes.sha512.to_lowercase()),
                ]),
                downloads: vec![f.url.clone()],
                file_size: f.size,
            }),
            None => overrides.push(item),
        }
    }
    if opts.disabled {
        overrides.extend(g.disabled.iter());
    }
    if opts.configs {
        overrides.extend(g.configs.iter());
    }
    if opts.options
        && let Some(o) = &g.options
    {
        overrides.push(o);
    }

    let mut deps = HashMap::from([("minecraft", g.instance.version.clone())]);
    if let Some(v) = &g.instance.loader_version {
        let key = match g.instance.loader {
            Loader::Fabric => Some("fabric-loader"),
            Loader::Forge => Some("forge"),
            Loader::NeoForge => Some("neoforge"),
            Loader::Vanilla => None,
        };
        if let Some(key) = key {
            deps.insert(key, v.clone());
        }
    }
    let version = opts.version.trim();
    let index = IndexOut {
        format_version: 1,
        game: "minecraft",
        version_id: if version.is_empty() { "1.0" } else { version },
        name,
        summary: opts.summary.trim(),
        files,
        dependencies: deps,
    };
    let linked = index.files.len();
    let embedded = overrides.len();
    let index_bytes = serde_json::to_vec_pretty(&index).expect("сериализуется");
    let entries: Vec<(String, PathBuf)> = overrides
        .iter()
        .map(|i| (format!("overrides/{}", i.rel), i.abs.clone()))
        .collect();
    let target = out.to_path_buf();
    let size = tokio::task::spawn_blocking(move || write_zip(&target, &index_bytes, &entries))
        .await
        .map_err(|e| Error::Other(e.to_string()))??;
    Ok(Exported {
        path: out.display().to_string(),
        size,
        linked,
        embedded,
    })
}

/// Записать архив через временный файл: оборвалось — старого файла не испортили.
fn write_zip(out: &Path, index: &[u8], entries: &[(String, PathBuf)]) -> Result<u64> {
    let tmp = out.with_extension("mrpack.tmp");
    let made = (|| -> Result<()> {
        let file = std::fs::File::create(&tmp).at(&tmp)?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        let zerr = |source| Error::Zip {
            path: out.to_path_buf(),
            source,
        };
        zip.start_file("modrinth.index.json", opts).map_err(zerr)?;
        zip.write_all(index).at(&tmp)?;
        for (name, path) in entries {
            zip.start_file(name.as_str(), opts).map_err(zerr)?;
            let mut f = std::fs::File::open(path).at(path)?;
            std::io::copy(&mut f, &mut zip).at(path)?;
        }
        zip.finish().map_err(zerr)?;
        Ok(())
    })();
    if let Err(e) = made {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, out).at(out)?;
    Ok(std::fs::metadata(out).at(out)?.len())
}
