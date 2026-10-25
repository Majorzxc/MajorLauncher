//! Запуск сборки и наблюдение за игрой до её закрытия.
//!
//! Пока игра идёт, окна лаунчера нет (принцип 4 ТЗ): работает только этот
//! код — читает вывод игры, раз в пару секунд смотрит на заголовок её окна и
//! копит время по мирам и серверам ([`crate::playtime`]).

use std::collections::VecDeque;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::account::Accounts;
use crate::builds::Instance;
use crate::cancel::Cancel;
use crate::config::GameSettings;
use crate::download::Downloader;
use crate::error::{Error, IoContext, Result};
use crate::install::{self, Mode, Progress};
use crate::launch::{self, LaunchCommand, LaunchOptions};
use crate::library::Library;
use crate::loaders;
use crate::meta::VersionManifest;
use crate::options::{self, GameVersion, Profile};
use crate::paths::DataDir;
use crate::playtime::{PlaceTime, SessionRecord, Tracker};
use crate::{builds, content, servers, system};

/// Сколько последних строк вывода держать — для сообщения о краше.
const TAIL_LINES: usize = 200;
/// Как часто смотреть на окно игры, пока оно не появилось и потом.
const POLL_STARTING: Duration = Duration::from_millis(500);
const POLL_PLAYING: Duration = Duration::from_secs(2);

/// Память для сборки: своя, общая или «авто».
pub fn memory_mb(instance: &Instance, game: &GameSettings) -> u32 {
    instance
        .settings
        .memory_mb
        .or(game.memory_mb)
        .unwrap_or_else(|| system::auto_memory_mb(system::total_ram_mb()))
}

/// Готовый к запуску набор: команда и что нужно знать о версии.
pub struct Launch {
    pub cmd: LaunchCommand,
    pub version: GameVersion,
}

/// Подготовить запуск: аккаунт по умолчанию, файлы версии, команда.
pub async fn prepare(
    data: &DataDir,
    http: &Downloader,
    instance: &Instance,
    game: &GameSettings,
    progress: Progress,
    cancel: &Cancel,
    quick_play: Option<crate::playtime::LastPlace>,
) -> Result<Launch> {
    let mut accounts = Accounts::load(data)?;
    let account = accounts.default_account().cloned().ok_or_else(|| {
        Error::Other("нет аккаунта: добавьте ник или войдите через Ely.by (справа вверху)".into())
    })?;
    // Вход и проверка файлов идут одновременно: оба ждут сеть или диск.
    // Загрузчик модов ставится один раз при первом запуске сборки.
    let version = async {
        let id = loaders::ensure(
            data,
            http,
            instance.loader,
            &instance.version,
            instance.loader_version.as_deref(),
            progress.clone(),
            cancel,
        )
        .await?;
        install::prepare(data, http, &id, Mode::Launch, progress).await
    };
    let (session, prepared) = cancel
        .guard(async {
            Ok(tokio::join!(
                accounts.session(data, http, &account.id),
                version
            ))
        })
        .await?;
    let (session, prepared) = (session?, prepared?);
    let jvm_args = instance
        .settings
        .jvm_args
        .as_deref()
        .unwrap_or(&game.jvm_args)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let opts = LaunchOptions {
        game_dir: data.instance(&instance.id),
        memory_mb: memory_mb(instance, game),
        console: false,
        extra_jvm_args: jvm_args,
        quick_play,
    };
    let cmd = launch::build(data, &prepared, &session, &opts)?;
    let version = GameVersion {
        release: prepared.version.release_time.clone(),
        data_version: options::data_version(&data.client_jar(&prepared.version.id)),
    };
    Ok(Launch { cmd, version })
}

/// Итог «Проверить файлы».
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Checked {
    /// Сколько файлов игры, библиотек, ассетов и Java скачано заново.
    pub fixed: usize,
    /// Сколько модов из каталога не совпадают с ним (их — на вкладке «Моды»).
    pub mods_changed: usize,
}

