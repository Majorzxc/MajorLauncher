//! Подозрительный код в модах (этап 5.4): эвристика по байт-коду. Это не
//! антивирус и не приговор — лаунчер объясняет, что нашёл, а решает игрок.
//!
//! Из каждого класса читается пул констант: какие методы Java код вызывает
//! (вызвать метод, не упомянув его там, нельзя) и какие строки в нём лежат.
//! Признаки двух уровней:
//! - **явные** — так делают стилеры и загрузчики чужого кода: пути к данным
//!   Discord, браузеров, аккаунтов лаунчеров и кошельков; класс, который сам
//!   качает из сети и сам же превращает байты в код;
//! - **косвенные** — бывают и у честных модов: запуск программ, нативные
//!   библиотеки, расшифровка вместе с созданием классов, токен сессии рядом
//!   с сетью, отправка в вебхуки Discord или Telegram.
//!
//! Модам с Modrinth показываются только явные признаки (их проверяет и
//! сам Modrinth), остальным — все.

use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Strength {
    /// Так делают стилеры и загрузчики чужого кода.
    Strong,
    /// Бывает и у честных модов.
    Weak,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub strength: Strength,
    /// Что делает код — для игрока.
    pub what: String,
    /// Где найдено (класс).
    pub at: String,
}

/// Вложенные jar проверяем, но не глубже этого.
const MAX_DEPTH: usize = 2;
const MAX_ENTRY: u64 = 64 * 1024 * 1024;
/// Больше этого одинаковых признаков не собираем.
const MAX_FINDINGS: usize = 8;

/// Пути к чужим секретам (в нижнем регистре, с прямыми слешами) и чьи они.
const SECRETS: &[(&str, &str)] = &[
    ("discord/local storage/leveldb", "токены Discord"),
    ("discordcanary/local storage", "токены Discord"),
    ("discordptb/local storage", "токены Discord"),
    ("google/chrome/user data", "данные Chrome (пароли, cookies)"),
    ("microsoft/edge/user data", "данные Edge (пароли, cookies)"),
    ("bravesoftware/brave-browser/user data", "данные Brave"),
    ("yandex/yandexbrowser/user data", "данные Яндекс Браузера"),
    ("opera software/opera", "данные Opera"),
    ("mozilla/firefox/profiles", "данные Firefox"),
    ("/login data", "сохранённые пароли браузера"),
    ("launcher_accounts.json", "аккаунты официального лаунчера"),
    (
        "launcher_accounts_microsoft_store.json",
        "аккаунты официального лаунчера",
    ),
    ("feather/accounts.json", "аккаунты Feather"),
    (
        "lunarclient/settings/game/accounts.json",
        "аккаунты Lunar Client",
    ),
    (".essential/microsoft_accounts.json", "аккаунты Essential"),
    ("prismlauncher/accounts.json", "аккаунты Prism Launcher"),
    ("polymc/accounts.json", "аккаунты PolyMC"),
    ("multimc/accounts.json", "аккаунты MultiMC"),
    ("telegram desktop/tdata", "сессию Telegram"),
    ("exodus/exodus.wallet", "криптокошелёк Exodus"),
    ("electrum/wallets", "криптокошелёк Electrum"),
];

const WEBHOOKS: &[&str] = &[
    "discord.com/api/webhooks",
    "discordapp.com/api/webhooks",
    "api.telegram.org/bot",
];

