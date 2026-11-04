//! Опрос сервера — то же, что делает игра в меню «Сетевая игра»: онлайн ли
//! он, сколько игроков, какая версия, описание и иконка, задержка.
//!
//! Протоколы: с 1.7 — пакеты с VarInt и ответ в JSON; 1.4–1.6 — старый
//! запрос `FE 01`; Beta 1.8–1.3 отвечают на него же строкой «описание§онлайн§
//! максимум». Новые серверы понимают и старый запрос, поэтому сначала новый,
//! не вышло — старый.

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use futures_util::{StreamExt, stream};
use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::dns;
use super::motd::{self, Span};

pub const DEFAULT_PORT: u16 = 25565;
const CONNECT: Duration = Duration::from_secs(5);
const REPLY: Duration = Duration::from_secs(5);
const PONG: Duration = Duration::from_secs(2);
/// Весь опрос одного сервера, со всеми попытками.
const TOTAL: Duration = Duration::from_secs(12);
/// Ответ с иконкой и списком модов Forge бывает большим, но не таким.
const MAX_PACKET: usize = 4 << 20;
/// Серверов за раз: опрос — это ожидание сети, а не работа процессора.
const PARALLEL: usize = 16;

/// Адрес как его пишет игрок: `host`, `host:port`, `[IPv6]:port`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub host: String,
    /// Не указан — 25565 или порт из записи SRV.
    pub port: Option<u16>,
}

impl Address {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() || s.len() > 255 || s.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return None;
        }
        let port = |p: &str| p.parse::<u16>().ok().filter(|p| *p != 0);
        if let Some(rest) = s.strip_prefix('[') {
            let (host, tail) = rest.split_once(']')?;
            host.parse::<std::net::Ipv6Addr>().ok()?;
            let port = match tail {
                "" => None,
                t => Some(port(t.strip_prefix(':')?)?),
            };
            return Some(Self {
                host: host.into(),
                port,
            });
        }
        if s.parse::<std::net::Ipv6Addr>().is_ok() {
            return Some(Self {
                host: s.into(),
                port: None,
            });
        }
        let (host, port) = match s.rsplit_once(':') {
            Some((h, p)) => (h, Some(port(p)?)),
            None => (s, None),
        };
        let ok = !host.is_empty()
            && host
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'));
        ok.then(|| Self {
            host: host.into(),
            port,
        })
    }

    fn is_ip(&self) -> bool {
        self.host.parse::<IpAddr>().is_ok()
    }
}

/// Почему сервер не ответил — словами для игрока.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PingError {
    #[error("неверный адрес")]
    BadAddress,
    #[error("адрес не найден")]
    NotFound,
    #[error("сервер не отвечает")]
    Timeout,
    #[error("сервер выключен или порт закрыт")]
    Refused,
    #[error("сервер недоступен из вашей сети")]
    Unreachable,
    #[error("нет соединения: {0}")]
    Network(String),
    #[error("ответ не похож на сервер Minecraft")]
    Protocol,
}

impl PingError {
    /// Коротко для окна: `offline` — сервер не ответил, `bad` — адрес не тот.
    pub fn kind(&self) -> &'static str {
        match self {
            PingError::BadAddress | PingError::NotFound => "bad",
            _ => "offline",
        }
    }
}

fn io_error(e: std::io::Error) -> PingError {
    use std::io::ErrorKind::*;
    match e.kind() {
        ConnectionRefused => PingError::Refused,
        NetworkUnreachable | HostUnreachable => PingError::Unreachable,
        TimedOut => PingError::Timeout,
        UnexpectedEof | InvalidData | ConnectionReset | ConnectionAborted => PingError::Protocol,
        _ => PingError::Network(e.to_string()),
    }
}

/// Моды на сервере (так о себе сообщают Forge и NeoForge).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModInfo {
    /// `Forge`; неизвестно — `None`.
    pub loader: Option<String>,
    /// Сколько модов; новые Forge список сжимают — тогда неизвестно.
    pub mods: Option<usize>,
}

