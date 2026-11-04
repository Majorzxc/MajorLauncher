//! Клавиши в options.txt: с 1.13 игра пишет имена (`key.keyboard.w`,
//! `key.mouse.left`), до 1.13 — числовые коды LWJGL 2 (`17`, `-100`). Общий
//! профиль хранит имена; для старых сборок лаунчер переводит их в коды и
//! обратно, чтобы клавиши были одни на все сборки.

/// Клавиатура: имя после `key.keyboard.` и код LWJGL 2.
const KEYS: &[(&str, i32)] = &[
    ("escape", 1),
    ("1", 2),
    ("2", 3),
    ("3", 4),
    ("4", 5),
    ("5", 6),
    ("6", 7),
    ("7", 8),
    ("8", 9),
    ("9", 10),
    ("0", 11),
    ("minus", 12),
    ("equal", 13),
    ("backspace", 14),
    ("tab", 15),
    ("q", 16),
    ("w", 17),
    ("e", 18),
    ("r", 19),
    ("t", 20),
    ("y", 21),
    ("u", 22),
    ("i", 23),
    ("o", 24),
    ("p", 25),
    ("left.bracket", 26),
    ("right.bracket", 27),
    ("enter", 28),
    ("left.control", 29),
    ("a", 30),
    ("s", 31),
    ("d", 32),
    ("f", 33),
    ("g", 34),
    ("h", 35),
    ("j", 36),
    ("k", 37),
    ("l", 38),
    ("semicolon", 39),
    ("apostrophe", 40),
    ("grave.accent", 41),
    ("left.shift", 42),
    ("backslash", 43),
    ("z", 44),
    ("x", 45),
    ("c", 46),
    ("v", 47),
    ("b", 48),
    ("n", 49),
    ("m", 50),
    ("comma", 51),
    ("period", 52),
    ("slash", 53),
    ("right.shift", 54),
    ("keypad.multiply", 55),
    ("left.alt", 56),
    ("space", 57),
    ("caps.lock", 58),
    ("f1", 59),
    ("f2", 60),
    ("f3", 61),
    ("f4", 62),
    ("f5", 63),
    ("f6", 64),
    ("f7", 65),
    ("f8", 66),
    ("f9", 67),
    ("f10", 68),
    ("num.lock", 69),
    ("scroll.lock", 70),
    ("keypad.7", 71),
    ("keypad.8", 72),
    ("keypad.9", 73),
    ("keypad.subtract", 74),
    ("keypad.4", 75),
    ("keypad.5", 76),
    ("keypad.6", 77),
    ("keypad.add", 78),
    ("keypad.1", 79),
    ("keypad.2", 80),
    ("keypad.3", 81),
    ("keypad.0", 82),
    ("keypad.decimal", 83),
    ("f11", 87),
    ("f12", 88),
    ("f13", 100),
    ("f14", 101),
    ("f15", 102),
    ("keypad.equal", 141),
    ("keypad.enter", 156),
    ("right.control", 157),
    ("keypad.divide", 181),
    ("print.screen", 183),
    ("right.alt", 184),
    ("pause", 197),
    ("home", 199),
    ("up", 200),
    ("page.up", 201),
    ("left", 203),
    ("right", 205),
    ("end", 207),
    ("down", 208),
    ("page.down", 209),
    ("insert", 210),
    ("delete", 211),
    ("left.win", 219),
    ("right.win", 220),
];

/// Действие игры для редактора клавиш: строка `key_<id>` в options.txt,
/// подпись и клавиша по умолчанию.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    pub default: &'static str,
}

const fn a(id: &'static str, label: &'static str, default: &'static str) -> Action {
    Action { id, label, default }
}

