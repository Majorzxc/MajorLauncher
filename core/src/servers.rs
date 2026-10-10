//! Общий список серверов (разделы 5.10 и 5.18 ТЗ): один `servers.dat` на все
//! сборки. Перед запуском он кладётся в сборку, после выхода забирается
//! обратно — добавил сервер в одной сборке, он есть во всех.

use std::path::{Path, PathBuf};

use crate::error::{IoContext, Result};
use crate::nbt::{self, Tag};
use crate::paths::DataDir;

/// Сборка уже отдала свои серверы в общий список (делается один раз).
const MERGED: &str = ".majorlauncher-servers";

fn shared(data: &DataDir) -> PathBuf {
    data.root().join("servers.dat")
}

pub fn before_launch(data: &DataDir, game_dir: &Path) -> Result<()> {
    let shared = shared(data);
    let local = game_dir.join("servers.dat");
    let mark = game_dir.join(MERGED);
    if !mark.exists() {
        // Первый запуск сборки с общим списком: её серверы добавляются в
        // общий, а не пропадают под ним.
        if let Some(own) = nbt::read_file(&local) {
            let merged = merge(nbt::read_file(&shared), own);
            nbt::write_file(&shared, &merged).at(&shared)?;
        }
        std::fs::create_dir_all(game_dir).at(game_dir)?;
        std::fs::write(&mark, b"").at(&mark)?;
    }
    if shared.exists() {
        std::fs::copy(&shared, &local).at(&local)?;
    }
    Ok(())
}

/// После игры: список сборки становится общим, если игрок что-то поменял.
pub fn after_exit(data: &DataDir, game_dir: &Path) -> Result<()> {
    let shared = shared(data);
    let local = game_dir.join("servers.dat");
    let Ok(bytes) = std::fs::read(&local) else {
        return Ok(());
    };
    // Битый файл (игру убили во время записи) общий список не портит.
    if nbt::parse(&bytes).is_none() || std::fs::read(&shared).is_ok_and(|old| old == bytes) {
        return Ok(());
    }
    std::fs::write(&shared, bytes).at(&shared)
}

/// Серверы `own`, которых нет в `base` (по адресу), дописываются в конец.
fn merge(base: Option<Tag>, own: Tag) -> Tag {
    let mut servers: Vec<Tag> = base
        .as_ref()
        .and_then(|t| t.get("servers"))
        .and_then(Tag::as_list)
        .map(<[Tag]>::to_vec)
        .unwrap_or_default();
    let ip = |t: &Tag| t.get("ip").and_then(Tag::as_str).map(str::to_lowercase);
    for server in own
        .get("servers")
        .and_then(Tag::as_list)
        .unwrap_or_default()
    {
        if !servers.iter().any(|s| ip(s) == ip(server)) {
            servers.push(server.clone());
        }
    }
    Tag::Compound(vec![("servers".into(), Tag::List(10, servers))])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(servers: &[(&str, &str)]) -> Tag {
        Tag::Compound(vec![(
            "servers".into(),
            Tag::List(
                10,
                servers
                    .iter()
                    .map(|(name, ip)| {
                        Tag::Compound(vec![
                            ("name".into(), Tag::String(name.to_string())),
                            ("ip".into(), Tag::String(ip.to_string())),
                        ])
                    })
                    .collect(),
            ),
        )])
    }

    fn ips(path: &Path) -> Vec<String> {
        nbt::read_file(path)
            .unwrap()
            .get("servers")
            .unwrap()
            .as_list()
            .unwrap()
            .iter()
            .map(|s| s.get("ip").unwrap().as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn builds_share_one_list() {
        let root = tempfile::tempdir().unwrap();
        let data = DataDir::new(root.path());
        let a = root.path().join("a");
        let b = root.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        nbt::write_file(&a.join("servers.dat"), &list(&[("A", "a.example.org")])).unwrap();
        nbt::write_file(
            &b.join("servers.dat"),
            &list(&[("B", "b.example.org"), ("A2", "A.example.org")]),
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
        nbt::write_file(&b.join("servers.dat"), &list(&[("B", "b.example.org")])).unwrap();
        after_exit(&data, &b).unwrap();
        before_launch(&data, &a).unwrap();
        assert_eq!(ips(&a.join("servers.dat")), vec!["b.example.org"]);

        // Битый файл после убитой игры общий список не трогает.
        std::fs::write(b.join("servers.dat"), b"\x0a\x00").unwrap();
        after_exit(&data, &b).unwrap();
        assert_eq!(ips(&root.path().join("servers.dat")), vec!["b.example.org"]);
    }
}
