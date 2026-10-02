//! Аккаунты. На этапе 1 — только офлайн-ник; Ely.by появится на этапе 2.

use md5::{Digest, Md5};

use crate::error::{Error, Result};
use crate::verify::hex;

#[derive(Debug, Clone)]
pub struct OfflineAccount {
    pub name: String,
    /// UUID без дефисов — так его ждут все версии игры.
    pub uuid: String,
}

impl OfflineAccount {
    pub fn new(name: &str) -> Result<Self> {
        let valid = (3..=16).contains(&name.len())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            return Err(Error::BadNickname(name.to_string()));
        }
        Ok(Self {
            name: name.to_string(),
            uuid: offline_uuid(name).replace('-', ""),
        })
    }
}

/// UUID офлайн-игрока — тот же, что вычисляет сам сервер Minecraft в офлайн-режиме
/// (`UUID.nameUUIDFromBytes("OfflinePlayer:" + ник)`, версия 3). Поэтому вещи
/// и прогресс на офлайн-серверах привязываются к нику одинаково во всех лаунчерах.
pub fn offline_uuid(name: &str) -> String {
    let mut b: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex(&b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_minecraft_offline_uuid() {
        // Общеизвестное значение: офлайн-UUID ника Notch.
        assert_eq!(
            offline_uuid("Notch"),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
    }

    #[test]
    fn validates_nickname() {
        assert!(OfflineAccount::new("Majorzxc").is_ok());
        assert!(OfflineAccount::new("ab").is_err());
        assert!(OfflineAccount::new("Мажор").is_err());
        assert!(OfflineAccount::new("a_very_long_nickname_123").is_err());
    }
}
