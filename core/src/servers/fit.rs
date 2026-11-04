//! Какой сборкой заходить на сервер: та, с которой на нём уже играли, потом —
//! по версии, которую называет сервер («1.21.4», «Paper 1.20.1», «1.8–26.3»).
//! Сервер может и пустить другую версию (ViaVersion), поэтому это подсказка:
//! играть можно любой сборкой.

use std::collections::HashMap;

use serde::Serialize;

use crate::builds::{Entry, QUICK_ID, State};
use crate::loaders::Loader;
use crate::playtime::{PlaceKind, SessionRecord};

/// Игра на сервере по статистике.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Played {
    pub seconds: u64,
    /// Когда играл последний раз, секунды с 1970 года.
    pub last: u64,
    /// Сборка и версия последней игры на нём.
    pub build: String,
    pub build_name: String,
    #[serde(skip)]
    version: String,
}

/// Адрес так, как его пишет статистика: без регистра и без порта 25565.
pub fn key(address: &str) -> String {
    let a = address.trim().to_lowercase();
    a.strip_suffix(":25565").map(str::to_string).unwrap_or(a)
}

/// Время и последняя сборка по каждому серверу (ключ — [`key`]).
pub fn played(records: &[SessionRecord]) -> HashMap<String, Played> {
    let mut out: HashMap<String, Played> = HashMap::new();
    for r in records {
        for p in r.places.iter().filter(|p| p.kind == PlaceKind::Server) {
            let e = out.entry(key(&p.name)).or_default();
            e.seconds += p.seconds;
            if r.end >= e.last {
                e.last = r.end;
                e.build = r.build.clone();
                e.build_name = r.build_name.clone();
                e.version = r.version.clone();
            }
        }
    }
    out
}

/// Релизы игры из сохранённого списка версий — без сети: только чтобы
/// предложить «Быструю игру» нужной версии.
pub fn cached_releases(data: &crate::paths::DataDir) -> Vec<String> {
    std::fs::read(data.cache().join("version_manifest_v2.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<crate::meta::VersionManifest>(&b).ok())
        .map(|m| {
            m.versions
                .into_iter()
                .filter(|v| v.kind == "release")
                .map(|v| v.id)
                .collect()
        })
        .unwrap_or_default()
}

/// Насколько сборка подходит; порядок — от лучшей.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// С ней играли на этом сервере последний раз.
    Last,
    /// Та самая версия, что у сервера.
    Exact,
    /// В диапазоне, который сервер пускает.
    Range,
    /// Сервер не назвал понятной версии.
    Unknown,
    /// Похоже, не пустит.
    No,
}

/// Сборка в списке «Играть на сервере».
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub id: String,
    pub name: String,
    pub version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub fit: Fit,
    /// Почему так: «вы играли на нём с этой сборкой», «сервер на 1.20.1».
    pub note: String,
}

/// Версия по частям; `None` — «x», любая.
type Ver = Vec<Option<u32>>;

fn parse_ver(t: &str) -> Option<Ver> {
    let parts: Vec<Option<u32>> = t
        .split('.')
        .map(|p| {
            if p == "x" {
                Some(None)
            } else {
                p.parse().ok().map(Some)
            }
        })
        .collect::<Option<_>>()?;
    let major = parts.first().copied().flatten()?;
    // 1.x — старая нумерация, 26.x — новая (с 2026); прочие числа — не версии.
    let ok = (2..=3).contains(&parts.len())
        && (major == 1 && parts[1].is_none_or(|m| m <= 40) || (20..=99).contains(&major));
    ok.then_some(parts)
}

/// Как сравнивать недостающие части: снизу — нулями, сверху — «любая».
fn padded(v: &Ver, top: bool) -> [u32; 3] {
    let mut out = [if top { u32::MAX } else { 0 }; 3];
    for (i, p) in v.iter().take(3).enumerate() {
        out[i] = p.unwrap_or(if top { u32::MAX } else { 0 });
    }
    out
}

/// Что пускает сервер, по его словам.
#[derive(Debug, Clone, PartialEq)]
enum Spec {
    Exact(Ver),
    Range(Ver, Option<Ver>),
}

fn spec(text: &str) -> Option<Spec> {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?:^|[^\d.])(\d+(?:\.(?:\d+|x)){1,2})(\+)?").unwrap()
    });
    let mut found: Vec<(Ver, bool)> = RE
        .captures_iter(text)
        .filter_map(|c| Some((parse_ver(&c[1])?, c.get(2).is_some())))
        .collect();
    match found.len() {
        0 => None,
        1 => {
            let (v, plus) = found.remove(0);
            Some(if plus {
                Spec::Range(v, None)
            } else if v.iter().any(Option::is_none) {
                // «1.21.x» — любая 1.21.
                Spec::Range(v.clone(), Some(v))
            } else {
                Spec::Exact(v)
            })
        }
        _ => {
            let lo = found
                .iter()
                .map(|(v, _)| v)
                .min_by_key(|v| padded(v, false))?;
            let hi = found
                .iter()
                .map(|(v, _)| v)
                .max_by_key(|v| padded(v, true))?;
            Some(Spec::Range(lo.clone(), Some(hi.clone())))
        }
    }
}

