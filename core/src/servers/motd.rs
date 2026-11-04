//! Описание сервера (MOTD) и прочий текст из ответа: JSON-компоненты игры и
//! старые коды `§`. Окно получает готовые куски с цветом и начертанием —
//! никакого HTML от сервера.

use serde::Serialize;
use serde_json::Value;

/// Кусок текста одного вида. Перевод строки — внутри `text`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Span {
    pub text: String,
    /// `#rrggbb`; нет — цвет текста по умолчанию.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub underlined: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub strikethrough: bool,
    /// «Мельтешащий» текст (`§k`): игра подменяет буквы случайными.
    #[serde(skip_serializing_if = "is_false")]
    pub obfuscated: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Style {
    color: Option<String>,
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
    obfuscated: bool,
}

/// Столько символов хватит любому описанию: больше — мусор или шутка.
const MAX_TEXT: usize = 4096;
const MAX_DEPTH: usize = 32;

/// 16 цветов игры: имя и код после `§`.
const COLORS: [(&str, char, &str); 16] = [
    ("black", '0', "#000000"),
    ("dark_blue", '1', "#0000aa"),
    ("dark_green", '2', "#00aa00"),
    ("dark_aqua", '3', "#00aaaa"),
    ("dark_red", '4', "#aa0000"),
    ("dark_purple", '5', "#aa00aa"),
    ("gold", '6', "#ffaa00"),
    ("gray", '7', "#aaaaaa"),
    ("dark_gray", '8', "#555555"),
    ("blue", '9', "#5555ff"),
    ("green", 'a', "#55ff55"),
    ("aqua", 'b', "#55ffff"),
    ("red", 'c', "#ff5555"),
    ("light_purple", 'd', "#ff55ff"),
    ("yellow", 'e', "#ffff55"),
    ("white", 'f', "#ffffff"),
];

fn named_color(name: &str) -> Option<String> {
    if let Some(hex) = name.strip_prefix('#') {
        return (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| format!("#{}", hex.to_lowercase()));
    }
    COLORS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, _, hex)| hex.to_string())
}

struct Builder {
    spans: Vec<Span>,
    len: usize,
}

impl Builder {
    fn push(&mut self, text: &str, style: &Style) {
        let room = MAX_TEXT.saturating_sub(self.len);
        if text.is_empty() || room == 0 {
            return;
        }
        let text: String = text.chars().filter(|c| *c != '\r').take(room).collect();
        self.len += text.chars().count();
        if let Some(last) = self.spans.last_mut()
            && last.color == style.color
            && last.bold == style.bold
            && last.italic == style.italic
            && last.underlined == style.underlined
            && last.strikethrough == style.strikethrough
            && last.obfuscated == style.obfuscated
        {
            last.text.push_str(&text);
            return;
        }
        self.spans.push(Span {
            text,
            color: style.color.clone(),
            bold: style.bold,
            italic: style.italic,
            underlined: style.underlined,
            strikethrough: style.strikethrough,
            obfuscated: style.obfuscated,
        });
    }

