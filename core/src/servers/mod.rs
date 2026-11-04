//! Общий список серверов (разделы 5.10 и 5.18 ТЗ): один `servers.dat` на все
//! сборки. Перед запуском он кладётся в сборку, после выхода забирается
//! обратно — добавил сервер в одной сборке, он есть во всех. Список правится
//! и из лаунчера; [`ping`] узнаёт, что сейчас на сервере.

mod dns;
pub mod motd;
pub mod ping;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::{Error, IoContext, Result};
use crate::nbt::{self, Tag};
use crate::paths::DataDir;

/// Сборка уже отдала свои серверы в общий список (делается один раз).
/// Внутри — тот список, что положили в сборку перед запуском: по нему после
/// выхода видно, что поменяла игра, а что — лаунчер, пока она шла.
const MERGED: &str = ".majorlauncher-servers";

/// Одна запись в файл за раз: окно правит список, пока опрос обновляет
/// иконки, а игра выходит и отдаёт свой.
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Общий список.
pub fn shared_path(data: &DataDir) -> PathBuf {
    data.root().join("servers.dat")
}

/// Свой список сборки (у неё выключен общий).
pub fn build_path(data: &DataDir, build: &str) -> PathBuf {
    data.instance(build).join("servers.dat")
}

/// Ресурспак сервера: как ответили игре на вопрос «Принять?».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Textures {
    /// Игра спросит при входе.
    Ask,
    Accept,
    Deny,
}

/// Сервер из списка игры.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Server {
    /// Место в файле: по нему окно правит, удаляет и переставляет.
    pub index: usize,
    pub name: String,
    /// Как в игре: `host`, `host:port` или `[IPv6]:port`.
    pub address: String,
    /// Иконка сервера — PNG 64×64 в base64, как её хранит игра.
    pub icon: Option<String>,
    pub textures: Textures,
    /// Игра запомнила сервер при прямом подключении — в её списке его нет.
    pub hidden: bool,
}

/// Изменение из окна; пустые поля остаются как были.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ServerEdit {
    pub name: Option<String>,
    pub address: Option<String>,
    pub textures: Option<Textures>,
}

/// Так игра называет сервер, если имя не ввели.
const DEFAULT_NAME: &str = "Сервер Minecraft";
const MAX_NAME: usize = 64;
/// Поле адреса в игре — до 128 символов.
const MAX_ADDRESS: usize = 128;

fn str_of<'a>(t: &'a Tag, key: &str) -> Option<&'a str> {
    t.get(key).and_then(Tag::as_str)
}

fn byte_of(t: &Tag, key: &str) -> Option<i8> {
    match t.get(key) {
        Some(Tag::Byte(b)) => Some(*b),
        _ => None,
    }
}

/// Адрес для сравнения: регистр и пробелы по краям не важны.
fn key(t: &Tag) -> Option<String> {
    str_of(t, "ip").map(|s| s.trim().to_lowercase())
}

fn set(t: &mut Tag, name: &str, value: Option<Tag>) {
    let Tag::Compound(items) = t else { return };
    let at = items.iter().position(|(k, _)| k == name);
    match (at, value) {
        (Some(i), Some(v)) => items[i].1 = v,
        (None, Some(v)) => items.push((name.into(), v)),
        (Some(i), None) => {
            items.remove(i);
        }
        (None, None) => {}
    }
}

fn to_server(index: usize, t: &Tag) -> Server {
    Server {
        index,
        name: str_of(t, "name").unwrap_or_default().to_string(),
        address: str_of(t, "ip").unwrap_or_default().to_string(),
        icon: str_of(t, "icon").map(str::to_string),
        textures: match byte_of(t, "acceptTextures") {
            Some(0) => Textures::Deny,
            Some(_) => Textures::Accept,
            None => Textures::Ask,
        },
        hidden: byte_of(t, "hidden").is_some_and(|b| b != 0),
    }
}

/// Записи файла. Нет файла — пустой список; битый — ошибка, а не пустой
/// список: иначе первая же правка затёрла бы его.
fn load(path: &Path) -> Result<Vec<Tag>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).at(path),
    };
    let root = nbt::parse(&bytes).ok_or_else(|| {
        Error::Other(format!(
            "список серверов {} повреждён — откройте его в игре или удалите файл",
            path.display()
        ))
    })?;
    Ok(entries(&root))
}

