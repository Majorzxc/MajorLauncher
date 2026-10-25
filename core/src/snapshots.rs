//! Снапшоты сборки и откат (раздел 5.5 ТЗ). Перед каждым изменением сборки —
//! установка, обновление и удаление модов, смена загрузчика — лаунчер
//! запоминает её состояние: версию игры и загрузчика, папку модов и настройки
//! модов (`config/`). Откат возвращает всё это одним действием.
//!
//! Снапшоты лежат в `snapshots/<сборка>/<id>/`, вне папки игры. Моды
//! хранятся жёсткими ссылками — места почти не занимают: файл на диске один,
//! пока его держит сборка или библиотека. Файлы мода никто не переписывает
//! на месте (новая версия — новый файл), поэтому ссылка надёжно хранит
//! старое содержимое. Настройки модов игра переписывает прямо в файлах —
//! их снапшот копирует.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::builds::{Builds, Instance, Loader, now};
use crate::content::Folder;
use crate::error::{Error, IoContext, Result, json};
use crate::paths::DataDir;

/// Сколько автоматических снапшотов держать на сборку; сделанные вручную
/// остаются, пока их не удалят.
pub const KEEP: usize = 20;
/// Файлы настроек больше этого не копируются: это кэши модов, а не настройки.
const CONFIG_FILE_MAX: u64 = 2 * 1024 * 1024;
const INFO: &str = "snapshot.json";
const BEFORE_ROLLBACK: &str = "перед откатом";

/// Файл в снапшоте: путь от папки модов или настроек, размер и дата.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapFile {
    pub path: String,
    pub size: u64,
    /// Дата изменения, нс с 1970 года.
    pub mtime: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    /// Секунды с 1970 года.
    pub created: u64,
    /// Перед чем сделан: «перед обновлением модов».
    pub reason: String,
    /// Сделан игроком: такие не удаляются сами.
    #[serde(default)]
    pub manual: bool,
    pub version: String,
    pub loader: Loader,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub pinned_mods: Vec<String>,
    pub mods: Vec<SnapFile>,
    pub config: Vec<SnapFile>,
    /// Слишком большие файлы в `config/` — не скопированы, откат их не трогает.
    #[serde(default)]
    pub config_skipped: Vec<String>,
    /// Отпечаток состояния: тот же — значит, с тех пор ничего не менялось.
    pub fingerprint: String,
}

/// Снапшот для окна: чем отличается от сборки сейчас.
#[derive(Debug, Clone, Serialize)]
pub struct Info {
    pub id: String,
    pub created: u64,
    pub reason: String,
    pub manual: bool,
    pub version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub mods: usize,
    /// Модов (файлов) сейчас в сборке, которых в снапшоте нет, — уйдут при откате.
    pub added: Vec<String>,
    /// Модов в снапшоте, которых сейчас нет, — вернутся при откате.
    pub removed: Vec<String>,
    /// Сколько места снапшот занимает сам (файлы, которые держит только он), байт.
    pub size: u64,
    /// Совпадает со сборкой сейчас.
    pub current: bool,
}

fn root(data: &DataDir, build: &str) -> Result<PathBuf> {
    // Только имя папки: из окна не должно прийти «../».
    if build.is_empty() || build.contains(['/', '\\']) || build.contains("..") {
        return Err(Error::BuildNotFound(build.into()));
    }
    Ok(data.root().join("snapshots").join(build))
}

fn dir(data: &DataDir, build: &str, id: &str) -> Result<PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(Error::Other(format!("снапшот «{id}» не найден")));
    }
    Ok(root(data, build)?.join(id))
}

/// Что сейчас в сборке: моды, настройки модов и отпечаток всего вместе.
struct State {
    mods: Vec<SnapFile>,
    config: Vec<SnapFile>,
    skipped: Vec<String>,
    fingerprint: String,
}

fn mtime(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as u64)
}

