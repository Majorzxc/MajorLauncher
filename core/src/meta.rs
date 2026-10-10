//! Описания версий от Mojang: список версий, JSON версии, индекс ассетов.
//! Поля, которые лаунчеру не нужны, не разбираем.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::rules::Rule;

pub const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";
pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net/";

#[derive(Debug, Clone, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    /// `release`, `snapshot`, `old_beta`, `old_alpha`.
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub sha1: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}

impl VersionManifest {
    /// Ищет версию по id; `release` и `snapshot` — последние версии.
    pub fn find(&self, id: &str) -> Option<&VersionEntry> {
        let id = match id {
            "release" | "latest" => self.latest.release.as_str(),
            "snapshot" => self.latest.snapshot.as_str(),
            other => other,
        };
        self.versions.iter().find(|v| v.id == id)
    }
}

/// Файл, который можно скачать: адрес, хеш и размер.
#[derive(Debug, Clone, Deserialize)]
pub struct Download {
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    pub path: Option<String>,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    /// Дата выхода (ISO 8601): по ней лаунчер понимает, какие настройки и
    /// форматы есть у версии. Сравнивается как строка.
    #[serde(rename = "releaseTime", default)]
    pub release_time: String,
    /// Версия с загрузчиком: поверх какой ванильной версии она стоит.
    #[serde(rename = "inheritsFrom", default)]
    pub inherits_from: Option<String>,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    /// Новые версии (1.13+): аргументы с правилами.
    pub arguments: Option<Arguments>,
    /// Старые версии: аргументы игры одной строкой.
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndexRef,
    pub downloads: VersionDownloads,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersion>,
    pub logging: Option<Logging>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

/// Аргумент — либо строка, либо значение с правилами.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    #[serde(rename = "totalSize")]
    pub total_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionDownloads {
    pub client: Download,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaVersion {
    pub component: String,
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Logging {
    pub client: Option<LoggingClient>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    /// Maven-координаты: `группа:имя:версия[:классификатор][@расширение]`.
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    /// Базовый адрес Maven-репозитория — у библиотек загрузчиков модов.
    pub url: Option<String>,
    /// Хеш и размер без блока `downloads` — так их даёт Fabric.
    pub sha1: Option<String>,
    pub size: Option<u64>,
    /// Старые установщики Forge: `false` — библиотека нужна только серверу.
    pub clientreq: Option<bool>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Старый формат нативных библиотек: ОС → классификатор.
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<Extract>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    /// Старые версии (до 1.7.?) ждут ассеты под их именами в отдельной папке.
    #[serde(rename = "virtual", default)]
    pub is_virtual: bool,
    /// Совсем старые версии (до 1.6) — в папке `resources` внутри папки игры.
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

/// Ключ библиотеки без версии: `группа:имя[:классификатор]`. По нему версия
/// с загрузчиком заменяет библиотеку ванили своей (Forge 1.12.2 везёт свой log4j).
pub fn library_key(name: &str) -> String {
    let coords = name.split('@').next().unwrap_or(name);
    let parts: Vec<&str> = coords.split(':').collect();
    match parts.as_slice() {
        [g, a, _, c, ..] => format!("{g}:{a}:{c}"),
        [g, a, ..] => format!("{g}:{a}"),
        _ => coords.to_string(),
    }
}

/// Описание версии с загрузчиком (`inheritsFrom`) поверх описания ванили —
/// так же, как это делает официальный лаунчер: главный класс и аргументы
/// загрузчика, его библиотеки раньше ванильных, остальное — от ванили.
pub fn inherit(child: &Value, parent: &Value) -> Value {
    let mut out = parent.clone();
    let (Some(out_map), Some(child_map)) = (out.as_object_mut(), child.as_object()) else {
        return out;
    };
    for (key, value) in child_map {
        match key.as_str() {
            "jar" | "_comment_" => {}
            "libraries" => {}
            "arguments" => {
                let merged = out_map
                    .entry("arguments")
                    .or_insert_with(|| Value::Object(Default::default()));
                for side in ["game", "jvm"] {
                    let extra = value
                        .get(side)
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    if let Some(obj) = merged.as_object_mut() {
                        let list = obj.entry(side).or_insert_with(|| Value::Array(Vec::new()));
                        if let Some(list) = list.as_array_mut() {
                            list.extend(extra);
                        }
                    }
                }
            }
            // Пустой `logging: {}` у Forge не отменяет настройку логов ванили.
            "logging" if value.get("client").is_none() => {}
            _ => {
                out_map.insert(key.clone(), value.clone());
            }
        }
    }
    let child_libs = child
        .get("libraries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let parent_libs = parent
        .get("libraries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    let libraries: Vec<Value> = child_libs
        .into_iter()
        .chain(parent_libs)
        .filter(|lib| {
            let name = lib.get("name").and_then(Value::as_str).unwrap_or_default();
            // Нативные библиотеки ванили (одно имя, разные правила) не схлопываем.
            lib.get("natives").is_some()
                || lib.get("rules").is_some()
                || seen.insert(library_key(name))
        })
        .collect();
    out_map.insert("libraries".into(), Value::Array(libraries));
    out
}

/// Путь внутри Maven-репозитория по координатам библиотеки:
/// `a.b:c:1.0:natives@zip` → `a/b/c/1.0/c-1.0-natives.zip`.
pub fn maven_path(name: &str) -> Option<String> {
    let (coords, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, "jar"),
    };
    let parts: Vec<&str> = coords.split(':').collect();
    let (group, artifact, version, classifier) = match parts.as_slice() {
        [g, a, v] => (*g, *a, *v, None),
        [g, a, v, c] => (*g, *a, *v, Some(*c)),
        _ => return None,
    };
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!(
        "{}/{artifact}/{version}/{file}",
        group.replace('.', "/")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3").as_deref(),
            Some("org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar")
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows").as_deref(),
            Some("org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar")
        );
        assert_eq!(
            maven_path("a.b:c:1.0@zip").as_deref(),
            Some("a/b/c/1.0/c-1.0.zip")
        );
        assert_eq!(maven_path("bad"), None);
    }

    #[test]
    fn child_version_overrides_parent() {
        let parent = serde_json::json!({
            "id": "1.12.2", "mainClass": "net.minecraft.client.main.Main",
            "minecraftArguments": "--username ${auth_player_name}",
            "assetIndex": {"id": "1.12"}, "logging": {"client": {"argument": "x"}},
            "libraries": [
                {"name": "org.apache.logging.log4j:log4j-core:2.8.1"},
                {"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4", "natives": {"windows": "natives-windows"}},
                {"name": "com.mojang:realms:1.10"}
            ]
        });
        let child = serde_json::json!({
            "id": "1.12.2-forge", "inheritsFrom": "1.12.2", "mainClass": "net.minecraft.launchwrapper.Launch",
            "minecraftArguments": "--username ${auth_player_name} --tweakClass X", "logging": {},
            "libraries": [{"name": "org.apache.logging.log4j:log4j-core:2.15.0"}, {"name": "net.minecraft:launchwrapper:1.12"}]
        });
        let v = inherit(&child, &parent);
        assert_eq!(v["id"], "1.12.2-forge");
        assert_eq!(v["mainClass"], "net.minecraft.launchwrapper.Launch");
        assert!(
            v["minecraftArguments"]
                .as_str()
                .unwrap()
                .contains("tweakClass")
        );
        assert_eq!(v["assetIndex"]["id"], "1.12");
        assert_eq!(
            v["logging"]["client"]["argument"], "x",
            "пустой logging не стирает ванильный"
        );
        let names: Vec<&str> = v["libraries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "org.apache.logging.log4j:log4j-core:2.15.0",
                "net.minecraft:launchwrapper:1.12",
                "org.lwjgl.lwjgl:lwjgl-platform:2.9.4",
                "com.mojang:realms:1.10"
            ]
        );

        let parent = serde_json::json!({"id": "1.21.1", "mainClass": "M", "arguments": {"game": ["--a"], "jvm": ["-b"]}, "libraries": []});
        let child = serde_json::json!({"id": "fabric", "mainClass": "K", "arguments": {"game": [], "jvm": ["-DFabricMcEmu= M "]}, "libraries": []});
        let v = inherit(&child, &parent);
        assert_eq!(
            v["arguments"]["jvm"],
            serde_json::json!(["-b", "-DFabricMcEmu= M "])
        );
        assert_eq!(v["arguments"]["game"], serde_json::json!(["--a"]));
        assert_eq!(
            library_key("a.b:c:1.0:natives-windows"),
            "a.b:c:natives-windows"
        );
        assert_eq!(library_key("a.b:c:1.0"), "a.b:c");
    }

    #[test]
    fn parses_mixed_arguments() {
        let json = r#"{"game": ["--username", {"rules": [{"action": "allow", "features": {"is_demo_user": true}}], "value": "--demo"}],
                       "jvm": [{"rules": [{"action": "allow", "os": {"name": "windows"}}], "value": ["-a", "-b"]}]}"#;
        let args: Arguments = serde_json::from_str(json).unwrap();
        assert!(matches!(args.game[0], Argument::Plain(_)));
        assert!(matches!(
            args.jvm[0],
            Argument::Conditional {
                value: ArgValue::Many(_),
                ..
            }
        ));
    }
}