fn entries(root: &Tag) -> Vec<Tag> {
    root.get("servers")
        .and_then(Tag::as_list)
        .map(<[Tag]>::to_vec)
        .unwrap_or_default()
}

fn root_of(servers: Vec<Tag>) -> Tag {
    Tag::Compound(vec![("servers".into(), Tag::List(10, servers))])
}

fn save(path: &Path, servers: Vec<Tag>) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).at(dir)?;
    }
    nbt::write_file(path, &root_of(servers)).at(path)
}

/// Список серверов файла (и скрытые — окно само решает, показывать ли их).
pub fn list(path: &Path) -> Result<Vec<Server>> {
    let _guard = lock();
    Ok(load(path)?
        .iter()
        .enumerate()
        .map(|(i, t)| to_server(i, t))
        .collect())
}

/// Прочитать, поменять и записать под замком.
fn modify<T>(path: &Path, f: impl FnOnce(&mut Vec<Tag>) -> Result<T>) -> Result<T> {
    let _guard = lock();
    let mut servers = load(path)?;
    let out = f(&mut servers)?;
    save(path, servers)?;
    Ok(out)
}

/// Запись `index` — та самая, что видел игрок: игра могла поменять список,
/// пока окно было открыто.
fn check(servers: &[Tag], index: usize, address: &str) -> Result<()> {
    match servers.get(index).and_then(key) {
        Some(k) if k == address.trim().to_lowercase() => Ok(()),
        _ => Err(Error::Other(
            "список серверов изменился — обновите его и попробуйте снова".into(),
        )),
    }
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.chars().count() > MAX_NAME {
        return Err(Error::Other(format!(
            "название длиннее {MAX_NAME} символов"
        )));
    }
    Ok(if name.is_empty() { DEFAULT_NAME } else { name }.to_string())
}

fn clean_address(address: &str) -> Result<String> {
    let address = address.trim();
    if address.is_empty() {
        return Err(Error::Other("введите адрес сервера".into()));
    }
    if address.chars().count() > MAX_ADDRESS || ping::Address::parse(address).is_none() {
        return Err(Error::Other(format!(
            "«{address}» не похоже на адрес сервера: нужен вид play.example.org, \
             play.example.org:25565 или 1.2.3.4:25565"
        )));
    }
    Ok(address.to_string())
}

fn textures_tag(t: Textures) -> Option<Tag> {
    match t {
        Textures::Ask => None,
        Textures::Accept => Some(Tag::Byte(1)),
        Textures::Deny => Some(Tag::Byte(0)),
    }
}

/// Добавить сервер в конец списка (как кнопка «Добавить» в игре).
/// Возвращает его место.
pub fn add(path: &Path, name: &str, address: &str) -> Result<usize> {
    let name = clean_name(name)?;
    let address = clean_address(address)?;
    let k = address.to_lowercase();
    modify(path, |servers| {
        if let Some(t) = servers
            .iter()
            .find(|t| key(t).as_deref() == Some(&k) && byte_of(t, "hidden").unwrap_or(0) == 0)
        {
            return Err(Error::Other(format!(
                "этот адрес уже есть в списке: «{}»",
                str_of(t, "name").unwrap_or_default()
            )));
        }
        // Скрытая запись того же сервера (прямое подключение) становится
        // обычной — иначе игра покажет его дважды.
        servers.retain(|t| key(t).as_deref() != Some(&k));
        servers.push(Tag::Compound(vec![
            ("name".into(), Tag::String(name)),
            ("ip".into(), Tag::String(address)),
        ]));
        Ok(servers.len() - 1)
    })
}