/// Файлы папки (без вложенных) или всего дерева, пути через «/».
fn walk(dir: &Path, deep: bool) -> Result<Vec<SnapFile>> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let here = dir.join(&rel);
        let entries = match std::fs::read_dir(&here) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).at(&here),
        };
        for entry in entries.flatten() {
            let path = rel.join(entry.file_name());
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                if deep {
                    stack.push(path);
                }
                continue;
            }
            out.push(SnapFile {
                path: path.to_string_lossy().replace('\\', "/"),
                size: meta.len(),
                mtime: mtime(&meta),
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn state(game_dir: &Path, instance: &Instance) -> Result<State> {
    let mods = walk(&game_dir.join(Folder::Mods.dir()), false)?;
    let (config, skipped): (Vec<_>, Vec<_>) = walk(&game_dir.join("config"), true)?
        .into_iter()
        .partition(|f| f.size <= CONFIG_FILE_MAX);
    let mut h = Sha1::new();
    h.update(format!(
        "{}|{:?}|{:?}|{:?}\n",
        instance.version, instance.loader, instance.loader_version, instance.pinned_mods
    ));
    for (kind, list) in [("mods", &mods), ("config", &config)] {
        for f in list {
            h.update(format!("{kind}/{}\t{}\t{}\n", f.path, f.size, f.mtime));
        }
    }
    Ok(State {
        mods,
        config,
        skipped: skipped.into_iter().map(|f| f.path).collect(),
        fingerprint: crate::verify::hex(&h.finalize()),
    })
}

/// Все снапшоты сборки, новые первыми.
fn load_all(data: &DataDir, build: &str) -> Result<Vec<Snapshot>> {
    let root = root(data, build)?;
    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).at(&root),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path().join(INFO);
        // Недописанный снапшот (оборвался) — без описания, его не видно.
        if let Ok(bytes) = std::fs::read(&path)
            && let Ok(s) = json::<Snapshot>(&bytes, "описание снапшота")
        {
            out.push(s);
        }
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(out)
}

/// Что предложить откатить, если игра падает раз за разом.
#[derive(Debug, Clone, Serialize)]
pub struct Hint {
    pub id: String,
    pub reason: String,
    pub created: u64,
}

/// Снапшот, к которому стоит откатиться, если сборка падает: последний,
/// сделанный после последнего удачного запуска (`since`) и отличный от
/// сборки сейчас. Изменений после удачной игры не было — откатывать нечего.
pub fn rollback_hint(data: &DataDir, build: &str, since: Option<u64>) -> Option<Hint> {
    let last = latest(data, build)?;
    // Последнее изменение — сам откат: предлагать «откатить откат» не будем.
    if last.reason == BEFORE_ROLLBACK || since.is_some_and(|t| last.created < t) {
        return None;
    }
    let instance = Builds::new(data.clone()).get(build).ok()?;
    let now = state(&data.instance(build), &instance).ok()?;
    (now.fingerprint != last.fingerprint).then_some(Hint {
        id: last.id,
        reason: last.reason,
        created: last.created,
    })
}

/// Последний снапшот сборки.
pub fn latest(data: &DataDir, build: &str) -> Option<Snapshot> {
    load_all(data, build).ok()?.into_iter().next()
}

/// Сохранить состояние сборки. Если с прошлого снапшота ничего не менялось,
/// новый не делается — возвращается прошлый (сделанный вручную — помечается
/// ручным, чтобы не удалился сам).
pub fn take(data: &DataDir, build: &str, reason: &str, manual: bool) -> Result<Snapshot> {
    let instance = Builds::new(data.clone()).get(build)?;
    let game_dir = data.instance(build);
    let st = state(&game_dir, &instance)?;
    if let Some(mut last) = latest(data, build)
        && last.fingerprint == st.fingerprint
    {
        if manual && !last.manual {
            last.manual = true;
            last.reason = reason.to_string();
            write_info(&dir(data, build, &last.id)?, &last)?;
        }
        return Ok(last);
    }

    let root = root(data, build)?;
    let mut id = (now_ms()).to_string();
    while root.join(&id).exists() {
        id = (id.parse::<u64>().unwrap_or(0) + 1).to_string();
    }
    let target = root.join(&id);
    let tmp = root.join(format!("{id}.tmp"));
    let _ = std::fs::remove_dir_all(&tmp);
    let made = (|| -> Result<()> {
        let mods = tmp.join("mods");
        std::fs::create_dir_all(&mods).at(&mods)?;
        for f in &st.mods {
            link_or_copy(
                &game_dir.join(Folder::Mods.dir()).join(&f.path),
                &mods.join(&f.path),
            )?;
        }
        for f in &st.config {
            let to = tmp.join("config").join(&f.path);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).at(parent)?;
            }
            let from = game_dir.join("config").join(&f.path);
            std::fs::copy(&from, &to).at(&from)?;
        }
        Ok(())
    })();
    if let Err(e) = made {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let snapshot = Snapshot {
        id: id.clone(),
        created: now(),
        reason: reason.to_string(),
        manual,
        version: instance.version.clone(),
        loader: instance.loader,
        loader_version: instance.loader_version.clone(),
        pinned_mods: instance.pinned_mods.clone(),
        mods: st.mods,
        config: st.config,
        config_skipped: st.skipped,
        fingerprint: st.fingerprint,
    };
    write_info(&tmp, &snapshot)?;
    std::fs::rename(&tmp, &target).at(&target)?;
    prune(data, build)?;
    Ok(snapshot)
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

fn write_info(dir: &Path, s: &Snapshot) -> Result<()> {
    let path = dir.join(INFO);
    let tmp = dir.join("snapshot.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s).expect("сериализуется")).at(&tmp)?;
    std::fs::rename(&tmp, &path).at(&path)
}

/// Жёсткая ссылка (тот же файл, без копии); на другом диске — копия.
fn link_or_copy(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).at(parent)?;
    }
    if std::fs::hard_link(from, to).is_err() {
        std::fs::copy(from, to).at(from)?;
    }
    Ok(())
}