/// Что ответил сервер.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    /// Как сервер называет свою версию: «1.21.1», «Paper 1.20.4», «1.8–1.21».
    pub version: String,
    /// Номер протокола; у Beta 1.8–1.3 его нет.
    pub protocol: Option<i32>,
    /// Игроки сейчас и место; сервер может их скрыть.
    pub online: Option<u32>,
    pub max: Option<u32>,
    /// Ники из ответа (сервер показывает часть или вписывает туда свой текст).
    pub sample: Vec<String>,
    pub motd: Vec<Span>,
    /// PNG 64×64 в base64, как игра пишет в `servers.dat`.
    pub favicon: Option<String>,
    pub ping_ms: u32,
    pub modded: Option<ModInfo>,
    /// Ответил старым протоколом (1.6 и раньше).
    pub legacy: bool,
}

/// Опросить сервер по адресу из списка.
pub async fn ping(address: &str) -> Result<Status, PingError> {
    let addr = Address::parse(address).ok_or(PingError::BadAddress)?;
    timeout(TOTAL, ping_addr(&addr))
        .await
        .unwrap_or(Err(PingError::Timeout))
}

/// Опросить несколько серверов сразу; ответы — в том же порядке.
pub async fn ping_many(addresses: &[String]) -> Vec<Result<Status, PingError>> {
    stream::iter(addresses.iter().map(|a| ping(a)))
        .buffered(PARALLEL)
        .collect()
        .await
}

/// Опросить несколько серверов и сообщать о каждом, как только ответил
/// (или не ответил): живые не ждут тех, что молчат.
pub async fn ping_each(
    addresses: Vec<String>,
    mut each: impl FnMut(&str, &Result<Status, PingError>),
) {
    let mut replies = stream::iter(addresses)
        .map(|a| async move {
            let r = ping(&a).await;
            (a, r)
        })
        .buffer_unordered(PARALLEL);
    while let Some((a, r)) = replies.next().await {
        each(&a, &r);
    }
}

async fn ping_addr(addr: &Address) -> Result<Status, PingError> {
    // Порт не указан — сначала запись SRV, как в игре. Обычный адрес ищется
    // тут же, параллельно: несуществующее имя Windows ищет по 10 с и дольше.
    let sockets = match addr.port {
        Some(p) => resolve(&addr.host, p).await?,
        None if addr.is_ip() => resolve(&addr.host, DEFAULT_PORT).await?,
        None => {
            let (srv, plain) =
                tokio::join!(dns::srv(&addr.host), resolve(&addr.host, DEFAULT_PORT));
            match srv {
                Some((target, port)) => resolve(&target, port).await?,
                None => plain?,
            }
        }
    };
    // В приветствии — адрес, который ввёл игрок: по нему прокси выбирают сервер.
    let hello_port = addr.port.unwrap_or(DEFAULT_PORT);
    let mut stream = connect(&sockets).await?;
    match modern(&mut stream, &addr.host, hello_port).await {
        Ok(status) => Ok(status),
        Err(PingError::Protocol | PingError::Timeout) => {
            drop(stream);
            let mut stream = connect(&sockets).await?;
            timeout(REPLY, legacy(&mut stream, &addr.host, hello_port))
                .await
                .unwrap_or(Err(PingError::Timeout))
        }
        Err(e) => Err(e),
    }
}

async fn resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>, PingError> {
    // Свой поток, как у SRV: зависший поиск не держит пул tokio и выход.
    let (tx, rx) = tokio::sync::oneshot::channel();
    let name = (host.to_string(), port);
    std::thread::spawn(move || {
        use std::net::ToSocketAddrs;
        let _ = tx.send(name.to_socket_addrs().map(Vec::from_iter));
    });
    // Долгий поиск — почти всегда имени нет: настоящие адреса находятся за доли секунды.
    let mut sockets = timeout(CONNECT, rx)
        .await
        .ok()
        .and_then(Result::ok)
        .and_then(Result::ok)
        .ok_or(PingError::NotFound)?;
    // Есть IPv4 — только он, как в игре (Java берёт IPv4): IPv6 дома часто
    // нет, и его «сеть недоступна» спрятала бы настоящую причину.
    if sockets.iter().any(SocketAddr::is_ipv4) {
        sockets.retain(SocketAddr::is_ipv4);
    }
    if sockets.is_empty() {
        return Err(PingError::NotFound);
    }
    Ok(sockets)
}