/// Переименовать, сменить адрес или решение о ресурспаке.
pub fn edit(path: &Path, index: usize, address: &str, change: &ServerEdit) -> Result<()> {
    let name = change.name.as_deref().map(clean_name).transpose()?;
    let new_address = change.address.as_deref().map(clean_address).transpose()?;
    modify(path, |servers| {
        check(servers, index, address)?;
        if let Some(new) = &new_address {
            let k = new.to_lowercase();
            let taken = servers.iter().enumerate().any(|(i, t)| {
                i != index
                    && key(t).as_deref() == Some(&k)
                    && byte_of(t, "hidden").unwrap_or(0) == 0
            });
            if taken {
                return Err(Error::Other("этот адрес уже есть в списке".into()));
            }
        }
        let t = &mut servers[index];
        if let Some(name) = name {
            set(t, "name", Some(Tag::String(name)));
        }
        if let Some(new) = new_address {
            if key(t).as_deref() != Some(&new.to_lowercase()) {
                // Иконка была от прежнего сервера.
                set(t, "icon", None);
            }
            set(t, "ip", Some(Tag::String(new)));
        }
        if let Some(textures) = change.textures {
            set(t, "acceptTextures", textures_tag(textures));
        }
        Ok(())
    })
}

pub fn remove(path: &Path, index: usize, address: &str) -> Result<()> {
    modify(path, |servers| {
        check(servers, index, address)?;
        servers.remove(index);
        Ok(())
    })
}

/// Переставить сервер на место `to` (места — как в [`list`]).
pub fn move_to(path: &Path, index: usize, address: &str, to: usize) -> Result<()> {
    modify(path, |servers| {
        check(servers, index, address)?;
        let t = servers.remove(index);
        servers.insert(to.min(servers.len()), t);
        Ok(())
    })
}

/// Иконки из ответа серверов — как игра обновляет их при опросе. Сервер без
/// иконки теряет старую. Файл пишется, только если что-то поменялось.
pub fn update_icons(path: &Path, icons: &[(String, Option<String>)]) -> Result<bool> {
    let _guard = lock();
    let mut servers = load(path)?;
    let mut changed = false;
    for t in servers.iter_mut() {
        let Some(k) = key(t) else { continue };
        let Some((_, icon)) = icons.iter().find(|(a, _)| a.trim().to_lowercase() == k) else {
            continue;
        };
        if str_of(t, "icon") != icon.as_deref() {
            set(t, "icon", icon.clone().map(Tag::String));
            changed = true;
        }
    }
    if changed {
        save(path, servers)?;
    }
    Ok(changed)
}

pub fn before_launch(data: &DataDir, game_dir: &Path) -> Result<()> {
    let _guard = lock();
    let shared = shared_path(data);
    let local = game_dir.join("servers.dat");
    let mark = game_dir.join(MERGED);
    if !mark.exists() {
        // Первый запуск сборки с общим списком: её серверы добавляются в
        // общий, а не пропадают под ним.
        if let Some(own) = nbt::read_file(&local) {
            let merged = merge(nbt::read_file(&shared), own);
            nbt::write_file(&shared, &merged).at(&shared)?;
        }
    }
    std::fs::create_dir_all(game_dir).at(game_dir)?;
    let given = match std::fs::read(&shared) {
        Ok(bytes) => {
            std::fs::write(&local, &bytes).at(&local)?;
            bytes
        }
        Err(_) => Vec::new(),
    };
    std::fs::write(&mark, given).at(&mark)
}

/// После игры: изменения игры переходят в общий список. Если и лаунчер менял
/// его, пока игра шла, — сливаются оба: ничего не теряется.
pub fn after_exit(data: &DataDir, game_dir: &Path) -> Result<()> {
    let _guard = lock();
    let shared = shared_path(data);
    let local = game_dir.join("servers.dat");
    let Ok(bytes) = std::fs::read(&local) else {
        return Ok(());
    };
    // Битый файл (игру убили во время записи) общий список не портит.
    let Some(game) = nbt::parse(&bytes) else {
        return Ok(());
    };
    let given = std::fs::read(game_dir.join(MERGED)).unwrap_or_default();
    let now = std::fs::read(&shared).ok();
    if now.as_deref() == Some(&bytes[..]) || (!given.is_empty() && given == bytes) {
        // Игра список не меняла.
        return Ok(());
    }
    let base = nbt::parse(&given);
    let current = now.as_deref().and_then(nbt::parse);
    match (base, current) {
        // Лаунчер его тоже правил — сливаем.
        (Some(base), Some(current)) if now.as_deref() != Some(&given[..]) => {
            let out = merge3(&entries(&base), &entries(&game), entries(&current));
            nbt::write_file(&shared, &root_of(out)).at(&shared)
        }
        _ => std::fs::write(&shared, bytes).at(&shared),
    }
}

