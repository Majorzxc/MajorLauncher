//! Статистика игрового времени (раздел 5.17 ТЗ): сколько играл всего, в какой
//! сборке, в каком мире и на каком сервере, сколько в меню.
//!
//! Где игрок, лаунчер узнаёт из вывода игры, который и так читает, пока ждёт
//! её закрытия: строка запуска встроенного сервера — вошёл в одиночный мир,
//! «Connecting to адрес, порт» — на сервер, «Stopping server» — вышел из мира.
//! С 1.15 игра пишет режим и в заголовке окна («Minecraft 1.21.1 - Одиночная
//! игра»), по нему видно и выход с сервера, о котором лог молчит.
//!
//! Сессии копятся в `stats.jsonl` в папке данных, по строке на сессию: файл
//! только дописывается, а удаление сборки не стирает её время.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{IoContext, Result};
use crate::paths::DataDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaceKind {
    /// Меню и загрузка игры.
    Menu,
    /// Одиночный мир.
    World,
    Server,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceTime {
    pub kind: PlaceKind,
    /// Название мира или адрес сервера; у меню пусто.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Папка мира: название можно сменить в игре, папка остаётся той же.
    /// В сессиях до 10.10.2026 пусто — см. [`resolve_worlds`].
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    pub seconds: u64,
}

impl PlaceTime {
    /// Ключ места для статистики. Мир — это папка внутри сборки (в разных
    /// сборках может быть свой «New World»), сервер — адрес, где бы ни играл.
    pub fn key(&self, build: &str) -> String {
        match self.kind {
            PlaceKind::Menu => "menu".into(),
            PlaceKind::Server => format!("server:{}", self.name),
            PlaceKind::World => format!("world:{build}/{}", self.folder()),
        }
    }

    fn folder(&self) -> &str {
        if self.id.is_empty() {
            &self.name
        } else {
            &self.id
        }
    }
}

/// Где сейчас игрок.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Spot {
    kind: PlaceKind,
    id: String,
    name: String,
}

impl Spot {
    fn menu() -> Self {
        Self {
            kind: PlaceKind::Menu,
            id: String::new(),
            name: String::new(),
        }
    }
}

/// Одна игровая сессия: от запуска игры до её закрытия.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub build: String,
    /// Название сборки на момент игры — чтобы показывать и удалённые сборки.
    pub build_name: String,
    pub version: String,
    /// Секунды с 1970 года.
    pub start: u64,
    pub end: u64,
    #[serde(default)]
    pub places: Vec<PlaceTime>,
}