/// «Проверить файлы»: хеш каждого файла версии, библиотек, ассетов и Java
/// пересчитывается заново, повреждённые и пропавшие скачиваются. Обычный запуск
/// сверяет файлы по размеру и дате и не видит порчу, при которой они не
/// меняются (так диск однажды испортил файлы Java). Моды тоже пересчитываются.
pub async fn verify(
    data: &DataDir,
    http: &Downloader,
    instance: &Instance,
    progress: Progress,
) -> Result<Checked> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let fixed = Arc::new(AtomicUsize::new(0));
    let counting: Progress = {
        let fixed = fixed.clone();
        Arc::new(move |event| {
            if let install::Event::Downloading { files, .. } = event {
                fixed.fetch_add(files, Ordering::Relaxed);
            }
            progress(event);
        })
    };
    let id = loaders::ensure(
        data,
        http,
        instance.loader,
        &instance.version,
        instance.loader_version.as_deref(),
        counting.clone(),
        &crate::cancel::Cancel::default(),
    )
    .await?;
    install::prepare(data, http, &id, Mode::Verify, counting).await?;

    let (d, build) = (data.clone(), instance.id.clone());
    tokio::task::spawn_blocking(move || crate::mods::rehash(&d, &build))
        .await
        .map_err(|e| Error::Other(format!("проверка модов прервана: {e}")))??;
    let report = crate::mods::report(http, data, &instance.id, false).await?;
    let mods_changed = report
        .issues
        .iter()
        .filter(|i| matches!(i.action, Some(crate::mods::Action::Redownload { .. })))
        .count();
    Ok(Checked {
        fixed: fixed.load(Ordering::Relaxed),
        mods_changed,
    })
}

/// Перед запуском: общие настройки игры, общий список серверов, ресурспаки и
/// шейдеры из библиотеки (раздел 5.18 ТЗ). Ошибка здесь игру не останавливает —
/// окно покажет её после игры.
pub fn before_launch(data: &DataDir, instance: &Instance, version: &GameVersion) -> Result<()> {
    let game_dir = data.instance(&instance.id);
    if !instance.settings.own_options {
        let mut profile = Profile::load(data)?;
        // Самый первый запуск после обновления: профилем становятся настройки
        // этой сборки — с ними игрок и играл.
        if profile.is_empty() && game_dir.join("options.txt").exists() {
            options::import(&mut profile, &game_dir, version)?;
            profile.seeded_from = Some(instance.name.clone());
            profile.save(data)?;
        }
        options::apply(&profile, &game_dir, version)?;
    }
    if !instance.settings.own_servers {
        servers::before_launch(data, &game_dir)?;
    }
    Library::new(data).link_into(&instance.id, &game_dir)
}

/// После игры: забрать изменённые настройки и список серверов в общие.
pub fn after_exit(data: &DataDir, instance: &Instance, version: &GameVersion) -> Result<()> {
    let game_dir = data.instance(&instance.id);
    if !instance.settings.own_options {
        let mut profile = Profile::load(data)?;
        options::import(&mut profile, &game_dir, version)?;
        profile.save(data)?;
    }
    if !instance.settings.own_servers {
        servers::after_exit(data, &game_dir)?;
    }
    Ok(())
}

/// Мир, который открывали в более новой версии, чем запускаемая.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RiskyWorld {
    pub folder: String,
    pub name: String,
    /// В какой версии открывали последним.
    pub version: String,
}

