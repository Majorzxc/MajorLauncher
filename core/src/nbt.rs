//! Формат NBT — так игра хранит `level.dat` (сведения о мире, в gzip) и
//! `servers.dat` (список серверов, без сжатия). Здесь только то, что нужно
//! лаунчеру: прочитать дерево целиком и записать его обратно.

use std::io::{Read, Write};

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    /// Тип элементов хранится отдельно: у пустого списка его не узнать по элементам.
    List(u8, Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    pub fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Tag::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i32> {
        match self {
            Tag::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Tag]> {
        match self {
            Tag::List(_, items) => Some(items),
            _ => None,
        }
    }

    fn id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::ByteArray(_) => 7,
            Tag::String(_) => 8,
            Tag::List(..) => 9,
            Tag::Compound(_) => 10,
            Tag::IntArray(_) => 11,
            Tag::LongArray(_) => 12,
        }
    }
}

/// Не даём чужому мусору съесть память: столько элементов в одном массиве
/// или списке и столько уровней вложенности хватит любому настоящему файлу.
const MAX_ITEMS: usize = 16 * 1024 * 1024;
const MAX_DEPTH: usize = 512;

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let out = self.bytes.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(out)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn arr<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }
    fn len(&mut self) -> Option<usize> {
        let n = i32::from_be_bytes(self.arr()?);
        usize::try_from(n).ok().filter(|n| *n <= MAX_ITEMS)
    }
    fn string(&mut self) -> Option<String> {
        let n = u16::from_be_bytes(self.arr()?) as usize;
        // Строки в Java «модифицированной» UTF-8; для имён миров и серверов
        // обычная UTF-8 почти всегда совпадает.
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }

    fn payload(&mut self, id: u8, depth: usize) -> Option<Tag> {
        if depth > MAX_DEPTH {
            return None;
        }
        Some(match id {
            1 => Tag::Byte(self.u8()? as i8),
            2 => Tag::Short(i16::from_be_bytes(self.arr()?)),
            3 => Tag::Int(i32::from_be_bytes(self.arr()?)),
            4 => Tag::Long(i64::from_be_bytes(self.arr()?)),
            5 => Tag::Float(f32::from_be_bytes(self.arr()?)),
            6 => Tag::Double(f64::from_be_bytes(self.arr()?)),
            7 => {
                let n = self.len()?;
                Tag::ByteArray(self.take(n)?.iter().map(|b| *b as i8).collect())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let item = self.u8()?;
                let n = self.len()?;
                let mut items = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    items.push(self.payload(item, depth + 1)?);
                }
                Tag::List(item, items)
            }
            10 => {
                let mut items = Vec::new();
                loop {
                    let id = self.u8()?;
                    if id == 0 {
                        break;
                    }
                    let name = self.string()?;
                    items.push((name, self.payload(id, depth + 1)?));
                }
                Tag::Compound(items)
            }
            11 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    v.push(i32::from_be_bytes(self.arr()?));
                }
                Tag::IntArray(v)
            }
            12 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    v.push(i64::from_be_bytes(self.arr()?));
                }
                Tag::LongArray(v)
            }
            _ => return None,
        })
    }
}

/// Корневой тег (всегда составной) из несжатых байтов.
pub fn parse(bytes: &[u8]) -> Option<Tag> {
    let mut r = Reader { bytes, at: 0 };
    if r.u8()? != 10 {
        return None;
    }
    r.string()?;
    r.payload(10, 0)
}

/// Прочитать файл: сжатый gzip (level.dat) или нет (servers.dat).
pub fn read_file(path: &std::path::Path) -> Option<Tag> {
    let raw = std::fs::read(path).ok()?;
    if raw.starts_with(&[0x1f, 0x8b]) {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(&raw[..])
            .take(64 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .ok()?;
        parse(&bytes)
    } else {
        parse(&raw)
    }
}

/// Записать корневой составной тег без сжатия (так хранится servers.dat).
pub fn write(root: &Tag) -> Vec<u8> {
    let mut out = vec![10, 0, 0];
    write_payload(&mut out, root);
    out
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    let n = bytes.len().min(u16::MAX as usize);
    out.extend_from_slice(&(n as u16).to_be_bytes());
    out.extend_from_slice(&bytes[..n]);
}

fn write_payload(out: &mut Vec<u8>, tag: &Tag) {
    match tag {
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::ByteArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            out.extend(v.iter().map(|b| *b as u8));
        }
        Tag::String(s) => write_string(out, s),
        Tag::List(item, items) => {
            out.push(if items.is_empty() {
                *item
            } else {
                items[0].id()
            });
            out.extend_from_slice(&(items.len() as i32).to_be_bytes());
            for t in items {
                write_payload(out, t);
            }
        }
        Tag::Compound(items) => {
            for (name, t) in items {
                out.push(t.id());
                write_string(out, name);
                write_payload(out, t);
            }
            out.push(0);
        }
        Tag::IntArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
    }
}

/// Записать файл целиком через временный: оборванная запись не испортит его.
pub fn write_file(path: &std::path::Path, root: &Tag) -> std::io::Result<()> {
    let tmp = path.with_extension("dat.tmp");
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(&write(root))?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let root = Tag::Compound(vec![
            (
                "servers".into(),
                Tag::List(
                    10,
                    vec![Tag::Compound(vec![
                        ("name".into(), Tag::String("Мой сервер".into())),
                        ("ip".into(), Tag::String("mc.example.org".into())),
                        ("acceptTextures".into(), Tag::Byte(1)),
                    ])],
                ),
            ),
            ("empty".into(), Tag::List(8, vec![])),
            ("nums".into(), Tag::LongArray(vec![1, -2])),
            ("pi".into(), Tag::Double(3.5)),
        ]);
        let bytes = write(&root);
        let back = parse(&bytes).unwrap();
        assert_eq!(back, root);
        let first = &back.get("servers").unwrap().as_list().unwrap()[0];
        assert_eq!(
            first.get("ip").and_then(Tag::as_str),
            Some("mc.example.org")
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse(b"").is_none());
        assert!(parse(b"\x0a\x00\x00\x09\x00\x01a\x0a\x7f\xff\xff\xff").is_none());
        assert!(parse(b"\x08\x00\x00").is_none());
    }
}
