//! Сведения о моде из самого jar (раздел 5.4 ТЗ): под какой загрузчик, id,
//! название, версия, зависимости и с чем мод несовместим. Нужны, чтобы
//! проверить мод не из каталога и чтобы найти недостающие зависимости.
//!
//! Форматы: `fabric.mod.json` (Fabric), `quilt.mod.json` (Quilt),
//! `META-INF/mods.toml` (Forge, NeoForge 1.20.1), `META-INF/neoforge.mods.toml`
//! (NeoForge 1.20.2+), `mcmod.info` (старый Forge).

use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    Fabric,
    Quilt,
    Forge,
    #[serde(rename = "neoforge")]
    NeoForge,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct JarMeta {
    /// Под какие загрузчики мод (бывают сразу под несколько).
    pub loaders: Vec<ModLoader>,
    pub mods: Vec<ModEntry>,
    /// id, которые мод приносит с собой: вложенные jar и `provides`.
    pub provides: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub depends: Vec<Dep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DepKind {
    Required,
    /// Fabric `breaks`, Forge `incompatible`: вместе игра не запустится.
    Breaks,
    /// Fabric `conflicts`: запустится, но автор предупреждает.
    Conflicts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RangeStyle {
    /// `>=0.15 <0.17`, `^1.2`, `1.20.x` (Fabric, Quilt).
    Semver,
    /// `[47,)`, `[1.20.1,1.21)` (Forge, NeoForge).
    Maven,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dep {
    pub id: String,
    pub kind: DepKind,
    /// Подходит любой из вариантов; пусто — любая версия.
    pub ranges: Vec<String>,
    pub style: RangeStyle,
}

impl Dep {
    /// Подходит ли версия.
    pub fn accepts(&self, version: &str) -> bool {
        self.ranges.is_empty()
            || self.ranges.iter().any(|r| match self.style {
                RangeStyle::Semver => semver_ok(r, version),
                RangeStyle::Maven => maven_ok(r, version),
            })
    }
}

/// Прочитать сведения из jar. Нет сведений — пустой `JarMeta` (библиотека,
/// совсем старый мод).
pub fn read(jar: &Path) -> JarMeta {
    let Ok(file) = std::fs::File::open(jar) else {
        return JarMeta::default();
    };
    read_zip(file, 0)
}

fn entry<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let mut e = zip.by_name(name).ok()?;
    if e.size() > 8 * 1024 * 1024 {
        return None;
    }
    let mut out = Vec::new();
    e.read_to_end(&mut out).ok()?;
    Some(out)
}

fn read_zip<R: Read + std::io::Seek>(reader: R, depth: usize) -> JarMeta {
    let Ok(mut zip) = zip::ZipArchive::new(reader) else {
        return JarMeta::default();
    };
    let mut meta = JarMeta::default();
    // Вложенные jar, которые объявил сам мод: путь может быть любым
    // (Forgix кладёт их в META-INF/forgix/multiversion/).
    let mut listed: Vec<String> = Vec::new();
    if let Some(bytes) = entry(&mut zip, "fabric.mod.json") {
        fabric(&bytes, &mut meta);
        listed.extend(jar_list(&bytes, &["jars"]));
    }
    if let Some(bytes) = entry(&mut zip, "quilt.mod.json") {
        quilt(&bytes, &mut meta);
        listed.extend(jar_list(&bytes, &["quilt_loader", "jars"]));
    }
    let manifest_version = entry(&mut zip, "META-INF/MANIFEST.MF").and_then(|m| {
        String::from_utf8_lossy(&m)
            .lines()
            .find_map(|l| l.strip_prefix("Implementation-Version:"))
            .map(|v| v.trim().to_string())
    });
    if let Some(bytes) = entry(&mut zip, "META-INF/neoforge.mods.toml") {
        toml_mods(
            &bytes,
            ModLoader::NeoForge,
            manifest_version.as_deref(),
            &mut meta,
        );
    }
    if let Some(bytes) = entry(&mut zip, "META-INF/mods.toml") {
        toml_mods(
            &bytes,
            ModLoader::Forge,
            manifest_version.as_deref(),
            &mut meta,
        );
    }
    if meta.loaders.is_empty()
        && let Some(bytes) = entry(&mut zip, "mcmod.info")
    {
        mcmod(&bytes, &mut meta);
    }
    // Вложенные jar — их моды считаются стоящими.
    if depth < 2 {
        let mut nested: Vec<String> = zip
            .file_names()
            .filter(|n| {
                (n.starts_with("META-INF/jars/") || n.starts_with("META-INF/jarjar/"))
                    && n.ends_with(".jar")
            })
            .map(str::to_string)
            .collect();
        for path in listed {
            let path = path.trim_start_matches('/').to_string();
            if !nested.contains(&path) {
                nested.push(path);
            }
        }
        for name in nested {
            if let Some(bytes) = entry(&mut zip, &name) {
                let inner = read_zip(std::io::Cursor::new(bytes), depth + 1);
                meta.provides.extend(inner.mods.into_iter().map(|m| m.id));
                meta.provides.extend(inner.provides);
            }
        }
    }
    meta.loaders.dedup();
    meta
}

/// Пути вложенных jar из описания мода: `"jars": [{"file": "…"}]` у Fabric,
/// `"quilt_loader": {"jars": ["…"]}` у Quilt.
fn jar_list(bytes: &[u8], at: &[&str]) -> Vec<String> {
    let Ok(mut v) = serde_json::from_slice::<Value>(bytes) else {
        return Vec::new();
    };
    for key in at {
        v = v[*key].take();
    }
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|j| j.as_str().or_else(|| j["file"].as_str()))
        .filter(|p| p.ends_with(".jar") && !p.contains(".."))
        .map(str::to_string)
        .collect()
}

/// Значение зависимости Fabric: строка или список строк.
fn ranges(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) if s.trim() == "*" => Vec::new(),
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => {
            let list: Vec<String> = a
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect();
            if list.iter().any(|s| s.trim() == "*") {
                Vec::new()
            } else {
                list
            }
        }
        _ => Vec::new(),
    }
}

