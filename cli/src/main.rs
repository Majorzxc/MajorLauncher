//! `mlcli` — консольная утилита поверх ядра. Через неё проверяется скачивание
//! и запуск игры до того, как появится окно, и на ней же гоняются тесты.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use majorlauncher_core::{
    Accounts, DataDir, DownloadOptions, Downloader, Error, Event, LaunchOptions, Mode, Progress,
    Session, install, launch,
};

#[derive(Parser)]
#[command(name = "mlcli", version, about = "Консольная утилита MajorLauncher")]
struct Cli {
    /// Папка данных: версии, библиотеки, ассеты, Java, сборки.
    #[arg(long, global = true, default_value = "mldata")]
    data: PathBuf,
    /// Сколько файлов качать одновременно.
    #[arg(long, global = true, default_value_t = 16)]
    parallel: usize,
    /// Не использовать запасные зеркала.
    #[arg(long, global = true)]
    no_mirrors: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Список версий игры.
    Versions {
        /// Показать и снапшоты, и старые альфы и беты.
        #[arg(long)]
        all: bool,
        /// Сколько версий показать.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Скачать версию целиком: Java, библиотеки, ассеты, клиент.
    Install {
        /// Версия: 1.21.1, 26.3 или release / snapshot — последняя.
        version: String,
    },
    /// Пересчитать хеши всех файлов версии и докачать повреждённые.
    Verify { version: String },
    /// Аккаунты: офлайн-ники и Ely.by. Без подкоманды — список.
    Accounts {
        #[command(subcommand)]
        action: Option<AccountsAction>,
    },
    /// Запустить версию (докачает недостающее). Без --account и --nick —
    /// с аккаунтом по умолчанию.
    Launch {
        version: String,
        /// Аккаунт из списка: ник или offline:ник / ely:ник.
        #[arg(long, conflicts_with = "nick")]
        account: Option<String>,
        /// Разовый офлайн-ник без сохранения: 3–16 символов, латиница, цифры, «_».
        #[arg(long)]
        nick: Option<String>,
        /// Память для игры, МБ.
        #[arg(long, default_value_t = 4096)]
        memory: u32,
        /// Имя сборки (папки игры). По умолчанию — как версия.
        #[arg(long)]
        instance: Option<String>,
        /// Только показать команду запуска, не запускать.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum AccountsAction {
    /// Список аккаунтов; звёздочка — аккаунт по умолчанию.
    List,
    /// Добавить офлайн-ник.
    AddOffline { nick: String },
    /// Войти через Ely.by. Пароль спрашивается без отображения на экране
    /// и не сохраняется — лаунчер хранит только токен в хранилище Windows.
    AddEly {
        /// Ник или почта Ely.by.
        login: String,
    },
    /// Удалить аккаунт (токен Ely.by отзывается на сервере).
    Remove { account: String },
    /// Сделать аккаунтом по умолчанию.
    Default { account: String },
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("MLCLI_LOG")
                .unwrap_or_else(|_| "error".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match run(cli).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("\nОшибка: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> majorlauncher_core::Result<ExitCode> {
    let data = DataDir::new(cli.data);
    let dl = Downloader::new(DownloadOptions {
        parallel: cli.parallel,
        mirrors: !cli.no_mirrors,
        ..DownloadOptions::default()
    })?;

    match cli.command {
        Command::Versions { all, limit } => {
            let manifest = install::fetch_manifest(&data, &dl).await?;
            println!(
                "Последний релиз: {}   последний снапшот: {}\n",
                manifest.latest.release, manifest.latest.snapshot
            );
            for v in manifest
                .versions
                .iter()
                .filter(|v| all || v.kind == "release")
                .take(limit)
            {
                let date = v.release_time.get(..10).unwrap_or(&v.release_time);
                println!("{:<24} {:<10} {date}", v.id, v.kind);
            }
        }
        Command::Install { version } => {
            let started = Instant::now();
            let prepared =
                install::prepare(&data, &dl, &version, Mode::Install, progress()).await?;
            finish_line();
            println!(
                "Версия {} установлена за {:.1} с",
                prepared.version.id,
                started.elapsed().as_secs_f64()
            );
        }
        Command::Verify { version } => {
            let started = Instant::now();
            let prepared = install::prepare(&data, &dl, &version, Mode::Verify, progress()).await?;
            finish_line();
            println!(
                "Версия {} проверена за {:.1} с, все файлы целы",
                prepared.version.id,
                started.elapsed().as_secs_f64()
            );
        }
        Command::Accounts { action } => {
            accounts(&data, &dl, action.unwrap_or(AccountsAction::List)).await?;
        }
        Command::Launch {
            version,
            account,
            nick,
            memory,
            instance,
            dry_run,
        } => {
            let started = Instant::now();
            // Вход и проверка файлов идут одновременно: оба ждут сеть или диск.
            let (session, prepared) = tokio::join!(
                session(&data, &dl, account, nick),
                install::prepare(&data, &dl, &version, Mode::Launch, progress())
            );
            let (session, prepared) = (session?, prepared?);
            let instance = instance.unwrap_or_else(|| prepared.version.id.clone());
            let opts = LaunchOptions {
                game_dir: data.instance(&instance),
                memory_mb: memory,
                console: true,
                extra_jvm_args: Vec::new(),
            };
            let cmd = launch::build(&data, &prepared, &session, &opts)?;
            finish_line();
            println!("Подготовка к запуску: {} мс", started.elapsed().as_millis());
            println!("Версия:  {}", prepared.version.id);
            println!(
                "Аккаунт: {} ({}, {})",
                session.name,
                session.kind.label(),
                session.uuid
            );
            if session.unverified {
                println!(
                    "         Нет связи с Ely.by: одиночная игра работает, на серверы Ely.by может не пустить."
                );
            }
            println!("Java:    {}", cmd.program.display());
            println!("Память:  {memory} МБ");
            println!("Сборка:  {}", cmd.cwd.display());

            if dry_run {
                println!("\nКоманда запуска:\n{}", cmd.program.display());
                for arg in &cmd.args {
                    println!("  {arg}");
                }
                return Ok(ExitCode::SUCCESS);
            }

            println!("\nЗапускаем игру…\n");
            let status = std::process::Command::new(&cmd.program)
                .args(&cmd.args)
                .current_dir(&cmd.cwd)
                .status()
                .map_err(|e| Error::Other(format!("не удалось запустить Java: {e}")))?;
            println!(
                "\nИгра закрылась, код выхода: {}",
                status.code().unwrap_or(-1)
            );
            return Ok(if status.success() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            });
        }
    }
    Ok(ExitCode::SUCCESS)
}

async fn session(
    data: &DataDir,
    dl: &Downloader,
    account: Option<String>,
    nick: Option<String>,
) -> majorlauncher_core::Result<Session> {
    if let Some(nick) = nick {
        return Session::offline(&nick);
    }
    let mut accounts = Accounts::load(data)?;
    let id = match account {
        Some(reference) => accounts.find(&reference)?.id.clone(),
        None => accounts
            .default_account()
            .ok_or(Error::NoAccounts)?
            .id
            .clone(),
    };
    accounts.session(data, dl, &id).await
}

async fn accounts(
    data: &DataDir,
    dl: &Downloader,
    action: AccountsAction,
) -> majorlauncher_core::Result<()> {
    let mut accounts = Accounts::load(data)?;
    match action {
        AccountsAction::List => {
            if accounts.list().is_empty() {
                println!(
                    "Аккаунтов нет. Добавьте: mlcli accounts add-offline <ник> или add-ely <логин>"
                );
            }
            for a in accounts.list() {
                let mark = if accounts.is_default(a) { "*" } else { " " };
                println!("{mark} {:<17} {:<8} {}", a.name, a.kind.label(), a.uuid);
            }
        }
        AccountsAction::AddOffline { nick } => {
            let a = accounts.add_offline(&nick)?;
            println!("Добавлен офлайн-ник {}", a.name);
        }
        AccountsAction::AddEly { login } => {
            // Подсказку печатаем сами: rpassword пишет её в обход консоли Windows,
            // и в PowerShell кириллица превращается в кракозябры.
            print!("Пароль Ely.by (не отображается): ");
            let _ = std::io::stdout().flush();
            let password = rpassword::read_password()
                .map_err(|e| Error::Other(format!("не удалось прочитать пароль: {e}")))?;
            let a = match accounts.add_ely(dl, &login, &password, None).await {
                Err(Error::ElyTwoFactorRequired) => {
                    let code = prompt("Код двухфакторной защиты: ")?;
                    accounts
                        .add_ely(dl, &login, &password, Some(code.trim()))
                        .await?
                }
                other => other?,
            };
            println!(
                "Вход выполнен: {} (Ely.by). Пароль не сохранён — только токен в хранилище Windows.",
                a.name
            );
        }
        AccountsAction::Remove { account } => {
            let a = accounts.remove(dl, &account).await?;
            println!("Удалён аккаунт {} ({})", a.name, a.kind.label());
        }
        AccountsAction::Default { account } => {
            accounts.set_default(&account)?;
            println!("Аккаунт по умолчанию: {}", accounts.find(&account)?.name);
        }
    }
    Ok(())
}

fn prompt(text: &str) -> majorlauncher_core::Result<String> {
    print!("{text}");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| Error::Other(format!("не удалось прочитать ввод: {e}")))?;
    Ok(line)
}

/// Полоса прогресса в одну строку, обновляется не чаще раза в 150 мс.
fn progress() -> Progress {
    struct State {
        files: usize,
        done: usize,
        bytes: u64,
        done_bytes: u64,
        drawn: Instant,
    }
    let state = Mutex::new(State {
        files: 0,
        done: 0,
        bytes: 0,
        done_bytes: 0,
        drawn: Instant::now(),
    });
    Arc::new(move |event| {
        let mut s = state.lock().unwrap();
        match event {
            Event::Checking { files } => {
                eprint!("\rПроверка файлов: {files}…{:<30}", "");
            }
            Event::Downloading { files, bytes } => {
                s.files = files;
                s.bytes = bytes;
                eprint!(
                    "\rНужно скачать: {files} файлов, {:.1} МБ{:<20}",
                    mb(bytes),
                    ""
                );
            }
            Event::FileDone { bytes } => {
                s.done += 1;
                s.done_bytes += bytes;
                if s.done == s.files || s.drawn.elapsed() > Duration::from_millis(150) {
                    s.drawn = Instant::now();
                    eprint!(
                        "\rЗагрузка: {}/{} файлов, {:.1}/{:.1} МБ{:<10}",
                        s.done,
                        s.files,
                        mb(s.done_bytes),
                        mb(s.bytes),
                        ""
                    );
                }
            }
            Event::Extracting => eprint!("\rРаспаковка нативных библиотек…{:<30}", ""),
        }
        let _ = std::io::stderr().flush();
    })
}

fn finish_line() {
    eprintln!();
}

fn mb(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}
