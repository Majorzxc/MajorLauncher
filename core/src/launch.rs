//! Сборка команды запуска игры. Ядро только готовит команду; запускает её
//! вызывающая сторона (`mlcli` сейчас, окно лаунчера на этапе 3).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::account::{Kind, Session};
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
    /// Сразу войти в мир или на сервер («Перезапустить и вернуться»).
    pub quick_play: Option<crate::playtime::LastPlace>,
}

#[derive(Debug, Clone)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Переменные окружения для процесса игры поверх обычных.
    pub env: Vec<(String, PathBuf)>,
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
    session: &Session,
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

    let mut vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", session.name.clone()),
        ("auth_uuid", session.uuid.clone()),
        ("auth_access_token", session.access_token.clone()),
        ("auth_session", "-".into()),
        ("auth_xuid", "0".into()),
        ("clientid", "0".into()),
        ("user_type", user_type(session.kind, modern).into()),
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
    let mut features = Features::default();
    // С 1.20 у игры есть штатный вход сразу в мир или на сервер — включаем
    // его признаками из описания версии. Раньше на сервер вели `--server`
    // и `--port`, а в одиночный мир — никак: игра откроется в меню.
    let mut legacy_server = Vec::new();
    if let Some(place) = &opts.quick_play {
        let supported = |var: &str| {
            version
                .arguments
                .as_ref()
                .is_some_and(|a| a.game.iter().any(|arg| arg_mentions(arg, var)))
        };
        match place {
            crate::playtime::LastPlace::World { folder, .. }
                if supported("${quickPlaySingleplayer}") =>
            {
                features.enabled.push("is_quick_play_singleplayer".into());
                vars.insert("quickPlaySingleplayer", folder.clone());
            }
            crate::playtime::LastPlace::Server { address } => match joins_server(version) {
                Join::QuickPlay => {
                    features.enabled.push("is_quick_play_multiplayer".into());
                    // Порт 25565 игра всё равно проверяет по записи SRV.
                    let full = match crate::servers::ping::Address::parse(address) {
                        Some(a) if a.port.is_none() => format!("{address}:25565"),
                        _ => address.clone(),
                    };
                    vars.insert("quickPlayMultiplayer", full);
                }
                Join::Args => {
                    if let Some(a) = crate::servers::ping::Address::parse(address) {
                        legacy_server = vec![
                            "--server".to_string(),
                            a.host,
                            "--port".to_string(),
                            a.port
                                .unwrap_or(crate::servers::ping::DEFAULT_PORT)
                                .to_string(),
                        ];
                    }
                }
                // До 1.6 игра не умеет заходить на сервер при запуске.
                Join::No => {}
            },
            _ => {}
        }
    }

    let mut args = vec![
        format!("-Xms{}M", opts.memory_mb.min(512)),
        format!("-Xmx{}M", opts.memory_mb),
    ];
    // authlib-injector (Ely.by) должен подключиться раньше, чем загрузится игра.
    if let Some(agent) = &session.agent {
        args.extend(agent.jvm_args());
    }
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
    args.extend(legacy_server);

    // До 1.6 игра держит настройки, миры, текстурпаки и журнал в
    // `%APPDATA%\.minecraft`, мимо `--gameDir`. Её папка игры — `<сборка>\.minecraft`
    // ([`home_dir`]), а APPDATA подменяется на саму сборку: так оба пути
    // ведут в одно место.
    let env = match game_dir.parent() {
        Some(build) if legacy_home(version) => vec![("APPDATA".to_string(), build.to_path_buf())],
        _ => Vec::new(),
    };
    Ok(LaunchCommand {
        program: java::executable(&prepared.java_home, opts.console),
        args,
        cwd: game_dir.clone(),
        env,
    })
}

/// Версия до 1.6: свою папку берёт из `%APPDATA%`, а не из `--gameDir`.
pub fn legacy_home(version: &crate::meta::VersionJson) -> bool {
    // Признак тот же, что у `Join::No`: старая строка аргументов — ник и сессия по порядку.
    version.arguments.is_none()
        && version
            .minecraft_arguments
            .as_deref()
            .is_some_and(|a| !a.contains("--username"))
}

