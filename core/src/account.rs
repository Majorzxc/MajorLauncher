//! Аккаунты: офлайн-ники и Ely.by, сколько угодно, один — по умолчанию
//! (раздел 5.2 ТЗ). Вход через Microsoft отложен до после 1.0.
//!
//! Список аккаунтов — в `accounts.json` в папке данных: там только ники, UUID
//! и тип. Токены Ely.by — в хранилище Windows ([`crate::secrets`]).

use std::path::PathBuf;

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::download::Downloader;
use crate::elyby::{self, Agent};
use crate::error::{Error, IoContext, Result, json};
use crate::paths::DataDir;
use crate::secrets;
use crate::verify::hex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Offline,
    Ely,
}

impl Kind {
    /// Как показывать игроку.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Offline => "офлайн",
            Kind::Ely => "Ely.by",
        }
    }

    /// Префикс для уточнения: `offline:Steve`, `ely:Steve`.
    fn prefix(self) -> &'static str {
        match self {
            Kind::Offline => "offline",
            Kind::Ely => "ely",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Постоянный идентификатор: `offline-<ник>` или `ely-<uuid>`.
    pub id: String,
    pub kind: Kind,
    pub name: String,
    /// UUID без дефисов.
    pub uuid: String,
    /// Логин Ely.by (ник или почта), чтобы при повторном входе не спрашивать его.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AccountsFile {
    /// Токен клиента для Ely.by: один на установку лаунчера (так требует протокол).
    #[serde(default)]
    client_token: String,
    #[serde(default)]
    default: Option<String>,
    #[serde(default)]
    accounts: Vec<Account>,
}

/// Секрет аккаунта Ely.by. JSON — чтобы позже добавлять поля без миграций.
#[derive(Serialize, Deserialize)]
struct Token {
    access_token: String,
}

/// Всё, что нужно игре от аккаунта при запуске.
#[derive(Debug, Clone)]
pub struct Session {
    pub kind: Kind,
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    /// authlib-injector для Ely.by — скины и вход на серверы Ely.by.
    pub agent: Option<Agent>,
    /// Токен не удалось проверить (нет интернета): одиночная игра работает,
    /// на серверы Ely.by может не пустить.
    pub unverified: bool,
}

impl Session {
    /// Разовый офлайн-запуск по нику, без сохранения аккаунта.
    pub fn offline(nick: &str) -> Result<Self> {
        validate_nick(nick)?;
        Ok(Self {
            kind: Kind::Offline,
            name: nick.to_string(),
            uuid: offline_uuid(nick).replace('-', ""),
            // Офлайн-игре токен не нужен, но пустым его оставлять нельзя.
            access_token: "0".into(),
            agent: None,
            unverified: false,
        })
    }
}

pub struct Accounts {
    path: PathBuf,
    file: AccountsFile,
}

impl Accounts {
    pub fn load(data: &DataDir) -> Result<Self> {
        let path = data.root().join("accounts.json");
        let mut file: AccountsFile = match std::fs::read(&path) {
            Ok(bytes) => json(&bytes, "список аккаунтов")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AccountsFile::default(),
            Err(e) => return Err(e).at(&path),
        };
        let mut accounts = Self {
            path,
            file: AccountsFile::default(),
        };
        if file.client_token.is_empty() {
            file.client_token = random_hex(16)?;
            accounts.file = file;
            accounts.save()?;
        } else {
            accounts.file = file;
        }
        Ok(accounts)
    }