/// Миры «Быстрой игры», которым опасна версия `target`: мир из новой версии
/// в старой может испортиться (пропадут чанки или предметы).
pub fn risky_worlds(game_dir: &Path, target: &str, manifest: &VersionManifest) -> Vec<RiskyWorld> {
    let released = |id: &str| {
        manifest
            .versions
            .iter()
            .find(|v| v.id == id)
            .map(|v| v.release_time.clone())
    };
    let Some(target_time) = released(target) else {
        return Vec::new();
    };
    let Ok(read) = std::fs::read_dir(game_dir.join("saves")) else {
        return Vec::new();
    };
    let mut out: Vec<RiskyWorld> = read
        .flatten()
        .filter_map(|e| {
            let info = content::level_info(&e.path().join("level.dat"))?;
            let version = info.version?;
            if released(&version)? <= target_time {
                return None;
            }
            let folder = e.file_name().into_string().ok()?;
            Some(RiskyWorld {
                name: info.name.unwrap_or_else(|| folder.clone()),
                folder,
                version,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Копии миров в `backups/` папки игры (zip) перед запуском старой версии.
pub fn backup_worlds(game_dir: &Path, worlds: &[RiskyWorld]) -> Result<()> {
    let stamp = builds::now();
    for w in worlds {
        let out = game_dir
            .join("backups")
            .join(format!("{}_{}_{stamp}.zip", w.folder, w.version));
        std::fs::create_dir_all(out.parent().expect("есть папка"))
            .map_err(|e| Error::Other(e.to_string()))?;
        builds::zip_dir(&game_dir.join("saves").join(&w.folder), &out)?;
    }
    Ok(())
}

/// Что игра делает, пока её окна ещё нет, — по строке её вывода. Fabric,
/// Quilt, Forge, NeoForge и сама игра пишут узнаваемые строки; номер — порядок
/// этапа: окно показывает только шаги вперёд (строки разных загрузчиков
/// приходят вперемешку). `None` — строка ничего нового не говорит.
pub fn loading_stage(line: &str) -> Option<(u8, String)> {
    if let Some(n) = mods_count(line) {
        return Some((2, format!("Загрузка модов: {n}")));
    }
    const STAGES: &[(&str, u8, &str)] = &[
        // Fabric и Quilt: «Loading Minecraft 1.21.10 with Fabric Loader 0.19.5».
        (" with Fabric Loader", 1, "Запуск Fabric"),
        (" with Quilt Loader", 1, "Запуск Quilt"),
        ("ModLauncher running", 1, "Запуск загрузчика модов"),
        ("Forge Mod Loader version", 1, "Запуск Forge"),
        ("Found mod file", 2, "Поиск модов"),
        ("Initializing MixinExtras", 3, "Подготовка модов"),
        ("Launching target", 3, "Подготовка модов"),
        ("Datafixer optimizations", 4, "Подготовка данных игры"),
        ("Setting user:", 5, "Запуск модов и игры"),
        ("Backend library: LWJGL", 6, "Создание окна игры"),
        ("LWJGL Version:", 6, "Создание окна игры"),
        ("Reloading ResourceManager", 7, "Загрузка ресурсов"),
    ];
    STAGES
        .iter()
        .find(|(pattern, ..)| line.contains(pattern))
        .map(|&(_, order, text)| (order, text.to_string()))
}

/// Сколько модов грузит загрузчик: «Loading 162 mods:» (Fabric, Quilt),
/// «has identified 120 mods to load» (Forge 1.12.2).
fn mods_count(line: &str) -> Option<u32> {
    ["Loading ", "has identified "].iter().find_map(|prefix| {
        let rest = &line[line.find(prefix)? + prefix.len()..];
        let digits = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        rest[digits..]
            .starts_with(" mod")
            .then(|| rest[..digits].parse().ok())
            .flatten()
    })
}

/// Сколько сборка грузилась до окна игры в прошлые разы, секунд, — по нему
/// окно показывает проценты запуска. `key` — id сборки (у «Быстрой игры» с версией).
pub fn load_time(data: &DataDir, key: &str) -> Option<u64> {
    load_times(data).get(key).copied()
}

/// Запомнить, сколько сборка грузилась. Среднее с прошлым разом: первый
/// запуск после загрузки файлов или перезагрузки ПК обычно дольше.
pub fn record_load_time(data: &DataDir, key: &str, secs: u64) -> Result<()> {
    let mut times = load_times(data);
    let secs = match times.get(key) {
        Some(old) => (old + secs).div_ceil(2),
        None => secs,
    };
    times.insert(key.to_string(), secs);
    let path = data.cache().join("load-times.json");
    let dir = data.cache();
    std::fs::create_dir_all(&dir).at(&dir)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec(&times).expect("числа сериализуются"),
    )
    .at(&tmp)?;
    std::fs::rename(&tmp, &path).at(&path)
}

/// Отпечаток того, что нужно сборке: версия игры и загрузчик. Совпадает с
/// записанным при прошлом удачном старте — значит, все файлы уже скачивались,
/// и если теперь чего-то нет, файл пропал (чаще всего — его удалил антивирус).
fn ready_key(instance: &Instance) -> String {
    format!(
        "{}|{:?}|{}",
        instance.version,
        instance.loader,
        instance.loader_version.as_deref().unwrap_or("")
    )
}

fn ready_path(data: &DataDir) -> std::path::PathBuf {
    data.cache().join("launch-ready.json")
}

/// Сборка уже запускалась с этими версиями — все её файлы когда-то были на месте.
pub fn was_ready(data: &DataDir, instance: &Instance) -> bool {
    std::fs::read(ready_path(data))
        .ok()
        .and_then(|b| serde_json::from_slice::<std::collections::HashMap<String, String>>(&b).ok())
        .and_then(|m| m.get(&load_key_for(instance)).cloned())
        .is_some_and(|k| k == ready_key(instance))
}

/// Запомнить: файлы сборки на месте, игра стартовала.
pub fn mark_ready(data: &DataDir, instance: &Instance) {
    let path = ready_path(data);
    let mut m: std::collections::HashMap<String, String> = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    m.insert(load_key_for(instance), ready_key(instance));
    let _ = std::fs::create_dir_all(data.cache());
    let _ = std::fs::write(&path, serde_json::to_vec(&m).expect("строки"));
}

/// У «Быстрой игры» своя запись на каждую версию.
fn load_key_for(instance: &Instance) -> String {
    if instance.quick {
        format!("{}/{}", instance.id, instance.version)
    } else {
        instance.id.clone()
    }
}

/// Сколько раз подряд сборка падала и когда последний раз закрылась без ошибки.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Streak {
    pub crashes: u32,
    /// Секунды с 1970 года.
    #[serde(default)]
    pub last_ok: Option<u64>,
}

/// Записать итог игры: падение продлевает серию, удачная игра её обнуляет.
pub fn record_result(data: &DataDir, build: &str, crashed: bool, at: u64) -> Streak {
    let path = data.cache().join("launch-results.json");
    let mut all: std::collections::HashMap<String, Streak> = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let s = all.entry(build.to_string()).or_default();
    if crashed {
        s.crashes += 1;
    } else {
        *s = Streak {
            crashes: 0,
            last_ok: Some(at),
        };
    }
    let streak = *s;
    let _ = std::fs::create_dir_all(data.cache());
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, serde_json::to_vec(&all).expect("числа сериализуются")).is_ok()
    {
        let _ = std::fs::rename(&tmp, &path);
    }
    streak
}

fn load_times(data: &DataDir) -> std::collections::HashMap<String, u64> {
    std::fs::read(data.cache().join("load-times.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Запущенная игра.
pub struct Game {
    child: Child,
    game_dir: PathBuf,
    /// Секунды с 1970 года.
    pub start: u64,
}

/// Чем закончилась игра.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// Код выхода; `None` — процесс завершён снаружи.
    pub code: Option<i32>,
    pub start: u64,
    pub end: u64,
    pub places: Vec<PlaceTime>,
    /// Последние строки вывода — для сообщения о краше.
    pub tail: Vec<String>,
    /// Окно игры успело появиться; нет — игра упала ещё при загрузке.
    pub had_window: bool,
    /// Где игрок был последним (мир или сервер).
    pub last_place: Option<crate::playtime::LastPlace>,
}

impl Outcome {
    /// Игра закрылась с ошибкой. Версии до 1.6 при закрытии окна крестиком
    /// пишут «Someone is closing me!» и выходят с кодом 1 — это не ошибка.
    pub fn crashed(&self) -> bool {
        self.code != Some(0)
            && !self
                .tail
                .iter()
                .any(|l| l.contains("Someone is closing me!"))
    }

    pub fn record(&self, instance: &Instance) -> SessionRecord {
        SessionRecord {
            build: instance.id.clone(),
            build_name: instance.name.clone(),
            version: instance.version.clone(),
            start: self.start,
            end: self.end,
            places: self.places.clone(),
        }
    }
}

pub fn spawn(cmd: &LaunchCommand) -> Result<Game> {
    let mut command = Command::new(&cmd.program);
    command
        .args(&cmd.args)
        .current_dir(&cmd.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Без чёрного окна консоли, даже если выбрана java.exe.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let child = command
        .spawn()
        .map_err(|e| Error::Other(format!("не удалось запустить Java: {e}")))?;
    Ok(Game {
        child,
        game_dir: cmd.cwd.clone(),
        start: unix_now(),
    })
}

impl Game {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Ждать закрытия игры. `on_window` вызывается один раз — когда у игры
    /// появилось окно (тогда лаунчер и прячет своё). До этого `on_stage`
    /// получает этапы загрузки ([`loading_stage`]).
    pub fn wait(
        mut self,
        mut on_window: impl FnMut(),
        mut on_stage: impl FnMut(String),
    ) -> Result<Outcome> {
        let (tx, rx) = mpsc::channel::<String>();
        // Вывод читаем постоянно: если канал переполнится, игра встанет.
        [
            self.child
                .stdout
                .take()
                .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
            self.child
                .stderr
                .take()
                .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        .for_each(|stream| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut reader = std::io::BufReader::new(stream);
                let mut buf = Vec::new();
                while reader.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
                    let line = String::from_utf8_lossy(&buf).trim_end().to_string();
                    buf.clear();
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        });
        drop(tx);

        let pid = self.child.id();
        let started = Instant::now();
        let secs = || started.elapsed().as_secs();
        let mut tracker = Tracker::new();
        let mut tail: VecDeque<String> = VecDeque::with_capacity(TAIL_LINES);
        let mut had_window = false;
        let mut stage = 0;
        let mut polled = Instant::now();
        let game_dir = self.game_dir.clone();
        let mut on_line = |tracker: &mut Tracker, line: String| {
            tracker.line(secs(), &line, || {
                content::current_world(&game_dir)
                    .unwrap_or_else(|| (String::new(), "Одиночный мир".into()))
            });
            if tail.len() == TAIL_LINES {
                tail.pop_front();
            }
            tail.push_back(line);
        };

        let status = loop {
            let poll = if had_window {
                POLL_PLAYING
            } else {
                POLL_STARTING
            };
            match rx.recv_timeout(poll) {
                Ok(line) => {
                    if !had_window
                        && let Some((order, text)) = loading_stage(&line)
                        && order >= stage
                    {
                        stage = order;
                        on_stage(text);
                    }
                    on_line(&mut tracker, line)
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                // Вывод закрыт — игра завершается, дождёмся её.
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break self.child.wait().map_err(|e| Error::Other(e.to_string()))?;
                }
            }
            if polled.elapsed() < poll {
                continue;
            }
            polled = Instant::now();
            if let Some(title) = system::window_title(pid) {
                if !had_window {
                    had_window = true;
                    on_window();
                }
                tracker.title(secs(), &title);
            }
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|e| Error::Other(e.to_string()))?
            {
                break status;
            }
        };
        let end = secs();
        // Дочитываем то, что игра успела написать перед закрытием. Не ждём
        // дольше полсекунды: вывод могли унаследовать процессы, запущенные игрой.
        while let Ok(line) = rx.recv_timeout(Duration::from_millis(500)) {
            on_line(&mut tracker, line);
        }
        Ok(Outcome {
            code: status.code(),
            start: self.start,
            end: self.start + end,
            last_place: tracker.last_place(),
            places: tracker.finish(end),
            tail: tail.into(),
            had_window,
        })
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(code: Option<i32>, tail: &[&str]) -> Outcome {
        Outcome {
            code,
            start: 0,
            end: 1,
            places: Vec::new(),
            tail: tail.iter().map(|s| s.to_string()).collect(),
            had_window: true,
            last_place: None,
        }
    }

    /// Быстрая игра: мир из 1.21.10 под угрозой при запуске 1.21.1, мир из
    /// 1.12.2 и мир без версии (до 1.9) — нет.
    #[test]
    fn worlds_from_newer_versions_are_risky() {
        use crate::nbt::{Tag, write_file};
        let game = tempfile::tempdir().unwrap();
        let world = |folder: &str, version: Option<&str>| {
            let mut data = vec![("LevelName".into(), Tag::String(format!("Мир {folder}")))];
            if let Some(v) = version {
                data.push((
                    "Version".into(),
                    Tag::Compound(vec![("Name".into(), Tag::String(v.into()))]),
                ));
            }
            let dir = game.path().join("saves").join(folder);
            std::fs::create_dir_all(&dir).unwrap();
            write_file(
                &dir.join("level.dat"),
                &Tag::Compound(vec![("Data".into(), Tag::Compound(data))]),
            )
            .unwrap();
        };
        world("new", Some("1.21.10"));
        world("old", Some("1.12.2"));
        world("ancient", None);
        let manifest: VersionManifest = serde_json::from_str(
            r#"{"latest":{"release":"1.21.10","snapshot":"1.21.10"},"versions":[
              {"id":"1.21.10","type":"release","url":"u","sha1":"s","time":"t","releaseTime":"2025-10-07T09:00:00+00:00"},
              {"id":"1.21.1","type":"release","url":"u","sha1":"s","time":"t","releaseTime":"2024-08-08T11:51:42+00:00"},
              {"id":"1.12.2","type":"release","url":"u","sha1":"s","time":"t","releaseTime":"2017-09-18T08:39:46+00:00"}]}"#,
        )
        .unwrap();
        let risky = risky_worlds(game.path(), "1.21.1", &manifest);
        assert_eq!(
            risky,
            vec![RiskyWorld {
                folder: "new".into(),
                name: "Мир new".into(),
                version: "1.21.10".into()
            }]
        );
        assert!(risky_worlds(game.path(), "1.21.10", &manifest).is_empty());

        backup_worlds(game.path(), &risky).unwrap();
        let backups: Vec<_> = std::fs::read_dir(game.path().join("backups"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(backups.len(), 1);
        assert!(
            backups[0]
                .file_name()
                .to_string_lossy()
                .starts_with("new_1.21.10_")
        );
    }

    #[test]
    fn loading_stages_from_game_output() {
        // Настоящие строки из логов сборок автора и тестовой NeoForge.
        let stage = |l: &str| loading_stage(l).map(|(_, t)| t);
        assert_eq!(
            stage("[13:54:23] [main/INFO]: Loading Minecraft 1.21.10 with Fabric Loader 0.19.5")
                .as_deref(),
            Some("Запуск Fabric")
        );
        assert_eq!(
            stage("[13:54:24] [main/INFO]: Loading 162 mods:").as_deref(),
            Some("Загрузка модов: 162")
        );
        assert_eq!(
            stage("Forge Mod Loader has identified 120 mods to load").as_deref(),
            Some("Загрузка модов: 120")
        );
        assert_eq!(
            stage("[main/INFO]: ModLauncher running: args [--username, Tester]").as_deref(),
            Some("Запуск загрузчика модов")
        );
        assert_eq!(
            stage("[Render thread/INFO]: Setting user: Majorzxc").as_deref(),
            Some("Запуск модов и игры")
        );
        assert_eq!(
            stage("Backend library: LWJGL version 3.3.3-snapshot").as_deref(),
            Some("Создание окна игры")
        );
        assert_eq!(
            stage("[main/INFO]: Loading ImmediateWindowProvider fmlearlywindow"),
            None
        );
        assert_eq!(stage("Loading 3 resource packs"), None);
        assert_eq!(stage("[Render thread/INFO]: Loaded 110 languages"), None);
    }

    #[test]
    fn remembers_ready_builds() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        let mut i: Instance = serde_json::from_value(serde_json::json!({
            "id": "b", "name": "b", "version": "1.21.1", "loader": "fabric",
            "loader_version": "0.16.10", "created": 0
        }))
        .unwrap();
        assert!(!was_ready(&data, &i));
        mark_ready(&data, &i);
        assert!(was_ready(&data, &i));
        // Сменили загрузчик — новые файлы качать нормально, это не «пропажа».
        i.loader_version = Some("0.17.0".into());
        assert!(!was_ready(&data, &i));
    }

    #[test]
    fn crash_streak() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        assert_eq!(record_result(&data, "a", true, 10).crashes, 1);
        assert_eq!(record_result(&data, "a", true, 20).crashes, 2);
        assert_eq!(record_result(&data, "b", true, 20).crashes, 1);
        let ok = record_result(&data, "a", false, 30);
        assert_eq!(
            ok,
            Streak {
                crashes: 0,
                last_ok: Some(30)
            }
        );
        let again = record_result(&data, "a", true, 40);
        assert_eq!(
            again,
            Streak {
                crashes: 1,
                last_ok: Some(30)
            }
        );
    }

    #[test]
    fn load_time_is_averaged() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        assert_eq!(load_time(&data, "a"), None);
        record_load_time(&data, "a", 60).unwrap();
        assert_eq!(load_time(&data, "a"), Some(60));
        record_load_time(&data, "a", 91).unwrap();
        assert_eq!(load_time(&data, "a"), Some(76));
        assert_eq!(load_time(&data, "b"), None);
    }

    #[test]
    fn closing_old_versions_is_not_a_crash() {
        assert!(!outcome(Some(0), &[]).crashed());
        assert!(outcome(Some(1), &["java.lang.OutOfMemoryError"]).crashed());
        assert!(outcome(None, &[]).crashed());
        // Настоящий хвост 1.5.2 после закрытия окна (10.10.2026).
        assert!(
            !outcome(
                Some(1),
                &[
                    "Someone is closing me!",
                    "AL lib: (EE) alc_cleanup: 1 device not closed"
                ]
            )
            .crashed()
        );
    }
}