/// Папка игры сборки — миры, настройки, серверы, скриншоты, логи: у версий
/// до 1.6 — `<сборка>\.minecraft` (туда указывает подменённый APPDATA), у
/// остальных — сама папка сборки.
pub fn home_dir(data: &DataDir, instance: &crate::builds::Instance) -> PathBuf {
    let dir = data.instance(&instance.id);
    if legacy_layout(data, &instance.version) {
        dir.join(".minecraft")
    } else {
        dir
    }
}

/// Подготовить папку игры перед запуском. У версий до 1.6 файлы игры, которые
/// лежат прямо в папке сборки (их туда положили импорт, копирование миров или
/// лаунчер до 1.0), переезжают в `.minecraft` — переименованием, без копий;
/// то, что там уже есть, не трогается.
pub fn prepare_home(data: &DataDir, instance: &crate::builds::Instance) -> Result<PathBuf> {
    let home = home_dir(data, instance);
    std::fs::create_dir_all(&home).at(&home)?;
    let root = data.instance(&instance.id);
    if home != root {
        for name in [
            "saves",
            "screenshots",
            "options.txt",
            "servers.dat",
            "texturepacks",
            "resourcepacks",
            "resources",
            "stats",
            "crash-reports",
        ] {
            let (from, to) = (root.join(name), home.join(name));
            if from.exists() && !to.exists() {
                std::fs::rename(&from, &to).at(&from)?;
            }
        }
    }
    Ok(home)
}

/// То же по номеру версии сборки: по скачанному описанию версии, а пока его
/// нет — по самому номеру (1.5.2, b1.7.3, снапшоты 12w… и 13w01–13w15).
pub fn legacy_layout(data: &DataDir, version: &str) -> bool {
    if let Ok(bytes) = std::fs::read(data.version_json(version))
        && let Ok(json) = serde_json::from_slice::<crate::meta::VersionJson>(&bytes)
    {
        return legacy_home(&json);
    }
    let old_prefix = ["a", "b", "c", "rd-", "inf-"].iter().any(|p| {
        version.starts_with(p) && version[p.len()..].starts_with(|c: char| c.is_ascii_digit())
    });
    if old_prefix {
        return true;
    }
    if let Some(rest) = version.strip_prefix("1.") {
        return rest
            .split(['.', '-', '_'])
            .next()
            .and_then(|m| m.parse::<u32>().ok())
            .is_some_and(|m| m < 6);
    }
    // Снапшоты вида 13w16a: год и неделя.
    let (year, week) = (version.get(..2), version.get(3..5));
    if version.get(2..3) == Some("w")
        && let (Some(Ok(y)), Some(Ok(w))) =
            (year.map(str::parse::<u32>), week.map(str::parse::<u32>))
    {
        return y < 13 || (y == 13 && w < 16);
    }
    false
}

/// Как версия заходит на сервер при запуске.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Join {
    /// С 1.20 — `--quickPlayMultiplayer`, игра сама ищет запись SRV.
    QuickPlay,
    /// 1.6–1.19 — `--server` и `--port`, без записи SRV.
    Args,
    /// До 1.6 — никак: сервер выбирают в игре.
    No,
}

pub fn joins_server(version: &crate::meta::VersionJson) -> Join {
    match (&version.arguments, &version.minecraft_arguments) {
        (Some(a), _)
            if a.game
                .iter()
                .any(|arg| arg_mentions(arg, "${quickPlayMultiplayer}")) =>
        {
            Join::QuickPlay
        }
        (Some(_), _) => Join::Args,
        // Параметры с именами (`--username`) — у нового Main с 1.6; до него —
        // ник и сессия просто по порядку.
        (None, Some(old)) if old.contains("--username") => Join::Args,
        _ => Join::No,
    }
}