    pub fn save(&self) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.file).expect("список аккаунтов сериализуется");
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, bytes).at(&tmp)?;
        std::fs::rename(&tmp, &self.path).at(&self.path)
    }

    pub fn list(&self) -> &[Account] {
        &self.file.accounts
    }

    pub fn is_default(&self, account: &Account) -> bool {
        self.default_account().is_some_and(|d| d.id == account.id)
    }

    /// Аккаунт по умолчанию; если он не выбран — первый в списке.
    pub fn default_account(&self) -> Option<&Account> {
        self.file
            .default
            .as_ref()
            .and_then(|id| self.file.accounts.iter().find(|a| &a.id == id))
            .or_else(|| self.file.accounts.first())
    }

    /// Найти по нику (без учёта регистра) или с уточнением `offline:` / `ely:`.
    pub fn find(&self, reference: &str) -> Result<&Account> {
        let (kind, name) = match reference.split_once(':') {
            Some(("offline", n)) => (Some(Kind::Offline), n),
            Some(("ely", n)) => (Some(Kind::Ely), n),
            _ => (None, reference),
        };
        let mut found = self
            .file
            .accounts
            .iter()
            .filter(|a| kind.is_none_or(|k| a.kind == k) && a.name.eq_ignore_ascii_case(name));
        match (found.next(), found.next()) {
            (Some(a), None) => Ok(a),
            (Some(_), Some(_)) => Err(Error::AmbiguousAccount(name.to_string())),
            (None, _) => Err(Error::AccountNotFound(reference.to_string())),
        }
    }

    pub fn set_default(&mut self, reference: &str) -> Result<()> {
        let id = self.find(reference)?.id.clone();
        self.file.default = Some(id);
        self.save()
    }

    pub fn add_offline(&mut self, nick: &str) -> Result<Account> {
        validate_nick(nick)?;
        let id = format!("offline-{}", nick.to_lowercase());
        if self.file.accounts.iter().any(|a| a.id == id) {
            return Err(Error::AccountExists(format!(
                "{}:{nick}",
                Kind::Offline.prefix()
            )));
        }
        let account = Account {
            id,
            kind: Kind::Offline,
            name: nick.to_string(),
            uuid: offline_uuid(nick).replace('-', ""),
            login: None,
        };
        self.insert(account.clone())?;
        Ok(account)
    }

    /// Войти в Ely.by. Пароль не сохраняется — только выданный токен.
    /// Повторный вход в уже добавленный аккаунт просто обновляет токен.
    pub async fn add_ely(
        &mut self,
        http: &Downloader,
        login: &str,
        password: &str,
        totp: Option<&str>,
    ) -> Result<Account> {
        let result =
            elyby::authenticate(http, login, password, totp, &self.file.client_token).await?;
        let account = Account {
            id: format!("ely-{}", result.uuid),
            kind: Kind::Ely,
            name: result.name,
            uuid: result.uuid,
            login: Some(login.to_string()),
        };
        store_token(&account.id, &result.access_token)?;
        match self.file.accounts.iter_mut().find(|a| a.id == account.id) {
            Some(existing) => {
                *existing = account.clone();
                self.save()?;
            }
            None => self.insert(account.clone())?,
        }
        Ok(account)
    }

    /// Удалить аккаунт: токен Ely.by отзывается на сервере и стирается из хранилища.
    pub async fn remove(&mut self, http: &Downloader, reference: &str) -> Result<Account> {
        let account = self.find(reference)?.clone();
        if account.kind == Kind::Ely {
            if let Some(token) = load_token(&account.id)?
                && let Err(e) =
                    elyby::invalidate(http, &token.access_token, &self.file.client_token).await
            {
                // Не страшно: токен всё равно удаляется из лаунчера.
                tracing::warn!("не удалось отозвать токен Ely.by: {e}");
            }
            secrets::delete(&account.id)?;
        }
        self.file.accounts.retain(|a| a.id != account.id);
        if self.file.default.as_deref() == Some(account.id.as_str()) {
            self.file.default = self.file.accounts.first().map(|a| a.id.clone());
        }
        self.save()?;
        Ok(account)
    }

    /// Подготовить аккаунт к запуску. Для Ely.by проверяет токен, при
    /// необходимости обновляет его и готовит authlib-injector.
    pub async fn session(
        &mut self,
        data: &DataDir,
        http: &Downloader,
        account_id: &str,
    ) -> Result<Session> {
        let account = self
            .file
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .cloned()
            .ok_or_else(|| Error::AccountNotFound(account_id.to_string()))?;

        if account.kind == Kind::Offline {
            return Session::offline(&account.name);
        }

        let token = load_token(&account.id)?.ok_or(Error::ElySessionExpired)?;
        let (check, agent) = tokio::join!(
            elyby::validate(http, &token.access_token),
            elyby::agent(data, http)
        );
        let agent = agent?;

        let (access_token, unverified) = match check {
            Ok(true) => (token.access_token, false),
            Ok(false) => {
                let fresh =
                    elyby::refresh(http, &token.access_token, &self.file.client_token).await?;
                store_token(&account.id, &fresh.access_token)?;
                // Ник на Ely.by можно сменить — подхватываем новый.
                if fresh.name != account.name
                    && let Some(a) = self.file.accounts.iter_mut().find(|a| a.id == account.id)
                {
                    a.name = fresh.name.clone();
                    self.save()?;
                }
                (fresh.access_token, false)
            }
            Err(e) if e.is_network() => {
                tracing::warn!("не удалось проверить вход Ely.by: {e}");
                (token.access_token, true)
            }
            Err(e) => return Err(e),
        };

        let name = self
            .file
            .accounts
            .iter()
            .find(|a| a.id == account.id)
            .map_or(account.name, |a| a.name.clone());
        Ok(Session {
            kind: Kind::Ely,
            name,
            uuid: account.uuid,
            access_token,
            agent: Some(agent),
            unverified,
        })
    }

    fn insert(&mut self, account: Account) -> Result<()> {
        if self.file.accounts.is_empty() {
            self.file.default = Some(account.id.clone());
        }
        self.file.accounts.push(account);
        self.save()
    }
}