    /// Строка с кодами `§`: они действуют внутри неё, `§r` возвращает к
    /// виду, с которым строка началась.
    fn legacy(&mut self, text: &str, base: &Style) {
        let mut style = base.clone();
        let mut chunk = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c != '§' {
                chunk.push(c);
                continue;
            }
            let Some(code) = chars.next().map(|c| c.to_ascii_lowercase()) else {
                break;
            };
            self.push(&chunk, &style);
            chunk.clear();
            match code {
                'k' => style.obfuscated = true,
                'l' => style.bold = true,
                'm' => style.strikethrough = true,
                'n' => style.underlined = true,
                'o' => style.italic = true,
                'r' => style = base.clone(),
                c => {
                    if let Some((_, _, hex)) = COLORS.iter().find(|(_, code, _)| *code == c) {
                        // Цвет сбрасывает начертание — как в игре.
                        style = Style {
                            color: Some(hex.to_string()),
                            ..Style::default()
                        };
                    }
                }
            }
        }
        self.push(&chunk, &style);
    }

    fn component(&mut self, v: &Value, parent: &Style, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        match v {
            Value::String(s) => self.legacy(s, parent),
            Value::Number(n) => self.push(&n.to_string(), parent),
            Value::Bool(b) => self.push(&b.to_string(), parent),
            // Список — первый элемент и остальные как его продолжение.
            Value::Array(items) => {
                for item in items {
                    self.component(item, parent, depth + 1);
                }
            }
            Value::Object(o) => {
                let mut style = parent.clone();
                if let Some(c) = o.get("color").and_then(Value::as_str).and_then(named_color) {
                    style.color = Some(c);
                }
                let flag =
                    |name: &str, was: bool| o.get(name).and_then(Value::as_bool).unwrap_or(was);
                style.bold = flag("bold", style.bold);
                style.italic = flag("italic", style.italic);
                style.underlined = flag("underlined", style.underlined);
                style.strikethrough = flag("strikethrough", style.strikethrough);
                style.obfuscated = flag("obfuscated", style.obfuscated);
                match o.get("text") {
                    Some(Value::String(s)) => self.legacy(s, &style),
                    Some(other @ (Value::Number(_) | Value::Bool(_))) => {
                        self.component(other, &style, depth + 1)
                    }
                    _ => {
                        // Перевода у лаунчера нет — показываем ключ, а не пустоту.
                        if let Some(key) = o.get("translate").and_then(Value::as_str) {
                            let fallback = o.get("fallback").and_then(Value::as_str);
                            self.legacy(fallback.unwrap_or(key), &style);
                        }
                    }
                }
                if let Some(extra) = o.get("extra") {
                    self.component(extra, &style, depth + 1);
                }
            }
            Value::Null => {}
        }
    }
}

/// Текст в формате игры: JSON-компонент или строка с кодами `§`.
pub fn from_json(v: &Value) -> Vec<Span> {
    let mut b = Builder {
        spans: Vec::new(),
        len: 0,
    };
    b.component(v, &Style::default(), 0);
    b.spans
}

/// Строка с кодами `§` (старые серверы, имена в списке игроков).
pub fn from_legacy(text: &str) -> Vec<Span> {
    from_json(&Value::String(text.to_string()))
}

/// Только текст, без цветов.
pub fn plain(spans: &[Span]) -> String {
    spans.iter().map(|s| s.text.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_codes() {
        let s = from_legacy("§aЗелёный §lжирный§r обычный §x?§9синий");
        assert_eq!(
            s,
            vec![
                Span {
                    text: "Зелёный ".into(),
                    color: Some("#55ff55".into()),
                    ..Default::default()
                },
                Span {
                    text: "жирный".into(),
                    color: Some("#55ff55".into()),
                    bold: true,
                    ..Default::default()
                },
                Span {
                    text: " обычный ?".into(),
                    ..Default::default()
                },
                Span {
                    text: "синий".into(),
                    color: Some("#5555ff".into()),
                    ..Default::default()
                },
            ]
        );
        assert_eq!(plain(&from_legacy("§cA§")), "A");
    }

    #[test]
    fn components() {
        let v = json!({
            "text": "",
            "extra": [
                {"text": "Hypixel ", "color": "green", "bold": true},
                {"text": "Network", "color": "#FF0000", "extra": [" §b[1.8-1.21]"]},
                "\n",
                {"translate": "multiplayer.status.unknown"},
                {"text": "x", "color": "javascript:alert(1)"}
            ]
        });
        let s = from_json(&v);
        assert_eq!(s[0].text, "Hypixel ");
        assert!(s[0].bold);
        assert_eq!(s[1].color.as_deref(), Some("#ff0000"));
        assert_eq!(s[1].text, "Network ");
        assert_eq!(s[2].text, "[1.8-1.21]");
        assert_eq!(s[2].color.as_deref(), Some("#55ffff"));
        assert_eq!(
            plain(&s),
            "Hypixel Network [1.8-1.21]\nmultiplayer.status.unknownx"
        );
        // Чужое значение цвета не проходит.
        assert_eq!(s.last().unwrap().color, None);
        assert_eq!(plain(&from_json(&json!(["a", ["b", {"text": 5}]]))), "ab5");
    }

    #[test]
    fn limits() {
        let deep = (0..100).fold(json!("x"), |v, _| json!({"text": "", "extra": [v]}));
        assert_eq!(plain(&from_json(&deep)), "");
        let long = "a".repeat(10_000);
        assert_eq!(plain(&from_legacy(&long)).len(), MAX_TEXT);
    }
}