/// Удалить лишние автоматические снапшоты, самые старые.
fn prune(data: &DataDir, build: &str) -> Result<()> {
    let auto: Vec<Snapshot> = load_all(data, build)?
        .into_iter()
        .filter(|s| !s.manual)
        .collect();
    for old in auto.iter().skip(KEEP) {
        let d = dir(data, build, &old.id)?;
        std::fs::remove_dir_all(&d).at(&d)?;
    }
    Ok(())
}

/// Снапшоты сборки для окна, новые первыми.
pub fn list(data: &DataDir, build: &str) -> Result<Vec<Info>> {
    let instance = Builds::new(data.clone()).get(build)?;
    let game_dir = data.instance(build);
    let now = state(&game_dir, &instance)?;
    let names_now: HashSet<&str> = now.mods.iter().map(|f| f.path.as_str()).collect();
    let mut out = Vec::new();
    for s in load_all(data, build)? {
        let names: HashSet<&str> = s.mods.iter().map(|f| f.path.as_str()).collect();
        let mut added: Vec<String> = names_now
            .difference(&names)
            .map(|n| n.to_string())
            .collect();
        let mut removed: Vec<String> = names
            .difference(&names_now)
            .map(|n| n.to_string())
            .collect();
        added.sort();
        removed.sort();
        out.push(Info {
            size: own_size(&dir(data, build, &s.id)?),
            current: s.fingerprint == now.fingerprint,
            id: s.id,
            created: s.created,
            reason: s.reason,
            manual: s.manual,
            version: s.version,
            loader: s.loader,
            loader_version: s.loader_version,
            mods: s.mods.len(),
            added,
            removed,
        });
    }
    Ok(out)
}

/// Сколько места занимает папка снапшота сама: файлы, у которых нет других
/// имён на диске (жёсткие ссылки на моды сборки и библиотеки — бесплатны).
fn own_size(dir: &Path) -> u64 {
    walk(dir, true)
        .unwrap_or_default()
        .iter()
        .filter(|f| crate::relocate::links(&dir.join(&f.path)).is_none_or(|n| n <= 1))
        .map(|f| f.size)
        .sum()
}