async fn connect(sockets: &[SocketAddr]) -> Result<TcpStream, PingError> {
    let mut first = None;
    for socket in sockets.iter().take(2) {
        let e = match timeout(CONNECT, TcpStream::connect(socket)).await {
            Ok(Ok(s)) => {
                let _ = s.set_nodelay(true);
                return Ok(s);
            }
            Ok(Err(e)) => io_error(e),
            Err(_) => PingError::Timeout,
        };
        first.get_or_insert(e);
    }
    Err(first.unwrap_or(PingError::NotFound))
}

fn put_varint(out: &mut Vec<u8>, v: i32) {
    let mut u = v as u32;
    loop {
        if u & !0x7f == 0 {
            out.push(u as u8);
            return;
        }
        out.push((u & 0x7f) as u8 | 0x80);
        u >>= 7;
    }
}

fn put_string(out: &mut Vec<u8>, s: &str) {
    put_varint(out, s.len() as i32);
    out.extend_from_slice(s.as_bytes());
}

/// VarInt, первый байт которого уже прочитан.
async fn read_varint(
    r: &mut (impl AsyncRead + Unpin),
    first: Option<u8>,
) -> Result<i32, PingError> {
    let mut out = 0u32;
    for i in 0..5 {
        let b = match (i, first) {
            (0, Some(b)) => b,
            _ => r.read_u8().await.map_err(io_error)?,
        };
        out |= ((b & 0x7f) as u32) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(out as i32);
        }
    }
    Err(PingError::Protocol)
}

fn take_varint(buf: &[u8], at: &mut usize) -> Option<i32> {
    let mut out = 0u32;
    for i in 0..5 {
        let b = *buf.get(*at)?;
        *at += 1;
        out |= ((b & 0x7f) as u32) << (7 * i);
        if b & 0x80 == 0 {
            return Some(out as i32);
        }
    }
    None
}

async fn read_packet(s: &mut TcpStream, first: Option<u8>) -> Result<Vec<u8>, PingError> {
    let len = read_varint(s, first).await?;
    let len = usize::try_from(len)
        .ok()
        .filter(|n| (1..=MAX_PACKET).contains(n))
        .ok_or(PingError::Protocol)?;
    let mut buf = vec![0; len];
    s.read_exact(&mut buf).await.map_err(io_error)?;
    Ok(buf)
}

async fn modern(s: &mut TcpStream, host: &str, port: u16) -> Result<Status, PingError> {
    let mut hello = vec![0x00];
    // Протокол «не знаю»: сервер ответит своей версией, а не подстроится под нашу.
    put_varint(&mut hello, -1);
    put_string(&mut hello, host);
    hello.extend_from_slice(&port.to_be_bytes());
    put_varint(&mut hello, 1);
    let mut out = Vec::with_capacity(hello.len() + 8);
    put_varint(&mut out, hello.len() as i32);
    out.extend_from_slice(&hello);
    // Запрос состояния: пакет 0 без данных.
    out.extend_from_slice(&[0x01, 0x00]);
    s.write_all(&out).await.map_err(io_error)?;
    let sent = Instant::now();

    let reply = timeout(REPLY, async {
        let first = s.read_u8().await.map_err(io_error)?;
        // 0xFF — старый сервер выгнал «странного» клиента.
        if first == 0xff {
            return Err(PingError::Protocol);
        }
        read_packet(s, Some(first)).await
    })
    .await
    .map_err(|_| PingError::Timeout)??;
    let waited = sent.elapsed();
    let mut at = 0;
    let json = (take_varint(&reply, &mut at) == Some(0))
        .then(|| take_varint(&reply, &mut at))
        .flatten()
        .and_then(|n| usize::try_from(n).ok())
        .and_then(|n| reply.get(at..at.checked_add(n)?))
        .ok_or(PingError::Protocol)?;
    let value: Value = serde_json::from_slice(json).map_err(|_| PingError::Protocol)?;
    if !value.is_object() {
        return Err(PingError::Protocol);
    }

    // Задержка — по пакету «пинг», как в игре; не ответил — по ответу.
    let ping = timeout(PONG, async {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let mut p = vec![0x09, 0x01];
        p.extend_from_slice(&stamp.to_be_bytes());
        let start = Instant::now();
        s.write_all(&p).await.map_err(io_error)?;
        read_packet(s, None).await?;
        Ok::<_, PingError>(start.elapsed())
    })
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or(waited);
    Ok(from_modern(&value, ping))
}

