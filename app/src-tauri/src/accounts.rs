//! Аккаунты в шапке окна: список, переключение, ники, вход в Ely.by.
//! Пароль Ely.by проходит через ядро прямо на сервер и нигде не сохраняется.

use majorlauncher_core::{Accounts, Error, Kind};
use serde::Serialize;
use tauri::State;

use crate::{AppState, CmdResult, text};

#[derive(Serialize)]
pub struct AccountView {
    id: String,
    kind: Kind,
    name: String,
    is_default: bool,
}

fn load(state: &State<AppState>) -> CmdResult<Accounts> {
    Accounts::load(&state.data()?).map_err(text)
}

fn views(accounts: &Accounts) -> Vec<AccountView> {
    accounts
        .list()
        .iter()
        .map(|a| AccountView {
            id: a.id.clone(),
            kind: a.kind,
            name: a.name.clone(),
            is_default: accounts.is_default(a),
        })
        .collect()
}

/// Все аккаунты; аккаунт по умолчанию отмечен. Папка данных не выбрана — пусто.
#[tauri::command]
pub fn accounts_list(state: State<AppState>) -> CmdResult<Vec<AccountView>> {
    if state.data().is_err() {
        return Ok(Vec::new());
    }
    Ok(views(&load(&state)?))
}

#[tauri::command]
pub fn account_set_default(state: State<AppState>, id: String) -> CmdResult<Vec<AccountView>> {
    let mut accounts = load(&state)?;
    accounts.set_default(&id).map_err(text)?;
    Ok(views(&accounts))
}

/// Добавить офлайн-ник. Новый аккаунт сразу становится выбранным.
#[tauri::command]
pub fn account_add_offline(state: State<AppState>, nick: String) -> CmdResult<Vec<AccountView>> {
    let mut accounts = load(&state)?;
    let account = accounts.add_offline(nick.trim()).map_err(text)?;
    accounts.set_default(&account.id).map_err(text)?;
    Ok(views(&accounts))
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ElyLogin {
    Done {
        accounts: Vec<AccountView>,
    },
    /// Включена двухфакторная защита — окно спросит код и повторит вход.
    NeedCode,
}

#[tauri::command]
pub async fn account_add_ely(
    state: State<'_, AppState>,
    login: String,
    password: String,
    code: Option<String>,
) -> CmdResult<ElyLogin> {
    let mut accounts = load(&state)?;
    let code = code.as_deref().map(str::trim).filter(|c| !c.is_empty());
    match accounts
        .add_ely(&state.http, login.trim(), &password, code)
        .await
    {
        Ok(account) => {
            accounts.set_default(&account.id).map_err(text)?;
            Ok(ElyLogin::Done {
                accounts: views(&accounts),
            })
        }
        Err(Error::ElyTwoFactorRequired) => Ok(ElyLogin::NeedCode),
        Err(e) => Err(text(e)),
    }
}

/// Удалить аккаунт. Токен Ely.by отзывается на сервере.
#[tauri::command]
pub async fn account_remove(state: State<'_, AppState>, id: String) -> CmdResult<Vec<AccountView>> {
    let mut accounts = load(&state)?;
    accounts.remove(&state.http, &id).await.map_err(text)?;
    Ok(views(&accounts))
}