fn fabric(bytes: &[u8], meta: &mut JarMeta) {
    // Встречаются комментарии и лишние запятые — такие файлы просто пропускаем.
    let Ok(v) = serde_json::from_slice::<Value>(bytes) else {
        return;
    };
    let Some(id) = v["id"].as_str() else { return };
    let mut depends = Vec::new();
    for (field, kind) in [
        ("depends", DepKind::Required),
        ("breaks", DepKind::Breaks),
        ("conflicts", DepKind::Conflicts),
    ] {
        if let Some(map) = v[field].as_object() {
            for (dep, range) in map {
                depends.push(Dep {
                    id: dep.clone(),
                    kind,
                    ranges: ranges(range),
                    style: RangeStyle::Semver,
                });
            }
        }
    }
    meta.loaders.push(ModLoader::Fabric);
    meta.mods.push(ModEntry {
        id: id.to_string(),
        name: v["name"].as_str().unwrap_or(id).to_string(),
        version: v["version"].as_str().unwrap_or_default().to_string(),
        depends,
    });
    if let Some(list) = v["provides"].as_array() {
        meta.provides
            .extend(list.iter().filter_map(|x| x.as_str().map(str::to_string)));
    }
}

fn quilt(bytes: &[u8], meta: &mut JarMeta) {
    let Ok(v) = serde_json::from_slice::<Value>(bytes) else {
        return;
    };
    let q = &v["quilt_loader"];
    let Some(id) = q["id"].as_str() else { return };
    let mut depends = Vec::new();
    for (field, kind) in [("depends", DepKind::Required), ("breaks", DepKind::Breaks)] {
        for d in q[field].as_array().into_iter().flatten() {
            let (dep, optional) = match d {
                Value::String(s) => (s.clone(), false),
                obj => (
                    obj["id"].as_str().unwrap_or_default().to_string(),
                    obj["optional"].as_bool().unwrap_or(false),
                ),
            };
            if !dep.is_empty() && !optional {
                depends.push(Dep {
                    id: dep,
                    kind,
                    ranges: ranges(&d["versions"]),
                    style: RangeStyle::Semver,
                });
            }
        }
    }
    if !meta.loaders.contains(&ModLoader::Fabric) {
        meta.mods.push(ModEntry {
            id: id.to_string(),
            name: q["metadata"]["name"].as_str().unwrap_or(id).to_string(),
            version: q["version"].as_str().unwrap_or_default().to_string(),
            depends,
        });
    }
    meta.loaders.push(ModLoader::Quilt);
}