impl SessionRecord {
    pub fn seconds(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

/// Следит, где сейчас игрок, и копит время по местам.
pub struct Tracker {
    /// Где игрок по логу.
    log_place: Spot,
    /// Режим по заголовку окна: `Some(true)` — в мире или на сервере.
    /// `None` — заголовок ни разу не показывал режим (версии до 1.15).
    title_playing: Option<bool>,
    current: Spot,
    since: u64,
    totals: Vec<PlaceTime>,
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Tracker {
    pub fn new() -> Self {
        Self {
            log_place: Spot::menu(),
            title_playing: None,
            current: Spot::menu(),
            since: 0,
            totals: Vec::new(),
        }
    }

    /// Строка вывода игры. `now` — секунды от запуска; `world` — папка и
    /// название одиночного мира, спрашиваются только при входе в мир.
    pub fn line(&mut self, now: u64, line: &str, world: impl FnOnce() -> (String, String)) {
        let msg = message(line);
        let place = if msg.contains("Starting integrated minecraft server") {
            let (id, name) = world();
            Spot {
                kind: PlaceKind::World,
                id,
                name,
            }
        } else if let Some(address) = server_address(msg) {
            Spot {
                kind: PlaceKind::Server,
                id: String::new(),
                name: address,
            }
        } else if msg.ends_with("Stopping server") {
            Spot::menu()
        } else {
            return;
        };
        self.log_place = place;
        self.update(now);
    }

    /// Заголовок окна игры. В мире он с припиской через « - », в меню — без неё.
    pub fn title(&mut self, now: u64, title: &str) {
        let playing = title.contains(" - ");
        if playing || self.title_playing.is_some() {
            self.title_playing = Some(playing);
        }
        self.update(now);
    }

    fn update(&mut self, now: u64) {
        let next = match self.title_playing {
            Some(false) => Spot::menu(),
            _ => self.log_place.clone(),
        };
        if next != self.current {
            self.close(now);
            self.current = next;
        }
    }

    fn close(&mut self, now: u64) {
        let seconds = now.saturating_sub(self.since);
        self.since = now;
        if seconds == 0 {
            return;
        }
        let c = &self.current;
        match self
            .totals
            .iter_mut()
            .find(|p| p.kind == c.kind && p.id == c.id && p.name == c.name)
        {
            Some(p) => p.seconds += seconds,
            None => self.totals.push(PlaceTime {
                kind: c.kind,
                name: c.name.clone(),
                id: c.id.clone(),
                seconds,
            }),
        }
    }

    /// Игра закрылась: время по местам.
    pub fn finish(mut self, now: u64) -> Vec<PlaceTime> {
        self.close(now);
        self.totals
    }
}

/// Текст сообщения из строки вывода. Новые версии пишут в консоль XML
/// (`<log4j:Message><![CDATA[текст]]></log4j:Message>`), старые — простой текст.
fn message(line: &str) -> &str {
    match line.find("<![CDATA[") {
        Some(start) => {
            let rest = &line[start + 9..];
            rest.find("]]>").map_or(rest, |end| &rest[..end]).trim()
        }
        None => line.trim(),
    }
}

/// Адрес из «Connecting to mc.example.org, 25565». Стандартный порт не пишем.
fn server_address(msg: &str) -> Option<String> {
    let rest = &msg[msg.find("Connecting to ")? + 14..];
    let (host, port) = rest.trim().rsplit_once(", ")?;
    let port: u16 = port.parse().ok()?;
    if host.is_empty() || host.contains(char::is_whitespace) {
        return None;
    }
    let host = host.to_lowercase();
    Some(if port == 25565 {
        host
    } else {
        format!("{host}:{port}")
    })
}

/// Хранилище сессий в папке данных.
pub struct Stats {
    path: PathBuf,
}

impl Stats {
    pub fn new(data: &DataDir) -> Self {
        Self {
            path: data.root().join("stats.jsonl"),
        }
    }

    pub fn append(&self, record: &SessionRecord) -> Result<()> {
        // Если прошлая запись оборвалась на полуслове, начинаем с новой строки,
        // иначе новая сессия склеится с обрывком и тоже пропадёт.
        let torn = std::fs::read(&self.path)
            .ok()
            .and_then(|bytes| bytes.last().copied())
            .is_some_and(|last| last != b'\n');
        let mut line = if torn {
            "\n".to_string()
        } else {
            String::new()
        };
        line += &serde_json::to_string(record).expect("сессия всегда сериализуется");
        line.push('\n');
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .at(&self.path)?;
        file.write_all(line.as_bytes()).at(&self.path)
    }

    /// Все сессии. Повреждённые строки пропускаются: из-за одной битой строки
    /// (например, свет выключили во время записи) статистика не пропадает.
    pub fn load(&self) -> Result<Vec<SessionRecord>> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e).at(&self.path),
        };
        Ok(text
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }
}

/// Что показать на экране статистики.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Filter {
    /// Только одна сборка.
    #[serde(default)]
    pub build: Option<String>,
    /// Только один вид мест: меню, одиночные миры или серверы.
    #[serde(default)]
    pub kind: Option<PlaceKind>,
    /// Только один мир или сервер: ключ из [`PlaceSummary::key`].
    #[serde(default)]
    pub place: Option<String>,
    /// Смещение местного времени от UTC в минутах — чтобы делить время по дням.
    #[serde(default)]
    pub utc_offset_min: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildTime {
    pub id: String,
    pub name: String,
    pub seconds: u64,
    pub sessions: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayTime {
    /// Номер дня с 1970 года по местному времени.
    pub day: i64,
    pub seconds: u64,
}

/// Мир, сервер или меню в сводке.
#[derive(Debug, Clone, Serialize)]
pub struct PlaceSummary {
    /// Чем фильтровать по этому месту.
    pub key: String,
    pub kind: PlaceKind,
    /// Название мира (последнее известное), адрес сервера; у меню пусто.
    pub name: String,
    /// Сборка и папка — у миров.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    /// Время с учётом фильтра.
    pub total: u64,
    pub sessions: u32,
    /// Самая длинная сессия, секунды.
    pub longest: u64,
    /// Первая сессия вообще — «считаем с …».
    pub since: Option<u64>,
    pub builds: Vec<BuildTime>,
    /// Миры, серверы и меню — без фильтра по месту, чтобы его можно было сменить.
    pub places: Vec<PlaceSummary>,
    pub days: Vec<DayTime>,
}

impl Summary {
    /// Показать у миров их нынешние названия: мир могли переименовать в игре
    /// после последней сессии. `worlds(сборка)` — пары «папка, название».
    pub fn rename_worlds(&mut self, mut worlds: impl FnMut(&str) -> Vec<(String, String)>) {
        for p in &mut self.places {
            if let (Some(build), Some(folder)) = (&p.build, &p.folder)
                && let Some((_, name)) = worlds(build).into_iter().find(|(f, _)| f == folder)
            {
                p.name = name;
            }
        }
    }
}

/// Сессии до 10.10.2026 помнят мир только по названию. Если сейчас в сборке
/// есть мир с таким названием или такой папкой — привязываем к его папке,
/// чтобы старое и новое время мира сложились вместе.
pub fn resolve_worlds(
    records: &mut [SessionRecord],
    mut worlds: impl FnMut(&str) -> Vec<(String, String)>,
) {
    for r in records {
        for p in &mut r.places {
            if p.kind != PlaceKind::World || !p.id.is_empty() {
                continue;
            }
            if let Some((folder, _)) = worlds(&r.build)
                .into_iter()
                .find(|(f, n)| *n == p.name || *f == p.name)
            {
                p.id = folder;
            }
        }
    }
}

/// Свести сессии в цифры для экрана статистики.
pub fn summarize(records: &[SessionRecord], filter: &Filter) -> Summary {
    let mut builds: Vec<BuildTime> = Vec::new();
    let mut places: Vec<PlaceSummary> = Vec::new();
    let mut days: HashMap<i64, u64> = HashMap::new();
    let (mut total, mut sessions, mut longest) = (0, 0, 0);

    for r in records {
        if filter.build.as_ref().is_some_and(|b| *b != r.build) {
            continue;
        }
        // Старые сессии без разбивки по местам целиком считаем меню.
        let fallback;
        let session_places = if r.places.is_empty() {
            fallback = [PlaceTime {
                kind: PlaceKind::Menu,
                name: String::new(),
                id: String::new(),
                seconds: r.seconds(),
            }];
            &fallback[..]
        } else {
            &r.places[..]
        };

        let mut seconds = 0;
        for p in session_places {
            let key = p.key(&r.build);
            if filter.kind.is_none_or(|k| k == p.kind) {
                match places.iter_mut().find(|x| x.key == key) {
                    // Название — из последней сессии: мир могли переименовать.
                    Some(x) => {
                        x.seconds += p.seconds;
                        x.name = p.name.clone();
                    }
                    None => {
                        let world = p.kind == PlaceKind::World;
                        places.push(PlaceSummary {
                            key: key.clone(),
                            kind: p.kind,
                            name: p.name.clone(),
                            build: world.then(|| r.build.clone()),
                            build_name: world.then(|| r.build_name.clone()),
                            folder: world.then(|| p.folder().to_string()),
                            seconds: p.seconds,
                        });
                    }
                }
            }
            let kind_ok = filter.kind.is_none_or(|k| k == p.kind);
            let place_ok = filter.place.as_ref().is_none_or(|k| *k == key);
            if kind_ok && place_ok {
                seconds += p.seconds;
            }
        }
        if seconds == 0 {
            continue;
        }
        total += seconds;
        sessions += 1;
        longest = longest.max(seconds);
        let day = (r.start as i64 + filter.utc_offset_min as i64 * 60).div_euclid(86_400);
        *days.entry(day).or_default() += seconds;
        match builds.iter_mut().find(|b| b.id == r.build) {
            Some(b) => {
                b.seconds += seconds;
                b.sessions += 1;
                // Имя — из последней сессии: сборку могли переименовать.
                b.name = r.build_name.clone();
            }
            None => builds.push(BuildTime {
                id: r.build.clone(),
                name: r.build_name.clone(),
                seconds,
                sessions: 1,
            }),
        }
    }

    builds.sort_by_key(|b| std::cmp::Reverse(b.seconds));
    places.sort_by_key(|p| std::cmp::Reverse(p.seconds));
    let mut days: Vec<DayTime> = days
        .into_iter()
        .map(|(day, seconds)| DayTime { day, seconds })
        .collect();
    days.sort_by_key(|d| d.day);
    Summary {
        total,
        sessions,
        longest,
        since: records.iter().map(|r| r.start).min(),
        builds,
        places,
        days,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> (String, String) {
        ("New World".into(), "Мой мир".into())
    }

    fn pt(kind: PlaceKind, name: &str, id: &str, seconds: u64) -> PlaceTime {
        PlaceTime {
            kind,
            name: name.into(),
            id: id.into(),
            seconds,
        }
    }

    #[test]
    fn reads_both_log_formats() {
        let xml =
            r#"  <log4j:Message><![CDATA[Connecting to Mc.Example.org, 25565]]></log4j:Message>"#;
        assert_eq!(message(xml), "Connecting to Mc.Example.org, 25565");
        assert_eq!(
            server_address(message(xml)).as_deref(),
            Some("mc.example.org")
        );
        assert_eq!(
            server_address("[12:00:00] [Server Connector #1/INFO]: Connecting to 127.0.0.1, 25566")
                .as_deref(),
            Some("127.0.0.1:25566")
        );
        assert_eq!(server_address("Connecting to database"), None);
        assert_eq!(server_address("Connecting to my server, now"), None);
    }

    /// Последовательность из настоящего лога 1.21.1 (03.10.2026).
    #[test]
    fn singleplayer_from_log() {
        let mut t = Tracker::new();
        t.line(
            30,
            "[09:22:47] [Server thread/INFO]: Starting integrated minecraft server version 1.21.1",
            world,
        );
        t.line(
            40,
            "[09:22:52] [Server thread/INFO]: Majorzxc joined the game",
            || unreachable!(),
        );
        t.line(
            60,
            "[09:23:18] [Server thread/INFO]: Stopping singleplayer server as player logged out",
            world,
        );
        t.line(
            61,
            "[09:23:18] [Server thread/INFO]: Stopping server",
            world,
        );
        let places = t.finish(100);
        assert_eq!(
            places,
            vec![
                pt(PlaceKind::Menu, "", "", 30 + 39),
                pt(PlaceKind::World, "Мой мир", "New World", 31),
            ]
        );
    }

    /// С 1.15: выход с сервера виден только по заголовку окна.
    #[test]
    fn server_exit_from_title() {
        let mut t = Tracker::new();
        // Пока заголовок ни разу не показал режим, ему не верим (старые версии).
        t.title(5, "Minecraft 1.21.1");
        t.line(
            10,
            "<log4j:Message><![CDATA[Connecting to play.example.org, 25565]]></log4j:Message>",
            world,
        );
        t.title(12, "Minecraft 1.21.1 - Сетевая игра (сторонний сервер)");
        t.title(70, "Minecraft 1.21.1");
        // Не удалось подключиться: заголовок без приписки — значит, в меню.
        t.line(
            80,
            "<log4j:Message><![CDATA[Connecting to down.example.org, 25565]]></log4j:Message>",
            world,
        );
        let places = t.finish(90);
        assert_eq!(
            places,
            vec![
                pt(PlaceKind::Menu, "", "", 10 + 20),
                pt(PlaceKind::Server, "play.example.org", "", 60),
            ]
        );
    }

    /// До 1.15 заголовок всегда «Minecraft 1.12.2»: работаем только по логу.
    #[test]
    fn old_versions_ignore_title() {
        let mut t = Tracker::new();
        t.title(5, "Minecraft 1.12.2");
        t.line(
            10,
            "[10:00:00] [Server thread/INFO]: Starting integrated minecraft server version 1.12.2",
            world,
        );
        t.title(20, "Minecraft 1.12.2");
        let places = t.finish(50);
        assert_eq!(places[1], pt(PlaceKind::World, "Мой мир", "New World", 40));
    }

    fn rec(build: &str, start: u64, places: Vec<(PlaceKind, &str, u64)>) -> SessionRecord {
        let places: Vec<PlaceTime> = places
            .into_iter()
            .map(|(kind, name, seconds)| pt(kind, name, "", seconds))
            .collect();
        let len: u64 = places.iter().map(|p| p.seconds).sum();
        SessionRecord {
            build: build.into(),
            build_name: build.to_uppercase(),
            version: "1.21.1".into(),
            start,
            end: start + len,
            places,
        }
    }

    #[test]
    fn summary_filters() {
        use PlaceKind::*;
        let records = vec![
            rec("a", 0, vec![(Menu, "", 60), (World, "w1", 600)]),
            rec("b", 86_400, vec![(Menu, "", 30), (Server, "s1", 1200)]),
            rec("a", 86_400 * 2, vec![(World, "w1", 100), (World, "w2", 50)]),
            // Сессия до разбивки по местам.
            SessionRecord {
                build: "c".into(),
                build_name: "C".into(),
                version: "1.5.2".into(),
                start: 10,
                end: 70,
                places: vec![],
            },
        ];
        let all = summarize(&records, &Filter::default());
        assert_eq!(all.total, 660 + 1230 + 150 + 60);
        assert_eq!(all.sessions, 4);
        assert_eq!(all.builds[0].id, "b");
        assert_eq!(all.days.len(), 3);

        let worlds = summarize(
            &records,
            &Filter {
                kind: Some(World),
                ..Default::default()
            },
        );
        assert_eq!(worlds.total, 750);
        assert_eq!(worlds.sessions, 2);
        assert_eq!(worlds.places.len(), 2);

        let w1 = summarize(
            &records,
            &Filter {
                kind: Some(World),
                place: Some("world:a/w1".into()),
                ..Default::default()
            },
        );
        assert_eq!(w1.total, 700);
        // Список мест не сужается выбранным местом — чтобы переключаться между ними.
        assert_eq!(w1.places.len(), 2);
        assert_eq!(w1.places[0].build_name.as_deref(), Some("A"));

        let only_a = summarize(
            &records,
            &Filter {
                build: Some("a".into()),
                ..Default::default()
            },
        );
        assert_eq!(only_a.total, 810);
        assert_eq!(only_a.builds.len(), 1);
    }

    /// Просьба автора 10.10.2026: переименовал мир — время не должно делиться.
    #[test]
    fn renamed_world_stays_one_place() {
        use PlaceKind::*;
        let mut records = vec![
            // Старые сессии: мир только по названию.
            rec("a", 0, vec![(World, "New World", 100)]),
            rec("a", 100, vec![(World, "Крепость", 50)]),
            // Новая сессия: с папкой.
            SessionRecord {
                places: vec![pt(World, "Крепость", "New World", 30)],
                ..rec("a", 200, vec![])
            },
            // Свой «New World» в другой сборке — другой мир.
            rec("b", 300, vec![(World, "New World", 7)]),
        ];
        // Сейчас в сборке «a» папка «New World» называется «Замок».
        let worlds = |build: &str| match build {
            "a" => vec![("New World".to_string(), "Замок".to_string())],
            _ => vec![],
        };
        // Без привязки «Крепость» из старой сессии — отдельное место.
        assert_eq!(summarize(&records, &Filter::default()).places.len(), 3);

        resolve_worlds(&mut records, worlds);
        let mut s = summarize(&records, &Filter::default());
        // «Крепость» не нашлась по имени (мир снова переименован) — но
        // «New World» привязан по папке, а новая сессия знает папку сама.
        assert_eq!(s.places.len(), 3);
        s.rename_worlds(worlds);
        let a = s
            .places
            .iter()
            .find(|p| p.key == "world:a/New World")
            .unwrap();
        assert_eq!((a.name.as_str(), a.seconds), ("Замок", 130));
        let b = s
            .places
            .iter()
            .find(|p| p.key == "world:b/New World")
            .unwrap();
        assert_eq!((b.name.as_str(), b.seconds), ("New World", 7));

        // Если старое название совпадает с нынешним — сессии складываются.
        let mut old = vec![
            rec("a", 0, vec![(World, "Замок", 40)]),
            SessionRecord {
                places: vec![pt(World, "Замок", "New World", 2)],
                ..rec("a", 50, vec![])
            },
        ];
        resolve_worlds(&mut old, worlds);
        let s = summarize(&old, &Filter::default());
        assert_eq!(s.places.len(), 1);
        assert_eq!(s.places[0].seconds, 42);
    }

    #[test]
    fn stats_file_skips_broken_lines() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        let stats = Stats::new(&data);
        assert!(stats.load().unwrap().is_empty());
        let r = rec("a", 5, vec![(PlaceKind::Menu, "", 10)]);
        stats.append(&r).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(dir.path().join("stats.jsonl"))
            .unwrap()
            .write_all(b"{\"build\":\"obr")
            .unwrap();
        stats.append(&r).unwrap();
        assert_eq!(stats.load().unwrap(), vec![r.clone(), r]);
    }
}
