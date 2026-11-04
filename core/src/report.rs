//! «Собрать логи лаунчера» (раздел 5.16 ТЗ): один zip для сообщения об
//! ошибке — журнал лаунчера, сведения о ПК, список сборок, настройки и
//! последний лог игры. Всё проходит ту же очистку, что и текст для ИИ-разбора
//! ([`crate::ai::sanitize`]): ник, UUID, имя ПК и имя пользователя в путях,
//! токены и адреса серверов убираются. Ключи ИИ и входа лежат в хранилище
//! Windows и сюда не попадают никогда.

use std::io::Write;
use std::path::Path;

use serde::Serialize;

use crate::ai::{Private, sanitize, system_private};
use crate::builds::{Builds, State};
use crate::config::Config;
use crate::error::{Error, IoContext, Result};
use crate::paths::DataDir;

/// Журнал лаунчера и прошлый журнал — в папке журналов окна.
pub const LOG: &str = "launcher.log";
pub const OLD_LOG: &str = "launcher.old.log";
/// Больше этого журнал при запуске уезжает в `launcher.old.log`.
pub const LOG_LIMIT: u64 = 512 * 1024;

/// Последний лог игры — не больше этого (хвост важнее начала).
const GAME_LOG_LIMIT: usize = 256 * 1024;

/// Что попало в архив — окно перечисляет игроку.
#[derive(Debug, Clone, Serialize)]
pub struct Collected {
    pub files: Vec<String>,
    pub size: u64,
}

/// Перед новым запуском: большой журнал — в прошлый, начинаем новый.
pub fn rotate(log_dir: &Path) {
    let log = log_dir.join(LOG);
    if std::fs::metadata(&log).is_ok_and(|m| m.len() > LOG_LIMIT) {
        let _ = std::fs::rename(&log, log_dir.join(OLD_LOG));
    }
}

/// Личное, что убрать из текстов: ники и UUID аккаунтов лаунчера, имя ПК.
fn private(data: Option<&DataDir>) -> Private {
    let mut p = system_private();
    if let Some(accounts) = data.and_then(|d| crate::account::Accounts::load(d).ok()) {
        for a in accounts.list() {
            p.push((a.name.clone(), "<ник>"));
            p.push((a.uuid.clone(), "<uuid>"));
            if let Some(login) = &a.login {
                p.push((login.clone(), "<логин>"));
            }
        }
    }
    p.retain(|(s, _)| s.len() >= 3);
    p
}

fn system_text(data: Option<&DataDir>) -> String {
    let hw = crate::health::hardware();
    let mut out = format!(
        "MajorLauncher {}\nWindows: {}\nПроцессор: {} ({} потоков)\nПамять: {} МБ, свободно {} МБ\n",
        crate::VERSION,
        windows_version(),
        hw.cpu.as_deref().unwrap_or("?"),
        hw.cores,
        hw.ram_total_mb.map_or("?".into(), |m| m.to_string()),
        hw.ram_free_mb.map_or("?".into(), |m| m.to_string()),
    );
    for g in &hw.gpus {
        out += &format!(
            "Видеокарта: {}{} · драйвер {} от {}\n",
            g.name,
            if g.discrete {
                ""
            } else {
                " (встроенная)"
            },
            g.driver.as_deref().unwrap_or("?"),
            g.driver_date.as_deref().unwrap_or("?"),
        );
    }
    if hw.laptop {
        out += &format!(
            "Ноутбук{}\n",
            if hw.power_saver {
                ", режим экономии энергии"
            } else {
                ""
            }
        );
    }
    if let Some(d) = data {
        let free = crate::datadir::free_space(d.root())
            .map_or("?".into(), |b| format!("{} МБ", b / 1024 / 1024));
        out += &format!("Папка данных: {} · свободно {free}\n", d.root().display());
    }
    out
}

#[cfg(windows)]
fn windows_version() -> String {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
    else {
        return "?".into();
    };
    let get = |name: &str| key.get_value::<String, _>(name).unwrap_or_default();
    format!(
        "{} {} (сборка {})",
        get("ProductName"),
        get("DisplayVersion"),
        get("CurrentBuild")
    )
}

#[cfg(not(windows))]
fn windows_version() -> String {
    std::env::consts::OS.into()
}