fn toml_mods(bytes: &[u8], loader: ModLoader, manifest: Option<&str>, meta: &mut JarMeta) {
    let Ok(v) = toml::from_str::<toml::Value>(&String::from_utf8_lossy(bytes)) else {
        return;
    };
    let deps = v.get("dependencies");
    let mut added = false;
    for m in v
        .get("mods")
        .and_then(|m| m.as_array())
        .into_iter()
        .flatten()
    {
        let Some(id) = m.get("modId").and_then(|x| x.as_str()) else {
            continue;
        };
        let mut version = m
            .get("version")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string();
        if version.contains("${") {
            version = manifest.unwrap_or_default().to_string();
        }
        let mut depends = Vec::new();
        for d in deps
            .and_then(|d| d.get(id))
            .and_then(|d| d.as_array())
            .into_iter()
            .flatten()
        {
            let Some(dep) = d.get("modId").and_then(|x| x.as_str()) else {
                continue;
            };
            // Forge: mandatory=true/false; NeoForge: type="required"/"optional"/"incompatible".
            let kind = match d.get("type").and_then(|t| t.as_str()) {
                Some("required") => Some(DepKind::Required),
                Some("incompatible") => Some(DepKind::Breaks),
                Some(_) => None,
                None if d.get("mandatory").and_then(|m| m.as_bool()) == Some(true) => {
                    Some(DepKind::Required)
                }
                None => None,
            };
            let Some(kind) = kind else { continue };
            let range = d
                .get("versionRange")
                .and_then(|x| x.as_str())
                .unwrap_or_default();
            depends.push(Dep {
                id: dep.to_string(),
                kind,
                ranges: if range.is_empty() || range == "*" {
                    Vec::new()
                } else {
                    vec![range.to_string()]
                },
                style: RangeStyle::Maven,
            });
        }
        // Один и тот же мод бывает описан и для Forge, и для NeoForge.
        if !meta.mods.iter().any(|x| x.id == id) {
            meta.mods.push(ModEntry {
                id: id.to_string(),
                name: m
                    .get("displayName")
                    .and_then(|x| x.as_str())
                    .unwrap_or(id)
                    .to_string(),
                version,
                depends,
            });
        }
        added = true;
    }
    if added {
        meta.loaders.push(loader);
    }
}

fn mcmod(bytes: &[u8], meta: &mut JarMeta) {
    let Ok(v) = serde_json::from_slice::<Value>(bytes) else {
        return;
    };
    let list = match &v {
        Value::Array(a) => a.clone(),
        obj => obj["modList"].as_array().cloned().unwrap_or_default(),
    };
    for m in list {
        if let Some(id) = m["modid"].as_str() {
            meta.mods.push(ModEntry {
                id: id.to_string(),
                name: m["name"].as_str().unwrap_or(id).to_string(),
                version: m["version"].as_str().unwrap_or_default().to_string(),
                depends: Vec::new(),
            });
        }
    }
    if !meta.mods.is_empty() {
        meta.loaders.push(ModLoader::Forge);
    }
}

/// Числа версии: `1.21.1` → [1, 21, 1], `0.16.10+build.3` → [0, 16, 10].
fn nums(v: &str) -> Vec<u64> {
    let core = v.split(['+', '-']).next().unwrap_or(v);
    core.split('.').map_while(|p| p.parse().ok()).collect()
}

fn cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut x, mut y) = (nums(a), nums(b));
    let len = x.len().max(y.len());
    x.resize(len, 0);
    y.resize(len, 0);
    x.cmp(&y)
}