/// Откатить сборку к снапшоту. Сначала сохраняется текущее состояние
/// («перед откатом») — откат тоже можно отменить.
pub fn restore(data: &DataDir, build: &str, id: &str) -> Result<()> {
    let snap_dir = dir(data, build, id)?;
    let bytes = std::fs::read(snap_dir.join(INFO)).at(&snap_dir)?;
    let snap: Snapshot = json(&bytes, "описание снапшота")?;
    take(data, build, BEFORE_ROLLBACK, false)?;

    let builds = Builds::new(data.clone());
    builds.set_state(
        build,
        &snap.version,
        snap.loader,
        snap.loader_version.clone(),
        snap.pinned_mods.clone(),
    )?;

    // Моды: лишние и изменённые убираем как обычно (свои файлы — в
    // библиотеку), недостающие возвращаем ссылкой из снапшота.
    let game_dir = data.instance(build);
    let mods_dir = game_dir.join(Folder::Mods.dir());
    let wanted: HashMap<&str, &SnapFile> = snap.mods.iter().map(|f| (f.path.as_str(), f)).collect();
    for f in walk(&mods_dir, false)? {
        let same = wanted
            .get(f.path.as_str())
            .is_some_and(|w| w.size == f.size && w.mtime == f.mtime);
        if !same {
            crate::mods::remove(data, build, &f.path)?;
        }
    }
    for f in &snap.mods {
        let to = mods_dir.join(&f.path);
        if !to.exists() {
            link_or_copy(&snap_dir.join("mods").join(&f.path), &to)?;
        }
    }

    // Настройки модов: как в снапшоте; большие файлы, которых он не
    // копировал, не трогаем.
    let config_dir = game_dir.join("config");
    let keep: HashSet<&str> = snap
        .config
        .iter()
        .map(|f| f.path.as_str())
        .chain(snap.config_skipped.iter().map(String::as_str))
        .collect();
    for f in walk(&config_dir, true)? {
        if !keep.contains(f.path.as_str()) && f.size <= CONFIG_FILE_MAX {
            let p = config_dir.join(&f.path);
            std::fs::remove_file(&p).at(&p)?;
        }
    }
    for f in &snap.config {
        let to = config_dir.join(&f.path);
        let same = std::fs::metadata(&to).is_ok_and(|m| m.len() == f.size && mtime(&m) == f.mtime);
        if !same {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).at(parent)?;
            }
            let from = snap_dir.join("config").join(&f.path);
            std::fs::copy(&from, &to).at(&from)?;
        }
    }
    Ok(())
}

/// Удалить снапшот.
pub fn delete(data: &DataDir, build: &str, id: &str) -> Result<()> {
    let d = dir(data, build, id)?;
    if !d.join(INFO).exists() {
        return Err(Error::Other(format!("снапшот «{id}» не найден")));
    }
    std::fs::remove_dir_all(&d).at(&d)
}