/// Методы Java, по которым видно, что делает код (`класс.метод`).
const NETWORK: &[&str] = &[
    "java/net/URL.openConnection",
    "java/net/URL.openStream",
    "java/net/http/HttpClient.send",
    "java/net/http/HttpClient.sendAsync",
    "java/net/Socket.<init>",
    "okhttp3/OkHttpClient.newCall",
];
const DEFINE: &[&str] = &[
    "java/lang/ClassLoader.defineClass",
    "java/security/SecureClassLoader.defineClass",
    "java/lang/invoke/MethodHandles$Lookup.defineClass",
    "java/lang/invoke/MethodHandles$Lookup.defineHiddenClass",
    "sun/misc/Unsafe.defineClass",
    "jdk/internal/misc/Unsafe.defineClass",
];
const URL_LOADER: &[&str] = &[
    "java/net/URLClassLoader.<init>",
    "java/net/URLClassLoader.newInstance",
];
const EXEC: &[&str] = &["java/lang/Runtime.exec", "java/lang/ProcessBuilder.start"];
const NATIVE: &[&str] = &[
    "java/lang/System.load",
    "java/lang/System.loadLibrary",
    "java/lang/Runtime.load",
];
const CIPHER: &[&str] = &["javax/crypto/Cipher.doFinal"];
/// Токен сессии игрока: имена Mojang (Forge, NeoForge) и intermediary (Fabric).
const TOKEN: &[&str] = &[
    "net/minecraft/client/User.getAccessToken",
    "net/minecraft/class_320.method_1674",
];

/// Что упоминает один класс.
#[derive(Default)]
struct Refs {
    methods: HashSet<String>,
    strings: Vec<String>,
}

impl Refs {
    fn calls(&self, list: &[&str]) -> bool {
        list.iter().any(|m| self.methods.contains(*m))
    }
}