/// Диапазон Fabric: условия через пробел (все), `*`, `1.20.x`, `^`, `~`, `>=`…
pub fn semver_ok(range: &str, version: &str) -> bool {
    use std::cmp::Ordering::*;
    range.split_whitespace().all(|cond| {
        let (op, v) = [">=", "<=", ">", "<", "=", "^", "~"]
            .iter()
            .find_map(|o| cond.strip_prefix(o).map(|rest| (*o, rest)))
            .unwrap_or(("", cond));
        if v == "*" || v.is_empty() {
            return true;
        }
        // После «+» — метаданные сборки (2.0.0+26.1.x): по правилам semver
        // на сравнение они не влияют, и «x» в них — не шаблон версии.
        let v = v.split_once('+').map_or(v, |(core, _)| core);
        if v.contains(['x', 'X', '*']) {
            let prefix: Vec<&str> = v
                .split('.')
                .take_while(|p| !matches!(*p, "x" | "X" | "*"))
                .collect();
            let have: Vec<String> = version.split('.').map(str::to_string).collect();
            return prefix
                .iter()
                .enumerate()
                .all(|(i, p)| have.get(i).is_some_and(|h| h == p));
        }
        let c = cmp(version, v);
        match op {
            ">=" => c != Less,
            "<=" => c != Greater,
            ">" => c == Greater,
            "<" => c == Less,
            "^" => {
                let n = nums(v);
                let upper = match n.as_slice() {
                    [0, minor, ..] => format!("0.{}", minor + 1),
                    [major, ..] => format!("{}", major + 1),
                    _ => return true,
                };
                c != Less && cmp(version, &upper) == Less
            }
            "~" => {
                let n = nums(v);
                let upper = match n.as_slice() {
                    [major, minor, ..] => format!("{major}.{}", minor + 1),
                    [major] => format!("{}", major + 1),
                    _ => return true,
                };
                c != Less && cmp(version, &upper) == Less
            }
            // Без оператора и `=` — ровно эта версия (по числам).
            _ => c == Equal,
        }
    })
}