/// Сборки: id, версия, загрузчик, когда запускалась — без миров и модов.
fn builds_text(data: &DataDir) -> String {
    let Ok(list) = Builds::new(data.clone()).list() else {
        return "список сборок не прочитался\n".into();
    };
    let mut out = String::new();
    for e in list {
        let b = &e.instance;
        out += &format!(
            "{} · {} · {:?} {} · модов {} · {}{}\n",
            b.id,
            b.version,
            b.loader,
            b.loader_version.as_deref().unwrap_or(""),
            e.mods.map_or("—".into(), |m| m.to_string()),
            b.last_played.map_or("не запускалась".into(), |t| format!(
                "запуск {t}"
            )),
            if e.state == State::Archived {
                " · в архиве"
            } else {
                ""
            },
        );
    }
    out
}

/// Последний лог самой недавно запущенной сборки — хвост.
fn last_game_log(data: &DataDir) -> Option<(String, String)> {
    let list = Builds::new(data.clone()).list().ok()?;
    let last = list
        .iter()
        .filter(|e| e.state == State::Active)
        .max_by_key(|e| e.instance.last_played.unwrap_or(0))?;
    let path = data
        .instance(&last.instance.id)
        .join("logs")
        .join("latest.log");
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let tail = match text.len().checked_sub(GAME_LOG_LIMIT) {
        Some(cut) => {
            let start = text
                .char_indices()
                .find(|(i, _)| *i >= cut)
                .map_or(0, |(i, _)| i);
            format!("(начало лога обрезано)\n{}", &text[start..])
        }
        None => text.into_owned(),
    };
    Some((format!("game-{}.log", last.instance.id), tail))
}

/// Собрать архив в `out`. `log_dir` — журналы лаунчера, `data` — папка
/// данных, если она есть (без неё — только журнал и сведения о ПК).
pub fn collect(
    out: &Path,
    log_dir: &Path,
    config: &Config,
    data: Option<&DataDir>,
) -> Result<Collected> {
    let private = private(data);
    let clean = |text: &str| sanitize(text, &private);
    let mut entries: Vec<(String, String)> = Vec::new();
    for name in [LOG, OLD_LOG] {
        if let Ok(bytes) = std::fs::read(log_dir.join(name)) {
            entries.push((name.into(), clean(&String::from_utf8_lossy(&bytes))));
        }
    }
    entries.push(("system.txt".into(), clean(&system_text(data))));
    let settings = serde_json::to_string_pretty(config).expect("настройки сериализуются");
    entries.push(("config.json".into(), clean(&settings)));
    if let Some(d) = data {
        entries.push(("builds.txt".into(), clean(&builds_text(d))));
        if let Some((name, log)) = last_game_log(d) {
            entries.push((name, clean(&log)));
        }
    }

    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).at(dir)?;
    }
    let file = std::fs::File::create(out).at(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, text) in &entries {
        zip.start_file(name.as_str(), options)
            .map_err(|e| Error::Other(format!("архив логов: {e}")))?;
        zip.write_all(text.as_bytes()).at(out)?;
    }
    zip.finish()
        .map_err(|e| Error::Other(format!("архив логов: {e}")))?;
    Ok(Collected {
        files: entries.into_iter().map(|(n, _)| n).collect(),
        size: std::fs::metadata(out).map_or(0, |m| m.len()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_is_collected_and_cleaned() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(
            logs.join(LOG),
            "WARN загрузка C:\\Users\\Ivan\\AppData\\x: token=abcdef123456\n",
        )
        .unwrap();
        let out = dir.path().join("out/logs.zip");
        let got = collect(&out, &logs, &Config::default(), None).unwrap();
        assert_eq!(got.files, ["launcher.log", "system.txt", "config.json"]);
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let mut log = String::new();
        std::io::Read::read_to_string(&mut zip.by_name(LOG).unwrap(), &mut log).unwrap();
        assert!(!log.contains("Ivan"), "{log}");
        assert!(!log.contains("abcdef123456"), "{log}");
    }

    #[test]
    fn rotation() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LOG), vec![b'x'; LOG_LIMIT as usize + 1]).unwrap();
        rotate(dir.path());
        assert!(!dir.path().join(LOG).exists());
        assert!(dir.path().join(OLD_LOG).exists());
        std::fs::write(dir.path().join(LOG), b"small").unwrap();
        rotate(dir.path());
        assert!(dir.path().join(LOG).exists());
    }
}