/// Прочитать пул констант класса. Битый класс — пустой результат.
fn refs(class: &[u8]) -> Refs {
    let mut out = Refs::default();
    let mut r = Reader { b: class, at: 0 };
    if r.u4() != Some(0xCAFE_BABE) {
        return out;
    }
    r.at += 4; // версия
    let Some(count) = r.u2() else { return out };
    let count = count as usize;
    // Слоты: Utf8 — текст; Class — номер Utf8; NameAndType — (имя, дескриптор);
    // Methodref — (класс, NameAndType).
    let mut utf8: Vec<Option<String>> = vec![None; count];
    let mut class_name: Vec<Option<u16>> = vec![None; count];
    let mut nat: Vec<Option<u16>> = vec![None; count];
    let mut method: Vec<(u16, u16)> = Vec::new();
    let mut string: Vec<u16> = Vec::new();
    let mut i = 1;
    while i < count {
        let Some(tag) = r.u1() else { return out };
        match tag {
            1 => {
                let Some(len) = r.u2() else { return out };
                let Some(bytes) = r.take(len as usize) else {
                    return out;
                };
                utf8[i] = Some(String::from_utf8_lossy(bytes).into_owned());
            }
            3 | 4 => r.at += 4,
            5 | 6 => {
                r.at += 8;
                i += 1; // занимает два слота
            }
            7 => class_name[i] = r.u2(),
            8 => {
                if let Some(s) = r.u2() {
                    string.push(s);
                }
            }
            9 => r.at += 4,
            10 | 11 => {
                let (Some(c), Some(n)) = (r.u2(), r.u2()) else {
                    return out;
                };
                method.push((c, n));
            }
            12 => {
                nat[i] = r.u2();
                r.at += 2;
            }
            15 => r.at += 3,
            16 | 19 | 20 => r.at += 2,
            17 | 18 => r.at += 4,
            _ => return out, // незнакомый тег — дальше не разобрать
        }
        i += 1;
    }
    let text = |n: u16| utf8.get(n as usize).and_then(|s| s.as_deref());
    for (c, n) in method {
        let owner = class_name.get(c as usize).copied().flatten().and_then(text);
        let name = nat.get(n as usize).copied().flatten().and_then(text);
        if let (Some(owner), Some(name)) = (owner, name) {
            out.methods.insert(format!("{owner}.{name}"));
        }
    }
    out.strings = string
        .into_iter()
        .filter_map(|s| text(s).map(str::to_string))
        .collect();
    out
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.at..self.at + n)?;
        self.at += n;
        Some(s)
    }
    fn u1(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u2(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }
    fn u4(&mut self) -> Option<u32> {
        self.take(4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// Признаки одного класса.
fn judge(class: &str, r: &Refs, out: &mut Vec<Finding>) {
    let mut add = |strength, what: String| {
        if out.len() < MAX_FINDINGS && !out.iter().any(|f| f.what == what) {
            out.push(Finding {
                strength,
                what,
                at: class.to_string(),
            });
        }
    };
    let lower: Vec<String> = r
        .strings
        .iter()
        .map(|s| s.to_lowercase().replace('\\', "/"))
        .collect();
    for (path, whose) in SECRETS {
        if lower.iter().any(|s| s.contains(path)) {
            add(
                Strength::Strong,
                format!("ищет файлы с чужими данными: {whose}"),
            );
        }
    }
    let net = r.calls(NETWORK);
    if net && (r.calls(DEFINE) || r.calls(URL_LOADER)) {
        add(
            Strength::Strong,
            "скачивает из сети и запускает код, которого нет в самом моде".into(),
        );
    } else if r.calls(URL_LOADER) {
        add(Strength::Weak, "подгружает классы из внешних файлов".into());
    }
    if r.calls(EXEC) {
        add(Strength::Weak, "запускает программы на компьютере".into());
    }
    if r.calls(NATIVE) {
        add(
            Strength::Weak,
            "загружает нативную библиотеку (код не на Java)".into(),
        );
    }
    if r.calls(CIPHER) && r.calls(DEFINE) {
        add(
            Strength::Weak,
            "расшифровывает данные и создаёт из них классы — так прячут код".into(),
        );
    }
    let webhook = lower.iter().any(|s| WEBHOOKS.iter().any(|w| s.contains(w)));
    if r.calls(TOKEN) && (net || webhook) {
        add(
            Strength::Weak,
            "берёт токен сессии игрока рядом с отправкой в сеть".into(),
        );
    }
    if webhook {
        add(
            Strength::Weak,
            "отправляет данные в вебхук Discord или бота Telegram".into(),
        );
    }
}

/// Проверить jar. Пусто — ничего подозрительного.
pub fn scan(jar: &Path) -> Vec<Finding> {
    inspect(jar).1
}

/// Один проход по jar: известная зараза ([`crate::malware`]) и признаки
/// подозрительного кода.
pub fn inspect(jar: &Path) -> (Option<String>, Vec<Finding>) {
    let Ok(file) = std::fs::File::open(jar) else {
        return (None, Vec::new());
    };
    let mut out = Vec::new();
    let mut malware = None;
    scan_zip(file, 0, "", &mut out, &mut malware);
    // Сначала явные.
    out.sort_by_key(|f| f.strength != Strength::Strong);
    (malware, out)
}

fn scan_zip<R: Read + std::io::Seek>(
    reader: R,
    depth: usize,
    inside: &str,
    out: &mut Vec<Finding>,
    malware: &mut Option<String>,
) {
    let Ok(mut zip) = zip::ZipArchive::new(reader) else {
        return;
    };
    for i in 0..zip.len() {
        if malware.is_some() {
            return;
        }
        let Ok(mut entry) = zip.by_index(i) else {
            continue;
        };
        let name = entry.name().to_string();
        let within = |what: &str| {
            if inside.is_empty() {
                format!("{what} в {name}")
            } else {
                format!("{what} в {name} (внутри {inside})")
            }
        };
        if let Some(found) = crate::malware::name_hit(&name) {
            *malware = Some(within(found));
            return;
        }
        let is_class = name.ends_with(".class");
        let is_jar = name.ends_with(".jar") && depth < MAX_DEPTH;
        if !(is_class || is_jar) || entry.size() > MAX_ENTRY {
            continue;
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        if entry.read_to_end(&mut bytes).is_err() {
            continue;
        }
        let place = if inside.is_empty() {
            name.clone()
        } else {
            format!("{name} (внутри {inside})")
        };
        if is_class {
            if let Some(found) = crate::malware::class_hit(&bytes) {
                *malware = Some(within(found));
                return;
            }
            if out.len() < MAX_FINDINGS {
                judge(&place, &refs(&bytes), out);
            }
        } else {
            scan_zip(std::io::Cursor::new(bytes), depth + 1, &name, out, malware);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Минимальный класс: пул констант с методами и строками.
    fn class(methods: &[(&str, &str)], strings: &[&str]) -> Vec<u8> {
        let mut pool: Vec<Vec<u8>> = Vec::new();
        let utf8 = |pool: &mut Vec<Vec<u8>>, s: &str| -> u16 {
            let mut e = vec![1];
            e.extend((s.len() as u16).to_be_bytes());
            e.extend(s.as_bytes());
            pool.push(e);
            pool.len() as u16
        };
        for (owner, name) in methods {
            let o = utf8(&mut pool, owner);
            pool.push([vec![7], o.to_be_bytes().to_vec()].concat());
            let c = pool.len() as u16;
            let n = utf8(&mut pool, name);
            let d = utf8(&mut pool, "()V");
            pool.push([vec![12], n.to_be_bytes().to_vec(), d.to_be_bytes().to_vec()].concat());
            let nt = pool.len() as u16;
            pool.push(
                [
                    vec![10],
                    c.to_be_bytes().to_vec(),
                    nt.to_be_bytes().to_vec(),
                ]
                .concat(),
            );
        }
        for s in strings {
            let u = utf8(&mut pool, s);
            pool.push([vec![8], u.to_be_bytes().to_vec()].concat());
        }
        let mut out = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65];
        out.extend(((pool.len() + 1) as u16).to_be_bytes());
        for e in pool {
            out.extend(e);
        }
        out
    }

    fn jar(dir: &Path, entries: &[(&str, Vec<u8>)]) -> std::path::PathBuf {
        let path = dir.join(format!("m{}.jar", entries.len()));
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn honest_mod_is_clean() {
        let dir = tempfile::tempdir().unwrap();
        // Проверка обновлений: сеть без создания кода — не признак.
        let c = class(
            &[
                ("java/net/URL", "openStream"),
                ("java/lang/String", "length"),
            ],
            &["https://api.modrinth.com/v2/project/x"],
        );
        assert!(scan(&jar(dir.path(), &[("a/Update.class", c)])).is_empty());
    }

    #[test]
    fn finds_stealer_and_loader() {
        let dir = tempfile::tempdir().unwrap();
        let stealer = class(
            &[("java/nio/file/Files", "readAllBytes")],
            &[
                "\\Discord\\Local Storage\\leveldb",
                "https://discord.com/api/webhooks/1/x",
            ],
        );
        let loader = class(
            &[
                ("java/net/URL", "openConnection"),
                ("java/lang/ClassLoader", "defineClass"),
            ],
            &[],
        );
        let found = scan(&jar(
            dir.path(),
            &[("x/A.class", stealer), ("x/B.class", loader)],
        ));
        assert!(
            found
                .iter()
                .any(|f| f.strength == Strength::Strong && f.what.contains("Discord"))
        );
        assert!(
            found
                .iter()
                .any(|f| f.strength == Strength::Strong && f.what.contains("скачивает"))
        );
        assert!(
            found
                .iter()
                .any(|f| f.strength == Strength::Weak && f.what.contains("вебхук"))
        );
        assert_eq!(found[0].strength, Strength::Strong, "явные — первыми");
    }

    #[test]
    fn weak_signs_and_broken_classes() {
        let dir = tempfile::tempdir().unwrap();
        let exec = class(&[("java/lang/ProcessBuilder", "start")], &[]);
        let found = scan(&jar(dir.path(), &[("a/Open.class", exec)]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].strength, Strength::Weak);
        // Битый класс и не класс — без паники.
        let found = scan(&jar(
            dir.path(),
            &[(
                "a/B.class",
                vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65, 0, 9, 1, 0xFF],
            )],
        ));
        assert!(found.is_empty());
    }
}