/// Удалить все снапшоты сборки — когда её удалили насовсем.
pub fn remove_all(data: &DataDir, build: &str) -> Result<()> {
    let r = root(data, build)?;
    match std::fs::remove_dir_all(&r) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e).at(&r),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(data: &DataDir) -> String {
        let b = Builds::new(data.clone());
        let choice = crate::builds::LoaderChoice {
            loader: Loader::Fabric,
            version: Some("0.16.10".into()),
        };
        let i = b.create("Тест", "1.21.1", choice, None).unwrap();
        i.id
    }

    #[test]
    fn take_restore_and_dedupe() {
        let root = tempfile::tempdir().unwrap();
        let data = DataDir::new(root.path());
        let id = build(&data);
        let game = data.instance(&id);
        let mods = game.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::create_dir_all(game.join("config/sodium")).unwrap();
        std::fs::write(mods.join("a.jar"), b"mod a v1").unwrap();
        std::fs::write(mods.join("b.jar"), b"mod b").unwrap();
        std::fs::write(game.join("config/sodium/options.json"), b"{\"fps\":60}").unwrap();

        let first = take(&data, &id, "перед установкой", false).unwrap();
        assert_eq!(first.mods.len(), 2);
        // Ничего не менялось — второй снапшот не делается.
        let again = take(&data, &id, "ещё раз", false).unwrap();
        assert_eq!(again.id, first.id);

        // Изменения: обновили a, убрали b, добавили c, поменяли настройку, сменили загрузчик.
        std::fs::remove_file(mods.join("a.jar")).unwrap();
        std::fs::write(mods.join("a-v2.jar"), b"mod a v2").unwrap();
        std::fs::remove_file(mods.join("b.jar")).unwrap();
        std::fs::write(mods.join("c.jar"), b"mod c").unwrap();
        std::fs::write(game.join("config/sodium/options.json"), b"{\"fps\":144}").unwrap();
        std::fs::write(game.join("config/new.toml"), b"x=1").unwrap();
        Builds::new(data.clone())
            .set_state(&id, "1.21.4", Loader::Fabric, Some("0.17.0".into()), vec![])
            .unwrap();

        let infos = list(&data, &id).unwrap();
        assert_eq!(infos.len(), 1);
        assert!(!infos[0].current);
        assert_eq!(infos[0].added, ["a-v2.jar", "c.jar"]);
        assert_eq!(infos[0].removed, ["a.jar", "b.jar"]);
        // Игра падает после изменений — есть что откатить; удачная игра
        // после снапшота — откатывать нечего.
        assert_eq!(rollback_hint(&data, &id, None).unwrap().id, first.id);
        assert!(rollback_hint(&data, &id, Some(first.created + 1)).is_none());

        restore(&data, &id, &first.id).unwrap();
        let names: Vec<String> = walk(&mods, false)
            .unwrap()
            .into_iter()
            .map(|f| f.path)
            .collect();
        assert_eq!(names, ["a.jar", "b.jar"]);
        assert_eq!(std::fs::read(mods.join("a.jar")).unwrap(), b"mod a v1");
        assert_eq!(
            std::fs::read(game.join("config/sodium/options.json")).unwrap(),
            b"{\"fps\":60}"
        );
        assert!(!game.join("config/new.toml").exists());
        let inst = Builds::new(data.clone()).get(&id).unwrap();
        assert_eq!(inst.version, "1.21.1");
        assert_eq!(inst.loader_version.as_deref(), Some("0.16.10"));

        // Откатить к первому снапшоту: сборка уже как в нём — подсказки нет.
        assert!(rollback_hint(&data, &id, None).is_none());

        // Перед откатом сохранилось прежнее состояние — откат можно отменить.
        let infos = list(&data, &id).unwrap();
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].reason, "перед откатом");
        assert!(infos[1].current, "сборка снова как в первом снапшоте");
        restore(&data, &id, &infos[0].id).unwrap();
        assert!(mods.join("c.jar").exists());
        assert_eq!(
            std::fs::read(game.join("config/sodium/options.json")).unwrap(),
            b"{\"fps\":144}"
        );
        // Свои файлы при откате не теряются: уезжают в библиотеку.
        assert!(data.root().join("library").exists());
    }

    #[test]
    fn manual_snapshots_survive_pruning() {
        let root = tempfile::tempdir().unwrap();
        let data = DataDir::new(root.path());
        let id = build(&data);
        let mods = data.instance(&id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("0.jar"), b"0").unwrap();
        let manual = take(&data, &id, "вручную", true).unwrap();
        for i in 1..=KEEP + 3 {
            std::fs::write(mods.join(format!("{i}.jar")), b"x").unwrap();
            take(&data, &id, "авто", false).unwrap();
        }
        let all = load_all(&data, &id).unwrap();
        assert_eq!(all.len(), KEEP + 1);
        assert!(all.iter().any(|s| s.id == manual.id));
        delete(&data, &id, &manual.id).unwrap();
        assert!(delete(&data, &id, "../x").is_err());
        remove_all(&data, &id).unwrap();
        assert!(load_all(&data, &id).unwrap().is_empty());
    }
}
