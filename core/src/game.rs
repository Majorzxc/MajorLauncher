//! Запуск сборки и наблюдение за игрой до её закрытия.
//!
//! Пока игра идёт, окна лаунчера нет (принцип 4 ТЗ): работает только этот
//! код — читает вывод игры, раз в пару секунд смотрит на заголовок её окна и
//! копит время по мирам и серверам ([`crate::playtime`]).

use std::collections::VecDeque;
use std::io::BufRead;
use std::path::PathBuf;
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
use crate::paths::DataDir;
use crate::playtime::{PlaceTime, SessionRecord, Tracker};
use crate::{content, system};

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

/// Подготовить запуск: аккаунт по умолчанию, файлы версии, команда.
pub async fn prepare(
    data: &DataDir,
    http: &Downloader,
    instance: &Instance,
    game: &GameSettings,
    progress: Progress,
) -> Result<LaunchCommand> {
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
    launch::build(data, &prepared, &session, &opts)
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
                content::current_world(&game_dir).unwrap_or_else(|| "Одиночный мир".into())
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