/// Серверы `own`, которых нет в `base` (по адресу), дописываются в конец.
fn merge(base: Option<Tag>, own: Tag) -> Tag {
    let mut servers = base.as_ref().map(entries).unwrap_or_default();
    for server in entries(&own) {
        if !servers.iter().any(|s| key(s) == key(&server)) {
            servers.push(server);
        }
    }
    root_of(servers)
}

/// Три списка: что дали игре (`base`), что она вернула (`game`) и что сейчас
/// в общем (`current`, его правил лаунчер). Изменения игры по каждому
/// серверу и полю ложатся поверх общего; порядок — общего списка.
fn merge3(base: &[Tag], game: &[Tag], mut current: Vec<Tag>) -> Vec<Tag> {
    let find = |list: &[Tag], k: &Option<String>| list.iter().position(|t| &key(t) == k);
    for b in base {
        let k = key(b);
        if find(game, &k).is_none() {
            current.retain(|t| key(t) != k);
        }
    }
    for g in game {
        let k = key(g);
        let old = find(base, &k).map(|i| &base[i]);
        if old == Some(g) {
            continue;
        }
        let Some(at) = find(&current, &k) else {
            current.push(g.clone());
            continue;
        };
        let (Some(old), Tag::Compound(fields)) = (old, g) else {
            current[at] = g.clone();
            continue;
        };
        for (name, value) in fields {
            if old.get(name) != Some(value) {
                set(&mut current[at], name, Some(value.clone()));
            }
        }
        if let Tag::Compound(was) = old {
            for (name, _) in was {
                if g.get(name).is_none() {
                    set(&mut current[at], name, None);
                }
            }
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list_of(servers: &[(&str, &str)]) -> Tag {
        root_of(
            servers
                .iter()
                .map(|(name, ip)| {
                    Tag::Compound(vec![
                        ("name".into(), Tag::String(name.to_string())),
                        ("ip".into(), Tag::String(ip.to_string())),
                    ])
                })
                .collect(),
        )
    }

    fn ips(path: &Path) -> Vec<String> {
        list(path).unwrap().into_iter().map(|s| s.address).collect()
    }

    fn names(path: &Path) -> Vec<String> {
        list(path).unwrap().into_iter().map(|s| s.name).collect()
    }

    #[test]
    fn builds_share_one_list() {
        let root = tempfile::tempdir().unwrap();
        let data = DataDir::new(root.path());
        let a = root.path().join("a");
        let b = root.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        nbt::write_file(&a.join("servers.dat"), &list_of(&[("A", "a.example.org")])).unwrap();
        nbt::write_file(
            &b.join("servers.dat"),
            &list_of(&[("B", "b.example.org"), ("A2", "A.example.org")]),
        )
        .unwrap();

        // Каждая сборка при первом запуске отдаёт свои серверы — ничего не теряется.
        before_launch(&data, &a).unwrap();
        before_launch(&data, &b).unwrap();
        assert_eq!(
            ips(&b.join("servers.dat")),
            vec!["a.example.org", "b.example.org"]
        );

        // В сборке «b» удалили сервер — после игры список общий для всех.
        nbt::write_file(&b.join("servers.dat"), &list_of(&[("B", "b.example.org")])).unwrap();
        after_exit(&data, &b).unwrap();
        before_launch(&data, &a).unwrap();
        assert_eq!(ips(&a.join("servers.dat")), vec!["b.example.org"]);

        // Битый файл после убитой игры общий список не трогает.
        std::fs::write(b.join("servers.dat"), b"\x0a\x00").unwrap();
        after_exit(&data, &b).unwrap();
        assert_eq!(ips(&shared_path(&data)), vec!["b.example.org"]);
    }

    #[test]
    fn launcher_and_game_edit_at_once() {
        let root = tempfile::tempdir().unwrap();
        let data = DataDir::new(root.path());
        let shared = shared_path(&data);
        let game = root.path().join("game");
        nbt::write_file(
            &shared,
            &list_of(&[("A", "a.org"), ("B", "b.org"), ("C", "c.org")]),
        )
        .unwrap();
        before_launch(&data, &game).unwrap();

        // Пока игра идёт, в лаунчере переименовали A, удалили B и добавили D.
        let a = &list(&shared).unwrap()[0];
        edit(
            &shared,
            0,
            &a.address,
            &ServerEdit {
                name: Some("Арена".into()),
                ..Default::default()
            },
        )
        .unwrap();
        remove(&shared, 1, "b.org").unwrap();
        add(&shared, "D", "d.org").unwrap();

        // А игра обновила иконку A, удалила C и добавила E.
        let mut a = list_of(&[("A", "a.org")]);
        let mut entry = entries(&a).remove(0);
        set(&mut entry, "icon", Some(Tag::String("iVBOR".into())));
        a = root_of(vec![
            entry,
            Tag::Compound(vec![
                ("name".into(), Tag::String("B".into())),
                ("ip".into(), Tag::String("b.org".into())),
            ]),
            Tag::Compound(vec![
                ("name".into(), Tag::String("E".into())),
                ("ip".into(), Tag::String("e.org".into())),
            ]),
        ]);
        nbt::write_file(&game.join("servers.dat"), &a).unwrap();
        after_exit(&data, &game).unwrap();

        let out = list(&shared).unwrap();
        let got: Vec<_> = out
            .iter()
            .map(|s| (s.name.as_str(), s.address.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![("Арена", "a.org"), ("D", "d.org"), ("E", "e.org")]
        );
        assert_eq!(out[0].icon.as_deref(), Some("iVBOR"));

        // Игра ничего не поменяла — правки лаунчера остаются как есть.
        before_launch(&data, &game).unwrap();
        move_to(&shared, 2, "e.org", 0).unwrap();
        after_exit(&data, &game).unwrap();
        assert_eq!(ips(&shared), vec!["e.org", "a.org", "d.org"]);
    }

    #[test]
    fn edits() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("servers.dat");
        assert!(list(&path).unwrap().is_empty());

        assert_eq!(add(&path, "  ", "mc.example.org").unwrap(), 0);
        assert_eq!(add(&path, "Второй 😀", "[::1]:25566").unwrap(), 1);
        assert!(add(&path, "x", "MC.example.org").is_err(), "тот же адрес");
        assert!(add(&path, "x", "bad address").is_err());
        assert!(add(&path, "x", "host:99999").is_err());
        assert_eq!(names(&path), vec![DEFAULT_NAME, "Второй 😀"]);

        // Устаревшее место — отказ, а не правка чужого сервера.
        assert!(remove(&path, 0, "[::1]:25566").is_err());
        edit(
            &path,
            1,
            "[::1]:25566",
            &ServerEdit {
                address: Some("play.example.org".into()),
                textures: Some(Textures::Deny),
                ..Default::default()
            },
        )
        .unwrap();
        let s = &list(&path).unwrap()[1];
        assert_eq!(s.address, "play.example.org");
        assert_eq!(s.textures, Textures::Deny);

        move_to(&path, 1, "play.example.org", 0).unwrap();
        assert_eq!(ips(&path), vec!["play.example.org", "mc.example.org"]);

        let icon = Some("AAAA".to_string());
        assert!(update_icons(&path, &[("MC.example.org".into(), icon.clone())]).unwrap());
        assert!(!update_icons(&path, &[("mc.example.org".into(), icon.clone())]).unwrap());
        assert_eq!(list(&path).unwrap()[1].icon, icon);

        // Скрытая запись (прямое подключение) при добавлении становится обычной.
        modify(&path, |s| {
            s.push(Tag::Compound(vec![
                ("name".into(), Tag::String("".into())),
                ("ip".into(), Tag::String("hidden.org".into())),
                ("hidden".into(), Tag::Byte(1)),
            ]));
            Ok(())
        })
        .unwrap();
        assert!(list(&path).unwrap()[2].hidden);
        add(&path, "Видимый", "hidden.org").unwrap();
        let all = list(&path).unwrap();
        assert_eq!(all.len(), 3);
        assert!(!all[2].hidden);

        std::fs::write(&path, b"garbage").unwrap();
        assert!(list(&path).is_err());
        assert!(
            add(&path, "x", "y.org").is_err(),
            "битый файл не перезаписывается"
        );
    }
}