fn show(v: &Ver) -> String {
    v.iter()
        .map(|p| p.map_or("x".to_string(), |n| n.to_string()))
        .collect::<Vec<_>>()
        .join(".")
}

fn fits(spec: &Spec, version: &str) -> Option<bool> {
    let v = padded(&parse_ver(version)?, false);
    Some(match spec {
        Spec::Exact(s) => padded(s, false) == v,
        Spec::Range(lo, hi) => {
            padded(lo, false) <= v && hi.as_ref().is_none_or(|h| v <= padded(h, true))
        }
    })
}

fn spec_text(spec: &Spec) -> String {
    match spec {
        Spec::Exact(v) => format!("сервер на {}", show(v)),
        Spec::Range(lo, Some(hi)) if lo == hi => format!("сервер на {}", show(lo)),
        Spec::Range(lo, Some(hi)) => format!("сервер пускает {}–{}", show(lo), show(hi)),
        Spec::Range(lo, None) => format!("сервер пускает {} и новее", show(lo)),
    }
}

/// Сборки для входа на сервер, лучшие сверху. `version` — что назвал сервер
/// при опросе, `modded` — на нём моды Forge, `releases` — известные релизы
/// игры (для «Быстрой игры» нужной версии).
pub fn choices(
    entries: &[Entry],
    version: Option<&str>,
    modded: bool,
    played: Option<&Played>,
    releases: &[String],
) -> Vec<Choice> {
    let spec = version.and_then(spec);
    let judge = |id: &str, game: &str, loader: Loader| -> (Fit, String) {
        if played.is_some_and(|p| p.build == id) {
            return (Fit::Last, "вы играли на нём с этой сборкой".into());
        }
        if modded && !matches!(loader, Loader::Forge | Loader::NeoForge) {
            return (
                Fit::No,
                "на сервере моды Forge — нужна сборка с ними".into(),
            );
        }
        match spec.as_ref().map(|s| (s, fits(s, game))) {
            Some((s, Some(true))) => (
                if matches!(s, Spec::Exact(_)) {
                    Fit::Exact
                } else {
                    Fit::Range
                },
                spec_text(s),
            ),
            Some((s, Some(false))) => (Fit::No, format!("{} — может не пустить", spec_text(s))),
            _ => (Fit::Unknown, String::new()),
        }
    };
    // До 1.6 игра не заходит на сервер при запуске — честно говорим об этом.
    let old = |v: &str| match parse_ver(v) {
        Some(p) => p[0] == Some(1) && p[1].is_some_and(|m| m < 6),
        None => ["a", "b", "c", "inf", "rd"]
            .iter()
            .any(|p| v.starts_with(p)),
    };
    let with_old = |version: &str, (fit, note): (Fit, String)| {
        if !old(version) {
            return (fit, note);
        }
        const OLD: &str = "сервер придётся выбрать в игре: до 1.6 сразу не зайти";
        (
            fit,
            if note.is_empty() {
                OLD.into()
            } else {
                format!("{note}; {OLD}")
            },
        )
    };
    let mut out: Vec<(Choice, u64)> = entries
        .iter()
        .filter(|e| e.state == State::Active && e.instance.id != QUICK_ID)
        .map(|e| {
            let i = &e.instance;
            let (fit, note) = with_old(&i.version, judge(&i.id, &i.version, i.loader));
            let choice = Choice {
                id: i.id.clone(),
                name: i.name.clone(),
                version: i.version.clone(),
                loader: i.loader,
                loader_version: i.loader_version.clone(),
                fit,
                note,
            };
            (choice, i.last_played.unwrap_or(0))
        })
        .collect();

    // «Быстрая игра» — ванильный сервер без отдельной сборки: нужной версии
    // или той, с которой уже заходили.
    let quick = match (played, &spec) {
        (Some(p), _) if p.build == QUICK_ID && !p.version.is_empty() => Some(p.version.clone()),
        (_, Some(Spec::Exact(v))) if !modded => Some(show(v)).filter(|v| releases.contains(v)),
        _ => None,
    };
    if let Some(v) = quick {
        let (fit, note) = judge(QUICK_ID, &v, Loader::Vanilla);
        let note = if note.is_empty() {
            "без отдельной сборки".into()
        } else {
            note
        };
        out.push((
            Choice {
                id: QUICK_ID.into(),
                name: "Быстрая игра".into(),
                version: v,
                loader: Loader::Vanilla,
                loader_version: None,
                fit,
                note,
            },
            0,
        ));
    }
    // Лучшие сверху, среди равных — недавние.
    out.sort_by(|(a, at), (b, bt)| a.fit.cmp(&b.fit).then(bt.cmp(at)));
    out.into_iter().map(|(c, _)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builds::Instance;
    use crate::playtime::PlaceTime;

    fn entry(id: &str, version: &str, loader: Loader, last: u64) -> Entry {
        let mut i: Instance = serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "version": version, "created": 0
        }))
        .unwrap();
        i.loader = loader;
        i.last_played = Some(last);
        Entry {
            instance: i,
            state: State::Active,
            has_cover: false,
            archive_size: None,
            mods: None,
        }
    }

    fn ids(c: &[Choice]) -> Vec<(&str, Fit)> {
        c.iter().map(|c| (c.id.as_str(), c.fit)).collect()
    }

    #[test]
    fn server_versions() {
        let ok = |text: &str, v: &str| spec(text).and_then(|s| fits(&s, v));
        assert_eq!(ok("1.21.4", "1.21.4"), Some(true));
        assert_eq!(ok("Paper 1.21.4", "1.21.1"), Some(false));
        assert_eq!(ok("1.21", "1.21.0"), Some(true));
        assert_eq!(ok("Velocity 1.7.2-26.3", "1.12.2"), Some(true));
        assert_eq!(ok("Velocity 1.7.2-26.3", "26.3.1"), Some(true));
        assert_eq!(ok("BotFilter 1.8.x-26.x", "26.1.2"), Some(true));
        assert_eq!(ok("We support: 1.20-1.21", "1.21.10"), Some(true));
        assert_eq!(ok("We support: 1.20-1.21", "1.19.4"), Some(false));
        assert_eq!(ok("Requires MC 1.8 / 1.21", "1.12.2"), Some(true));
        assert_eq!(ok("1.20.x", "1.20.6"), Some(true));
        assert_eq!(ok("1.20.x", "1.21"), Some(false));
        assert_eq!(ok("1.16+", "1.21.1"), Some(true));
        assert_eq!(ok("1.16+", "1.12.2"), Some(false));
        // Не версии: адреса, годы, мусор.
        assert_eq!(spec("Мой сервер 2026.11"), None);
        assert_eq!(spec("192.168.1.1"), None);
        assert_eq!(spec("§cОбслуживание"), None);
        assert_eq!(ok("1.21.4", "25w14a"), None);
        assert_eq!(ok("Wizards v1.21.4", "1.21.4"), Some(true));
    }

    #[test]
    fn picks_builds() {
        let entries = vec![
            entry("old", "1.12.2", Loader::Forge, 50),
            entry("new", "1.21.4", Loader::Fabric, 10),
            entry("new2", "1.21.4", Loader::Vanilla, 20),
            entry(QUICK_ID, "1.20.1", Loader::Vanilla, 99),
        ];
        let releases = vec!["1.21.4".to_string()];

        // Ванильный 1.21.4: сборки этой версии, «Быстрая игра», остальное ниже.
        let c = choices(&entries, Some("1.21.4"), false, None, &releases);
        assert_eq!(
            ids(&c),
            vec![
                ("new2", Fit::Exact),
                ("new", Fit::Exact),
                (QUICK_ID, Fit::Exact),
                ("old", Fit::No)
            ]
        );
        assert_eq!(c[2].version, "1.21.4");

        // Играли на нём сборкой «old» — она первая, даже если версия другая.
        let records = vec![SessionRecord {
            build: "old".into(),
            build_name: "Старая".into(),
            version: "1.12.2".into(),
            start: 0,
            end: 100,
            places: vec![PlaceTime {
                kind: PlaceKind::Server,
                name: "play.example.org".into(),
                id: String::new(),
                seconds: 70,
            }],
        }];
        let p = played(&records);
        let p = p.get(&key("Play.Example.org:25565")).unwrap();
        assert_eq!((p.seconds, p.build_name.as_str()), (70, "Старая"));
        let c = choices(&entries, Some("1.21.4"), false, Some(p), &releases);
        assert_eq!(c[0].id, "old");
        assert_eq!(c[0].fit, Fit::Last);

        // Сервер с модами Forge: ванильные и Fabric — не подойдут.
        let c = choices(&entries, Some("1.12.2"), true, None, &releases);
        assert_eq!(ids(&c)[0], ("old", Fit::Exact));
        assert!(c.iter().skip(1).all(|c| c.fit == Fit::No));

        // Версия неизвестна — порядок по недавним, без «Быстрой игры».
        let c = choices(&entries, None, false, None, &releases);
        assert_eq!(ids(&c)[0], ("old", Fit::Unknown));
        assert!(c.iter().all(|c| c.id != QUICK_ID));

        let old = vec![entry("b", "1.5.2", Loader::Vanilla, 1)];
        let c = choices(&old, Some("1.5.2"), false, None, &releases);
        assert_eq!(c[0].fit, Fit::Exact);
        assert!(c[0].note.contains("до 1.6"), "{}", c[0].note);
    }
}