/// Действия ванильной игры (как в «Управление → Привязки клавиш»). Действия
/// модов редактор показывает по их id.
pub static ACTIONS: &[Action] = &[
    a("key.forward", "Вперёд", "key.keyboard.w"),
    a("key.back", "Назад", "key.keyboard.s"),
    a("key.left", "Влево", "key.keyboard.a"),
    a("key.right", "Вправо", "key.keyboard.d"),
    a("key.jump", "Прыжок", "key.keyboard.space"),
    a("key.sneak", "Красться", "key.keyboard.left.shift"),
    a("key.sprint", "Бег", "key.keyboard.left.control"),
    a("key.attack", "Атаковать / разрушить", "key.mouse.left"),
    a("key.use", "Использовать / поставить", "key.mouse.right"),
    a("key.pickItem", "Выбрать блок", "key.mouse.middle"),
    a("key.drop", "Выбросить предмет", "key.keyboard.q"),
    a("key.swapOffhand", "Во вторую руку", "key.keyboard.f"),
    a("key.inventory", "Инвентарь", "key.keyboard.e"),
    a("key.chat", "Чат", "key.keyboard.t"),
    a("key.command", "Команда", "key.keyboard.slash"),
    a("key.playerlist", "Список игроков", "key.keyboard.tab"),
    a("key.advancements", "Достижения", "key.keyboard.l"),
    a(
        "key.socialInteractions",
        "Взаимодействие с игроками",
        "key.keyboard.p",
    ),
    a("key.screenshot", "Скриншот", "key.keyboard.f2"),
    a("key.togglePerspective", "Смена вида", "key.keyboard.f5"),
    a("key.smoothCamera", "Плавная камера", "key.keyboard.unknown"),
    a("key.fullscreen", "Полный экран", "key.keyboard.f11"),
    a(
        "key.spectatorOutlines",
        "Подсветка игроков (наблюдатель)",
        "key.keyboard.unknown",
    ),
    a(
        "key.saveToolbarActivator",
        "Сохранить панель",
        "key.keyboard.c",
    ),
    a(
        "key.loadToolbarActivator",
        "Загрузить панель",
        "key.keyboard.x",
    ),
    a("key.hotbar.1", "Ячейка 1", "key.keyboard.1"),
    a("key.hotbar.2", "Ячейка 2", "key.keyboard.2"),
    a("key.hotbar.3", "Ячейка 3", "key.keyboard.3"),
    a("key.hotbar.4", "Ячейка 4", "key.keyboard.4"),
    a("key.hotbar.5", "Ячейка 5", "key.keyboard.5"),
    a("key.hotbar.6", "Ячейка 6", "key.keyboard.6"),
    a("key.hotbar.7", "Ячейка 7", "key.keyboard.7"),
    a("key.hotbar.8", "Ячейка 8", "key.keyboard.8"),
    a("key.hotbar.9", "Ячейка 9", "key.keyboard.9"),
];

/// Такую клавишу игра понимает: из таблицы, «не назначена» или кнопка мыши.
pub fn is_known(name: &str) -> bool {
    to_legacy(name).is_some()
}

/// Мышь до 1.13: −100 — левая кнопка, −99 — правая, −98 — средняя, дальше —
/// боковые. С 1.13: `key.mouse.left|right|middle|4|5…`.
const MOUSE_BASE: i32 = -100;

/// Имя клавиши (`key.keyboard.w`) → код старых версий. Нет такой — `None`.
pub fn to_legacy(name: &str) -> Option<i32> {
    if name == "key.keyboard.unknown" {
        return Some(0);
    }
    if let Some(key) = name.strip_prefix("key.keyboard.") {
        return KEYS.iter().find(|(n, _)| *n == key).map(|(_, c)| *c);
    }
    let button = match name.strip_prefix("key.mouse.")? {
        "left" => 0,
        "right" => 1,
        "middle" => 2,
        n => n.parse::<i32>().ok().filter(|b| (4..=16).contains(b))? - 1,
    };
    Some(MOUSE_BASE + button)
}

/// Код старых версий → имя клавиши, как пишут новые версии.
pub fn from_legacy(code: &str) -> Option<String> {
    let code: i32 = code.trim().parse().ok()?;
    if code == 0 {
        return Some("key.keyboard.unknown".into());
    }
    if (MOUSE_BASE..MOUSE_BASE + 16).contains(&code) {
        let button = code - MOUSE_BASE;
        return Some(match button {
            0 => "key.mouse.left".into(),
            1 => "key.mouse.right".into(),
            2 => "key.mouse.middle".into(),
            b => format!("key.mouse.{}", b + 1),
        });
    }
    KEYS.iter()
        .find(|(_, c)| *c == code)
        .map(|(n, _)| format!("key.keyboard.{n}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for (name, code) in [
            ("key.keyboard.w", 17),
            ("key.keyboard.left.shift", 42),
            ("key.keyboard.space", 57),
            ("key.keyboard.f5", 63),
            ("key.keyboard.unknown", 0),
            ("key.mouse.left", -100),
            ("key.mouse.right", -99),
            ("key.mouse.middle", -98),
            ("key.mouse.4", -97),
            ("key.mouse.5", -96),
        ] {
            assert_eq!(to_legacy(name), Some(code), "{name}");
            assert_eq!(
                from_legacy(&code.to_string()).as_deref(),
                Some(name),
                "{code}"
            );
        }
        assert_eq!(to_legacy("key.keyboard.menu"), None);
        assert_eq!(from_legacy("abc"), None);
        assert_eq!(from_legacy("999"), None);
        // Коды в таблице не повторяются.
        let mut codes: Vec<i32> = KEYS.iter().map(|(_, c)| *c).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), KEYS.len());
        for a in ACTIONS {
            assert!(is_known(a.default), "{}", a.id);
        }
    }
}