/// Упоминает ли аргумент описания версии переменную (`${quickPlayMultiplayer}`).
fn arg_mentions(arg: &Argument, var: &str) -> bool {
    match arg {
        Argument::Plain(s) => s.contains(var),
        Argument::Conditional { value, .. } => match value {
            ArgValue::One(s) => s.contains(var),
            ArgValue::Many(list) => list.iter().any(|s| s.contains(var)),
        },
    }
}

/// Тип пользователя для аргумента `--userType`. Новые версии знают только `msa`
/// (тип влияет лишь на телеметрию), старые ждут `legacy` или `mojang`.
fn user_type(kind: Kind, modern: bool) -> &'static str {
    match (kind, modern) {
        (_, true) => "msa",
        (Kind::Offline, false) => "legacy",
        (Kind::Ely, false) => "mojang",
    }
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

    fn build_with(version_json: &str, quick: Option<crate::playtime::LastPlace>) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        let prepared = Prepared {
            version: serde_json::from_str(version_json).unwrap(),
            java_home: dir.path().join("java"),
            classpath: Vec::new(),
            natives_dir: dir.path().join("natives"),
            asset_index: serde_json::from_str(r#"{"objects":{}}"#).unwrap(),
            log_config: None,
        };
        let session = Session {
            kind: Kind::Offline,
            name: "Tester".into(),
            uuid: "u".into(),
            access_token: "t".into(),
            agent: None,
            unverified: false,
        };
        let opts = LaunchOptions {
            game_dir: dir.path().join("game"),
            memory_mb: 2048,
            console: false,
            extra_jvm_args: Vec::new(),
            quick_play: quick,
        };
        build(&data, &prepared, &session, &opts).unwrap().args
    }

    #[test]
    fn quick_play_into_world_or_server() {
        use crate::playtime::LastPlace;
        let modern = r#"{"id":"1.21.1","mainClass":"M","assetIndex":{"id":"17","url":"u","sha1":"s","size":1},
            "downloads":{"client":{"url":"u","sha1":"s","size":1}},
            "arguments":{"jvm":[],"game":["--username","${auth_player_name}",
              {"rules":[{"action":"allow","features":{"is_quick_play_singleplayer":true}}],"value":["--quickPlaySingleplayer","${quickPlaySingleplayer}"]},
              {"rules":[{"action":"allow","features":{"is_quick_play_multiplayer":true}}],"value":["--quickPlayMultiplayer","${quickPlayMultiplayer}"]}]}}"#;
        let world = LastPlace::World {
            folder: "Новый мир".into(),
            name: "Мир".into(),
        };
        let args = build_with(modern, Some(world));
        let at = args
            .iter()
            .position(|a| a == "--quickPlaySingleplayer")
            .unwrap();
        assert_eq!(args[at + 1], "Новый мир");
        assert!(!args.contains(&"--quickPlayMultiplayer".to_string()));

        let server = LastPlace::Server {
            address: "mc.example.org".into(),
        };
        let args = build_with(modern, Some(server.clone()));
        let at = args
            .iter()
            .position(|a| a == "--quickPlayMultiplayer")
            .unwrap();
        assert_eq!(args[at + 1], "mc.example.org:25565");

        // Без перезапуска — никаких лишних флагов.
        let args = build_with(modern, None);
        assert!(!args.iter().any(|a| a.starts_with("--quickPlay")));

        // Старая версия: на сервер — `--server`/`--port`, в мир — никак.
        let old = r#"{"id":"1.12.2","mainClass":"M","assetIndex":{"id":"1.12","url":"u","sha1":"s","size":1},
            "downloads":{"client":{"url":"u","sha1":"s","size":1}},
            "minecraftArguments":"--username ${auth_player_name}"}"#;
        let args = build_with(
            old,
            Some(LastPlace::Server {
                address: "play.net:25570".into(),
            }),
        );
        assert!(args.ends_with(&[
            "--server".into(),
            "play.net".into(),
            "--port".into(),
            "25570".into()
        ]));
        let args = build_with(
            old,
            Some(LastPlace::World {
                folder: "w".into(),
                name: "w".into(),
            }),
        );
        assert!(!args.contains(&"--server".to_string()));

        // IPv6 с портом — адрес без скобок, порт отдельно.
        let args = build_with(
            old,
            Some(LastPlace::Server {
                address: "[::1]:25566".into(),
            }),
        );
        assert!(args.ends_with(&[
            "--server".into(),
            "::1".into(),
            "--port".into(),
            "25566".into()
        ]));

        // 1.5.2: ник и сессия по порядку — `--server` игра не поймёт.
        let ancient = r#"{"id":"1.5.2","mainClass":"M","assetIndex":{"id":"pre-1.6","url":"u","sha1":"s","size":1},
            "downloads":{"client":{"url":"u","sha1":"s","size":1}},
            "minecraftArguments":"${auth_player_name} ${auth_session} --gameDir ${game_directory}"}"#;
        let args = build_with(
            ancient,
            Some(LastPlace::Server {
                address: "play.net".into(),
            }),
        );
        assert!(!args.contains(&"--server".to_string()));
    }

    #[test]
    fn old_versions_keep_their_files_in_the_build() {
        let ancient: crate::meta::VersionJson = serde_json::from_str(
            r#"{"id":"1.5.2","mainClass":"M","assetIndex":{"id":"pre-1.6","url":"u","sha1":"s","size":1},
            "downloads":{"client":{"url":"u","sha1":"s","size":1}},
            "minecraftArguments":"${auth_player_name} ${auth_session} --gameDir ${game_directory}"}"#,
        )
        .unwrap();
        assert!(legacy_home(&ancient));
        let modern: crate::meta::VersionJson = serde_json::from_str(
            r#"{"id":"1.6.4","mainClass":"M","assetIndex":{"id":"legacy","url":"u","sha1":"s","size":1},
            "downloads":{"client":{"url":"u","sha1":"s","size":1}},
            "minecraftArguments":"--username ${auth_player_name} --session ${auth_session}"}"#,
        )
        .unwrap();
        assert!(!legacy_home(&modern));

        // Описания версии ещё нет — по номеру.
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        for old in [
            "1.5.2",
            "1.0",
            "1.2.5",
            "b1.7.3",
            "a1.2.6",
            "c0.30_01c",
            "rd-132211",
            "12w50a",
            "13w10b",
        ] {
            assert!(legacy_layout(&data, old), "{old}");
        }
        for new in [
            "1.6.4",
            "1.12.2",
            "1.21.1",
            "26.3",
            "13w16a",
            "24w14a",
            "1.21.11-rc1",
            "something",
        ] {
            assert!(!legacy_layout(&data, new), "{new}");
        }

        // Файлы, лежавшие в корне старой сборки, переезжают в `.minecraft`.
        let b = crate::builds::Builds::new(data.clone())
            .create(
                "Старая",
                "1.5.2",
                crate::builds::LoaderChoice::default(),
                None,
            )
            .unwrap();
        let root = data.instance(&b.id);
        std::fs::create_dir_all(root.join("saves/Мир")).unwrap();
        std::fs::write(root.join("options.txt"), "fov:0.5").unwrap();
        let home = prepare_home(&data, &b).unwrap();
        assert_eq!(home, root.join(".minecraft"));
        assert!(home.join("saves/Мир").is_dir() && home.join("options.txt").exists());
        assert!(!root.join("saves").exists());
        // Новая версия — папка сборки и есть папка игры.
        let n = crate::builds::Builds::new(data.clone())
            .create(
                "Новая",
                "1.21.1",
                crate::builds::LoaderChoice::default(),
                None,
            )
            .unwrap();
        assert_eq!(prepare_home(&data, &n).unwrap(), data.instance(&n.id));
    }

    #[test]
    fn substitutes_known_and_keeps_unknown() {
        let vars = HashMap::from([("a", "1".to_string()), ("bb", "22".to_string())]);
        assert_eq!(substitute("x${a}y${bb}", &vars), "x1y22");
        assert_eq!(substitute("${nope}", &vars), "${nope}");
        assert_eq!(substitute("-D=${a}/java", &vars), "-D=1/java");
        assert_eq!(substitute("broken ${a", &vars), "broken ${a");
    }
}
