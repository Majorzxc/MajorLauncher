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
use crate::config::GameSettings;
use crate::download::Downloader;
use crate::error::{Error, Result};
use crate::install::{self, Mode, Progress};
use crate::launch::{self, LaunchCommand, LaunchOptions};
use crate::library::Library;
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
) -> Result<Launch> {
    let mut accounts = Accounts::load(data)?;
    let account = accounts.default_account().cloned().ok_or_else(|| {
        Error::Other("нет аккаунта: добавьте ник или войдите через Ely.by (справа вверху)".into())
    })?;
    // Вход и проверка файлов идут одновременно: оба ждут сеть или диск.
    let (session, prepared) = tokio::join!(
        accounts.session(data, http, &account.id),
        install::prepare(data, http, &instance.version, Mode::Launch, progress)
    );
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
    };
    let cmd = launch::build(data, &prepared, &session, &opts)?;
    let version = GameVersion {
        release: prepared.version.release_time.clone(),
        data_version: options::data_version(&data.client_jar(&prepared.version.id)),
    };
    Ok(Launch { cmd, version })
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
    /// появилось окно (тогда лаунчер и прячет своё).
    pub fn wait(mut self, mut on_window: impl FnMut()) -> Result<Outcome> {
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
                Ok(line) => on_line(&mut tracker, line),
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