/// Диапазон Maven: `[47,)`, `(,48)`, `[1.20.1,1.21)`, несколько через запятую
/// между скобками; голая версия — «не младше».
pub fn maven_ok(range: &str, version: &str) -> bool {
    use std::cmp::Ordering::*;
    let range = range.trim();
    if !range.starts_with(['[', '(']) {
        return cmp(version, range) != Less;
    }
    // Разбить на отдельные отрезки: «[1,2),[3,)».
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    for (i, c) in range.char_indices() {
        match c {
            '[' | '(' => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
            }
            ']' | ')' => {
                depth -= 1;
                if depth == 0 {
                    parts.push(&range[start..=i]);
                }
            }
            _ => {}
        }
    }
    parts.iter().any(|p| {
        let inner = &p[1..p.len() - 1];
        let (lo, hi) = match inner.split_once(',') {
            Some((l, h)) => (l.trim(), h.trim()),
            None => (inner.trim(), inner.trim()),
        };
        let lo_ok = lo.is_empty()
            || match (p.starts_with('['), cmp(version, lo)) {
                (true, c) => c != Less,
                (false, c) => c == Greater,
            };
        let hi_ok = hi.is_empty()
            || match (p.ends_with(']'), cmp(version, hi)) {
                (true, c) => c != Greater,
                (false, c) => c == Less,
            };
        lo_ok && hi_ok
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn jar(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut out);
            for (name, bytes) in entries {
                zip.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(bytes).unwrap();
            }
            zip.finish().unwrap();
        }
        out.into_inner()
    }

    /// Случаи из сборки автора «Фэнтези-выживание (26.1.2)», 18.10.2026.
    #[test]
    fn build_metadata_and_forgix_nested_jars() {
        // RightClickHarvest: jamlib >=2.0.0+26.1.x, стоит 2.3.1+26.1.x.
        assert!(semver_ok(">=2.0.0+26.1.x", "2.3.1+26.1.x"));
        assert!(!semver_ok(">=2.4.0+26.1.x", "2.3.1+26.1.x"));
        assert!(
            semver_ok("1.21.x", "1.21.4"),
            "шаблон до «+» работает как раньше"
        );
        assert!(!semver_ok("1.21.x", "1.20.1"));

        // Glowing Ores (Forgix): geo_mod вложен по пути из fabric.mod.json.
        let inner = jar(&[("fabric.mod.json", br#"{"id":"geo_mod","version":"1.0.3"}"#)]);
        let outer = jar(&[
            (
                "fabric.mod.json",
                br#"{"id":"glowingores","version":"1","depends":{"geo_mod":"*"},
                    "jars":[{"file":"META-INF/forgix/multiversion/GEO-v1.0.3-mc26.1.2.jar"}]}"#,
            ),
            (
                "META-INF/forgix/multiversion/GEO-v1.0.3-mc26.1.2.jar",
                &inner,
            ),
        ]);
        let meta = read_zip(std::io::Cursor::new(outer), 0);
        assert!(
            meta.provides.iter().any(|p| p == "geo_mod"),
            "{:?}",
            meta.provides
        );
    }

    #[test]
    fn ranges() {
        assert!(semver_ok(">=0.15.0", "0.16.10"));
        assert!(!semver_ok(">=0.17", "0.16.10"));
        assert!(semver_ok(">=1.20 <1.21", "1.20.4"));
        assert!(!semver_ok(">=1.20 <1.21", "1.21.1"));
        assert!(semver_ok("1.21.x", "1.21.1"));
        assert!(!semver_ok("1.20.x", "1.21.1"));
        assert!(semver_ok("~1.21", "1.21.1"));
        assert!(semver_ok("^0.16.0", "0.16.10") && !semver_ok("^0.16.0", "0.17.0"));
        assert!(semver_ok("1.21.1", "1.21.1") && !semver_ok("1.21", "1.21.1"));
        assert!(maven_ok("[47,)", "47.4.10"));
        assert!(!maven_ok("[48,)", "47.4.10"));
        assert!(maven_ok("[1.20.1,1.21)", "1.20.1") && !maven_ok("[1.20.1,1.21)", "1.21"));
        assert!(maven_ok("(,48)", "47.4.0"));
        assert!(maven_ok("[1,2),[3,)", "3.1"));
        assert!(maven_ok("47.1", "47.4.10"));
    }

    #[test]
    fn reads_fabric_and_forge() {
        let dir = tempfile::tempdir().unwrap();
        let nested = jar(&[(
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"fabric-api-base","version":"0.4"}"#,
        )]);
        let fabric = jar(&[
            (
                "fabric.mod.json",
                br#"{"schemaVersion":1,"id":"iris","name":"Iris","version":"1.8.12",
                    "depends":{"fabricloader":">=0.15","minecraft":["1.21","1.21.1"],"sodium":"*"},
                    "breaks":{"optifabric":"*"}}"#,
            ),
            ("META-INF/jars/base.jar", &nested),
        ]);
        let path = dir.path().join("iris.jar");
        std::fs::write(&path, fabric).unwrap();
        let m = read(&path);
        assert_eq!(m.loaders, [ModLoader::Fabric]);
        assert_eq!(m.mods[0].name, "Iris");
        assert_eq!(m.provides, ["fabric-api-base"]);
        let mc = m.mods[0]
            .depends
            .iter()
            .find(|d| d.id == "minecraft")
            .unwrap();
        assert!(mc.accepts("1.21.1") && !mc.accepts("1.20.1"));
        assert!(
            m.mods[0]
                .depends
                .iter()
                .any(|d| d.id == "optifabric" && d.kind == DepKind::Breaks)
        );

        let toml = br#"
modLoader="javafml"
loaderVersion="[47,)"
[[mods]]
modId="jei"
version="${file.jarVersion}"
displayName="Just Enough Items"
[[dependencies.jei]]
    modId="forge"
    mandatory=true
    versionRange="[47.1,)"
    side="BOTH"
[[dependencies.jei]]
    modId="minecraft"
    mandatory=true
    versionRange="[1.20.1,1.21)"
"#;
        let forge = jar(&[
            ("META-INF/mods.toml", toml),
            (
                "META-INF/MANIFEST.MF",
                b"Manifest-Version: 1.0\r\nImplementation-Version: 15.2.0\r\n",
            ),
        ]);
        let path = dir.path().join("jei.jar");
        std::fs::write(&path, forge).unwrap();
        let m = read(&path);
        assert_eq!(m.loaders, [ModLoader::Forge]);
        assert_eq!(
            (m.mods[0].name.as_str(), m.mods[0].version.as_str()),
            ("Just Enough Items", "15.2.0")
        );
        let forge_dep = m.mods[0].depends.iter().find(|d| d.id == "forge").unwrap();
        assert!(forge_dep.accepts("47.4.10") && !forge_dep.accepts("47.0.3"));

        let empty = dir.path().join("lib.jar");
        std::fs::write(&empty, jar(&[("a/B.class", b"x")])).unwrap();
        assert_eq!(read(&empty), JarMeta::default());
    }
}