fn store_token(id: &str, access_token: &str) -> Result<()> {
    let token = Token {
        access_token: access_token.to_string(),
    };
    secrets::set(
        id,
        &serde_json::to_string(&token).expect("токен сериализуется"),
    )
}

fn load_token(id: &str) -> Result<Option<Token>> {
    match secrets::get(id)? {
        Some(s) => Ok(serde_json::from_str(&s).ok()),
        None => Ok(None),
    }
}

fn random_hex(bytes: usize) -> Result<String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| Error::Other(format!("нет случайных чисел: {e}")))?;
    Ok(hex(&buf))
}

/// Ник: 3–16 символов, латиница, цифры и «_» — как у Mojang и Ely.by.
fn validate_nick(name: &str) -> Result<()> {
    let valid = (3..=16).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if valid {
        Ok(())
    } else {
        Err(Error::BadNickname(name.to_string()))
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
        assert!(Session::offline("Majorzxc").is_ok());
        assert!(Session::offline("ab").is_err());
        assert!(Session::offline("Мажор").is_err());
        assert!(Session::offline("a_very_long_nickname_123").is_err());
    }

    fn store() -> (tempfile::TempDir, Accounts) {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::load(&DataDir::new(dir.path())).unwrap();
        (dir, accounts)
    }

    #[test]
    fn offline_accounts_and_default() {
        let (dir, mut accounts) = store();
        accounts.add_offline("Steve").unwrap();
        accounts.add_offline("Alex").unwrap();
        assert!(matches!(
            accounts.add_offline("steve"),
            Err(Error::AccountExists(_))
        ));

        assert_eq!(
            accounts.default_account().unwrap().name,
            "Steve",
            "первый — по умолчанию"
        );
        accounts.set_default("alex").unwrap();
        assert_eq!(accounts.default_account().unwrap().name, "Alex");

        // Переживает перезагрузку, токен клиента не меняется.
        let token = accounts.file.client_token.clone();
        let reloaded = Accounts::load(&DataDir::new(dir.path())).unwrap();
        assert_eq!(reloaded.list().len(), 2);
        assert_eq!(reloaded.default_account().unwrap().name, "Alex");
        assert_eq!(reloaded.file.client_token, token);
    }

    #[test]
    fn find_with_kind_prefix() {
        let (_dir, mut accounts) = store();
        accounts.add_offline("Steve").unwrap();
        accounts.file.accounts.push(Account {
            id: "ely-1".into(),
            kind: Kind::Ely,
            name: "Steve".into(),
            uuid: "1".into(),
            login: None,
        });
        assert!(matches!(
            accounts.find("Steve"),
            Err(Error::AmbiguousAccount(_))
        ));
        assert_eq!(accounts.find("ely:steve").unwrap().id, "ely-1");
        assert_eq!(accounts.find("offline:Steve").unwrap().kind, Kind::Offline);
        assert!(matches!(
            accounts.find("Herobrine"),
            Err(Error::AccountNotFound(_))
        ));
    }
}