fn ms(d: Duration) -> u32 {
    d.as_millis().min(u32::MAX as u128) as u32
}

fn count(v: &Value) -> Option<u32> {
    v.as_u64().map(|n| n.min(u32::MAX as u64) as u32)
}

fn from_modern(v: &Value, ping: Duration) -> Status {
    let version = &v["version"];
    let players = &v["players"];
    let sample = players["sample"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|p| p["name"].as_str())
                .map(|n| motd::plain(&motd::from_legacy(n)))
                .take(20)
                .collect()
        })
        .unwrap_or_default();
    Status {
        version: version["name"]
            .as_str()
            .map(|n| motd::plain(&motd::from_legacy(n)))
            .unwrap_or_default(),
        protocol: version["protocol"]
            .as_i64()
            .and_then(|p| i32::try_from(p).ok()),
        online: count(&players["online"]),
        max: count(&players["max"]),
        sample,
        motd: motd::from_json(&v["description"]),
        favicon: v["favicon"].as_str().and_then(clean_favicon),
        ping_ms: ms(ping),
        modded: modded(v),
        legacy: false,
    }
}

fn modded(v: &Value) -> Option<ModInfo> {
    // Прокси (BungeeCord, Velocity) присылают пустые сведения Forge «для
    // совместимости» — это не моды.
    if let Some(data) = v.get("forgeData") {
        let mods = data["mods"].as_array().map(Vec::len).unwrap_or(0);
        // Новый Forge сжимает список в строку «d».
        let packed = data["d"].as_str().is_some_and(|d| !d.is_empty());
        if mods > 0 || packed {
            return Some(ModInfo {
                loader: Some("Forge".into()),
                mods: (mods > 0).then_some(mods),
            });
        }
    }
    if let Some(info) = v.get("modinfo") {
        // Старый Forge: в списке есть и сама игра с загрузчиком.
        const CORE: [&str; 5] = ["minecraft", "mcp", "fml", "forge", "forgemod"];
        let mods = info["modList"].as_array().map(|l| {
            l.iter()
                .filter(|m| {
                    m["modid"]
                        .as_str()
                        .is_none_or(|id| !CORE.contains(&id.to_lowercase().as_str()))
                })
                .count()
        });
        if mods.is_some_and(|n| n > 0) {
            return Some(ModInfo {
                loader: Some("Forge".into()),
                mods,
            });
        }
    }
    (v["isModded"].as_bool() == Some(true)).then_some(ModInfo {
        loader: None,
        mods: None,
    })
}

/// Иконку игра принимает только PNG 64×64 — так же и мы: в `servers.dat`
/// и в окно ничего другого не попадёт.
fn clean_favicon(s: &str) -> Option<String> {
    let b64: String = s
        .strip_prefix("data:image/png;base64,")?
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if b64.len() > 256 * 1024 {
        return None;
    }
    let png = STANDARD.decode(&b64).ok()?;
    let size = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    let ok = png.len() > 24
        && png.starts_with(b"\x89PNG\r\n\x1a\n")
        && &png[12..16] == b"IHDR"
        && size(16) == 64
        && size(20) == 64;
    ok.then_some(b64)
}

