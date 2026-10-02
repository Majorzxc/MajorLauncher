//! Сборка команды запуска игры. Ядро только готовит команду; запускает её
//! вызывающая сторона (`mlcli` сейчас, окно лаунчера на этапе 3).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::account::OfflineAccount;
use crate::error::{IoContext, Result};
use crate::install::Prepared;
use crate::meta::{ArgValue, Argument};
use crate::paths::DataDir;
use crate::rules::{self, Features};
use crate::{VERSION, java};

pub const LAUNCHER_NAME: &str = "MajorLauncher";

#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// Папка игры сборки: миры, моды, настройки.
    pub game_dir: PathBuf,
    /// Максимум памяти для игры, МБ.
    pub memory_mb: u32,
    /// `java.exe` с консолью (для `mlcli`) или `javaw.exe` без неё.
    pub console: bool,
    pub extra_jvm_args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

/// Аргументы JVM для старых версий, в описании которых их нет.
const LEGACY_JVM_ARGS: &[&str] = &[
    "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump",
    "-Djava.library.path=${natives_directory}",
    "-Dminecraft.launcher.brand=${launcher_name}",
    "-Dminecraft.launcher.version=${launcher_version}",
    "-cp",
    "${classpath}",
];

pub fn build(
    data: &DataDir,
    prepared: &Prepared,
    account: &OfflineAccount,
    opts: &LaunchOptions,
) -> Result<LaunchCommand> {
    let version = &prepared.version;
    let game_dir = &opts.game_dir;
    std::fs::create_dir_all(game_dir).at(game_dir)?;

    // Новые версии хотят подпапки для разных нативных библиотек — создаём заранее.
    let natives = &prepared.natives_dir;
    for sub in ["", "java", "jna", "lwjgl", "netty"] {
        let dir = natives.join(sub);
        std::fs::create_dir_all(&dir).at(&dir)?;
    }

    let game_assets = place_legacy_assets(data, prepared, game_dir)?;
    let classpath = prepared
        .classpath
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(";");
    let modern = version.arguments.is_some();

    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", account.name.clone()),
        ("auth_uuid", account.uuid.clone()),
        // Офлайн-игре токен не нужен, но пустым его оставлять нельзя.
        ("auth_access_token", "0".into()),
        ("auth_session", "-".into()),
        ("auth_xuid", "0".into()),
        ("clientid", "0".into()),
        ("user_type", if modern { "msa" } else { "legacy" }.into()),
        ("user_properties", "{}".into()),
        ("version_name", version.id.clone()),
        (
            "version_type",
            if version.kind.is_empty() {
                "release".into()
            } else {
                version.kind.clone()
            },
        ),
        ("game_directory", game_dir.display().to_string()),
        ("assets_root", data.assets().display().to_string()),
        ("game_assets", game_assets.display().to_string()),
        ("assets_index_name", version.asset_index.id.clone()),
        ("natives_directory", natives.display().to_string()),
        ("library_directory", data.libraries().display().to_string()),
        ("classpath", classpath),
        ("classpath_separator", ";".into()),
        ("launcher_name", LAUNCHER_NAME.into()),
        ("launcher_version", VERSION.into()),
    ]);
    let features = Features::default();

    let mut args = vec![
        format!("-Xms{}M", opts.memory_mb.min(512)),
        format!("-Xmx{}M", opts.memory_mb),
    ];
    match &version.arguments {
        Some(a) => args.extend(expand(&a.jvm, &features, &vars)),
        None => args.extend(LEGACY_JVM_ARGS.iter().map(|a| substitute(a, &vars))),
    }
    if let (Some(path), Some(client)) = (
        &prepared.log_config,
        version.logging.as_ref().and_then(|l| l.client.as_ref()),
    ) {
        args.push(
            client
                .argument
                .replace("${path}", &path.display().to_string()),
        );
    }
    args.extend(opts.extra_jvm_args.iter().cloned());
    args.push(version.main_class.clone());
    match (&version.arguments, &version.minecraft_arguments) {
        (Some(a), _) => args.extend(expand(&a.game, &features, &vars)),
        (None, Some(line)) => args.extend(line.split_whitespace().map(|a| substitute(a, &vars))),
        (None, None) => {}
    }

    Ok(LaunchCommand {
        program: java::executable(&prepared.java_home, opts.console),
        args,
        cwd: game_dir.clone(),
    })
}

fn expand(list: &[Argument], features: &Features, vars: &HashMap<&str, String>) -> Vec<String> {
    let mut out = Vec::new();
    for arg in list {
        match arg {
            Argument::Plain(s) => out.push(substitute(s, vars)),
            Argument::Conditional { rules, value } if rules::allowed(rules, features) => {
                match value {
                    ArgValue::One(s) => out.push(substitute(s, vars)),
                    ArgValue::Many(v) => out.extend(v.iter().map(|s| substitute(s, vars))),
                }
            }
            Argument::Conditional { .. } => {}
        }
    }
    out
}

/// Подставить `${имя}` из `vars`. Неизвестные подстановки остаются как есть.
fn substitute(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..start + 2 + end + 1]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Старые версии ищут ассеты не по хешу, а по имени: до 1.7 — в `assets/virtual`,
/// до 1.6 — в папке `resources` внутри папки игры. Раскладываем копии.
/// Возвращает путь для подстановки `${game_assets}`.
fn place_legacy_assets(data: &DataDir, prepared: &Prepared, game_dir: &Path) -> Result<PathBuf> {
    let index = &prepared.asset_index;
    let target = if index.map_to_resources {
        game_dir.join("resources")
    } else if index.is_virtual {
        data.virtual_assets(&prepared.version.asset_index.id)
    } else {
        return Ok(data.assets());
    };

    for (name, obj) in &index.objects {
        let dst = target.join(name);
        if std::fs::metadata(&dst).is_ok_and(|m| m.len() == obj.size) {
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).at(parent)?;
        }
        let src = data.asset_object(&obj.hash);
        std::fs::copy(&src, &dst).at(&dst)?;
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_known_and_keeps_unknown() {
        let vars = HashMap::from([("a", "1".to_string()), ("bb", "22".to_string())]);
        assert_eq!(substitute("x${a}y${bb}", &vars), "x1y22");
        assert_eq!(substitute("${nope}", &vars), "${nope}");
        assert_eq!(substitute("-D=${a}/java", &vars), "-D=1/java");
        assert_eq!(substitute("broken ${a", &vars), "broken ${a");
    }
}