/// Старый запрос (1.6): `FE 01 FA` и «MC|PingHost» с адресом.
async fn legacy(s: &mut TcpStream, host: &str, port: u16) -> Result<Status, PingError> {
    let utf16 = |out: &mut Vec<u8>, text: &str| {
        let units: Vec<u16> = text.encode_utf16().collect();
        out.extend_from_slice(&(units.len() as u16).to_be_bytes());
        for u in units {
            out.extend_from_slice(&u.to_be_bytes());
        }
    };
    let mut req = vec![0xfe, 0x01, 0xfa];
    utf16(&mut req, "MC|PingHost");
    let host_units = host.encode_utf16().count();
    req.extend_from_slice(&((7 + 2 * host_units) as u16).to_be_bytes());
    req.push(74); // протокол 1.6.2
    utf16(&mut req, host);
    req.extend_from_slice(&(port as i32).to_be_bytes());
    s.write_all(&req).await.map_err(io_error)?;
    let sent = Instant::now();

    if s.read_u8().await.map_err(io_error)? != 0xff {
        return Err(PingError::Protocol);
    }
    let len = s.read_u16().await.map_err(io_error)? as usize;
    let mut raw = vec![0; len * 2];
    s.read_exact(&mut raw).await.map_err(io_error)?;
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_be_bytes(c))
        .collect();
    parse_legacy(&String::from_utf16_lossy(&units), sent.elapsed()).ok_or(PingError::Protocol)
}

fn parse_legacy(text: &str, ping: Duration) -> Option<Status> {
    let num = |s: &str| s.trim().parse::<u32>().ok();
    let (protocol, version, motd, online, max) = if let Some(rest) = text.strip_prefix("§1\0") {
        // 1.4–1.6: протокол, версия, описание, онлайн, максимум через \0.
        let parts: Vec<&str> = rest.split('\0').collect();
        let [protocol, version, motd, online, max] = parts[..] else {
            return None;
        };
        (
            protocol.parse::<i32>().ok(),
            motd::plain(&motd::from_legacy(version)),
            motd,
            online,
            max,
        )
    } else {
        // Beta 1.8–1.3: «описание§онлайн§максимум».
        let mut parts = text.rsplitn(3, '§');
        let max = parts.next()?;
        let online = parts.next()?;
        let motd = parts.next()?;
        (None, String::new(), motd, online, max)
    };
    Some(Status {
        version,
        protocol,
        online: Some(num(online)?),
        max: Some(num(max)?),
        sample: Vec::new(),
        motd: motd::from_legacy(motd),
        favicon: None,
        ping_ms: ms(ping),
        modded: None,
        legacy: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn addresses() {
        let a = |s: &str| Address::parse(s).map(|a| (a.host, a.port));
        assert_eq!(a("mc.example.org"), Some(("mc.example.org".into(), None)));
        assert_eq!(a(" play.ru:25566 "), Some(("play.ru".into(), Some(25566))));
        assert_eq!(a("[::1]:25565"), Some(("::1".into(), Some(25565))));
        assert_eq!(a("::1"), Some(("::1".into(), None)));
        assert_eq!(a("сервер.рф"), Some(("сервер.рф".into(), None)));
        for bad in [
            "",
            "a b",
            "host:",
            "host:0",
            "host:70000",
            "a:b:c",
            "http://x",
            "[::1",
            "x/y",
        ] {
            assert_eq!(a(bad), None, "{bad}");
        }
    }

    #[test]
    fn legacy_replies() {
        let s = parse_legacy(
            "§1\x0074\x001.6.4\x00§aМой сервер\x003\x0020",
            Duration::from_millis(5),
        )
        .unwrap();
        assert_eq!((s.protocol, s.version.as_str()), (Some(74), "1.6.4"));
        assert_eq!((s.online, s.max), (Some(3), Some(20)));
        assert_eq!(motd::plain(&s.motd), "Мой сервер");
        let s = parse_legacy("A Minecraft Server§0§10", Duration::ZERO).unwrap();
        assert_eq!((s.online, s.max, s.protocol), (Some(0), Some(10), None));
        assert_eq!(motd::plain(&s.motd), "A Minecraft Server");
        assert!(parse_legacy("мусор", Duration::ZERO).is_none());
    }

    fn png64() -> String {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&64u32.to_be_bytes());
        png.extend_from_slice(&64u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0]);
        STANDARD.encode(png)
    }

    #[test]
    fn favicons() {
        let good = png64();
        assert_eq!(
            clean_favicon(&format!("data:image/png;base64,{good}")),
            Some(good.clone())
        );
        // Старые серверы переносили строки в base64.
        let wrapped = format!("data:image/png;base64,{}\n{}", &good[..10], &good[10..]);
        assert_eq!(clean_favicon(&wrapped), Some(good));
        assert_eq!(clean_favicon("data:image/svg+xml;base64,PHN2Zz4="), None);
        assert_eq!(clean_favicon("data:image/png;base64,!!!"), None);
    }

    /// Поддельный сервер на 127.0.0.1: отвечает так, как вернёт `reply`.
    async fn fake(reply: impl Fn(Vec<u8>) -> Vec<u8> + Send + 'static) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = listener.accept().await else {
                    return;
                };
                let mut buf = vec![0; 1024];
                let n = s.read(&mut buf).await.unwrap_or(0);
                buf.truncate(n);
                let out = reply(buf);
                let _ = s.write_all(&out).await;
                // Ответить на пинг тем же, если он придёт; нет — закрыть.
                let mut ping = [0u8; 10];
                let wait = timeout(Duration::from_millis(500), s.read_exact(&mut ping));
                if let Ok(Ok(_)) = wait.await {
                    let _ = s.write_all(&ping).await;
                }
            }
        });
        port
    }

    fn modern_reply(json: &str) -> Vec<u8> {
        let mut body = vec![0x00];
        put_string(&mut body, json);
        let mut out = Vec::new();
        put_varint(&mut out, body.len() as i32);
        out.extend_from_slice(&body);
        out
    }

    #[tokio::test]
    async fn modern_server() {
        let json = serde_json::json!({
            "version": {"name": "Paper 1.21.1", "protocol": 767},
            "players": {"max": 100, "online": 5, "sample": [{"name": "§cAlex", "id": "0"}]},
            "description": {"text": "Привет", "color": "gold"},
            "favicon": format!("data:image/png;base64,{}", png64()),
            "forgeData": {"mods": [{"modId": "a"}, {"modId": "b"}]}
        })
        .to_string();
        let port = fake(move |req| {
            // Приветствие: пакет 0, протокол -1, адрес, порт, «состояние».
            assert_eq!(req[1], 0x00);
            assert!(req.windows(9).any(|w| w == b"127.0.0.1"));
            modern_reply(&json)
        })
        .await;
        let s = ping(&format!("127.0.0.1:{port}")).await.unwrap();
        assert_eq!(s.version, "Paper 1.21.1");
        assert_eq!(s.protocol, Some(767));
        assert_eq!((s.online, s.max), (Some(5), Some(100)));
        assert_eq!(s.sample, vec!["Alex"]);
        assert_eq!(s.motd[0].color.as_deref(), Some("#ffaa00"));
        assert!(s.favicon.is_some());
        assert_eq!(s.modded.unwrap().mods, Some(2));
        let proxy = serde_json::json!({"modinfo": {"type": "FML", "modList": []}, "forgeData": {"mods": []}});
        assert_eq!(modded(&proxy), None);
        assert!(!s.legacy);
    }

    #[tokio::test]
    async fn old_server_falls_back() {
        // Сервер 1.6: на новый запрос выгоняет пакетом 0xFF, на старый отвечает.
        let port = fake(|req| {
            let text = if req.starts_with(&[0xfe, 0x01, 0xfa]) {
                "§1\x0078\x001.6.4\x00Старый\x001\x0010"
            } else {
                "Outdated client!"
            };
            let units: Vec<u16> = text.encode_utf16().collect();
            let mut out = vec![0xff];
            out.extend_from_slice(&(units.len() as u16).to_be_bytes());
            for u in units {
                out.extend_from_slice(&u.to_be_bytes());
            }
            out
        })
        .await;
        let s = ping(&format!("127.0.0.1:{port}")).await.unwrap();
        assert!(s.legacy);
        assert_eq!(s.version, "1.6.4");
        assert_eq!(motd::plain(&s.motd), "Старый");
    }

    #[tokio::test]
    async fn not_a_server() {
        let port = fake(|_| b"HTTP/1.1 400 Bad Request\r\n\r\n".to_vec()).await;
        assert_eq!(
            ping(&format!("127.0.0.1:{port}")).await,
            Err(PingError::Protocol)
        );
        // Порт никто не слушает.
        let free = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = free.local_addr().unwrap().port();
        drop(free);
        assert_eq!(
            ping(&format!("127.0.0.1:{port}")).await,
            Err(PingError::Refused)
        );
        assert_eq!(ping("bad address").await, Err(PingError::BadAddress));
    }
}
