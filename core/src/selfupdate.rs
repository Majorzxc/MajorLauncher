//! Обновления самого лаунчера (раздел 5.13 ТЗ) — с GitHub Releases, без своих
//! серверов. Установщик собирает GitHub Actions (`.github/workflows/release.yml`),
//! рядом кладёт `update.json` — версию, имя файла, размер и SHA-256 — и его
//! подпись Ed25519. Лаунчер берёт обновление, только если подпись сходится с
//! вшитым ключом, версия в подписанном описании новее текущей и скачанный
//! файл совпал по размеру и хешу. Подменённый релиз или файл не ставится.
//!
//! Подписать выпуск: `mlcli release sign --key <ключ> <установщик>`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::download::{Downloader, Task};
use crate::error::{Error, IoContext, Result, json};

/// Выпуски лаунчера: черновики не видны, бета — с пометкой `prerelease`.
pub const RELEASES_URL: &str =
    "https://api.github.com/repos/Majorzxc/MajorLauncher/releases?per_page=20";

/// Открытый ключ подписи выпусков (Ed25519, base64). Закрытый — в секрете
/// `UPDATE_SIGNING_KEY` репозитория, им подписывает только сборка релиза.
pub const PUBLIC_KEY: &str = "4CD5HSfY9iLM1g7Rs0MyRmbqTHcsdJbp4rJ+7vl9ek4=";

/// Описание выпуска и его подпись — файлы рядом с установщиком.
pub const MANIFEST: &str = "update.json";
pub const SIGNATURE: &str = "update.json.sig";

/// Каналы выпусков (раздел 5.16 ТЗ).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    /// Ещё и пробные версии (`1.1.0-beta.1`).
    Beta,
}

/// Подписанное описание выпуска (`update.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Всегда «MajorLauncher»: подпись другого файла этим ключом сюда не подойдёт.
    pub product: String,
    pub version: String,
    /// Имя установщика среди файлов выпуска.
    pub file: String,
    pub size: u64,
    pub sha256: String,
}

const PRODUCT: &str = "MajorLauncher";

impl Manifest {
    /// Описание установщика: размер и хеш считаются по самому файлу.
    pub fn for_file(path: &Path, version: &str) -> Result<Self> {
        Version::parse(version)
            .ok_or_else(|| Error::Other(format!("«{version}» — не номер версии")))?;
        let bytes = std::fs::read(path).at(path)?;
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::Other(format!("{}: нет имени файла", path.display())))?
            .to_string();
        Ok(Self {
            product: PRODUCT.into(),
            version: version.into(),
            file,
            size: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
        })
    }
}

/// Подписать выпуск (`mlcli release sign`): описание установщика, его байты
/// для `update.json` и подпись для `update.json.sig`.
pub fn sign_release(
    installer: &Path,
    version: &str,
    pkcs8: &[u8],
) -> Result<(Manifest, Vec<u8>, String)> {
    let manifest = Manifest::for_file(installer, version)?;
    let bytes = serde_json::to_vec_pretty(&manifest).expect("описание выпуска сериализуется");
    let sig = crate::signing::sign(&bytes, pkcs8)?;
    Ok((manifest, bytes, sig))
}

/// Номер версии `1.2.3` или `1.2.3-beta.4`. Выпуск новее пробной версии того же номера.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    core: [u64; 3],
    pre: Vec<String>,
}

impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches('v');
        let (core, pre) = match text.split_once('-') {
            Some((c, p)) => (c, p.split('.').map(str::to_string).collect()),
            None => (text, Vec::new()),
        };
        let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
        let v = [parts.next()??, parts.next()??, parts.next()??];
        if parts.next().is_some() || pre.iter().any(String::is_empty) {
            return None;
        }
        Some(Self { core: v, pre })
    }

    pub fn is_pre(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        self.core
            .cmp(&other.core)
            .then_with(|| match (self.is_pre(), other.is_pre()) {
                (false, false) => Ordering::Equal,
                (false, true) => Ordering::Greater,
                (true, false) => Ordering::Less,
                (true, true) => {
                    for (a, b) in self.pre.iter().zip(&other.pre) {
                        let o = match (a.parse::<u64>(), b.parse::<u64>()) {
                            (Ok(x), Ok(y)) => x.cmp(&y),
                            (Ok(_), Err(_)) => Ordering::Less,
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => a.cmp(b),
                        };
                        if o != Ordering::Equal {
                            return o;
                        }
                    }
                    self.pre.len().cmp(&other.pre.len())
                }
            })
    }
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

impl Release {
    fn asset(&self, name: &str) -> Option<&str> {
        self.assets
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.browser_download_url.as_str())
    }
}

/// Найденное обновление.
#[derive(Debug, Clone, Serialize)]
pub struct Available {
    pub version: String,
    pub beta: bool,
    /// Что нового — текст выпуска на GitHub (Markdown).
    pub notes: String,
    /// Дата выпуска (ISO 8601).
    pub published: Option<String>,
    /// Страница выпуска на GitHub.
    pub page: String,
    pub size: u64,
    /// Откуда качать — окну не нужно, оно ничего не передаёт обратно.
    #[serde(skip)]
    pub url: String,
    #[serde(skip)]
    pub manifest: Manifest,
}

/// Кандидаты по списку выпусков GitHub: подходящие каналу и новее текущей
/// версии, от новых к старым. Подписи здесь ещё не проверены.
fn candidates(
    releases: &[u8],
    channel: Channel,
    current: &Version,
) -> Result<Vec<(Version, Release)>> {
    let list: Vec<Release> = json(releases, "список выпусков GitHub")?;
    let mut out: Vec<(Version, Release)> = list
        .into_iter()
        .filter(|r| !r.draft && (channel == Channel::Beta || !r.prerelease))
        .filter_map(|r| Some((Version::parse(&r.tag_name)?, r)))
        .filter(|(v, r)| {
            v > current && (channel == Channel::Beta || !v.is_pre()) && r.asset(MANIFEST).is_some()
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(out)
}

/// Описание выпуска годится: подпись сходится, это лаунчер, версия та же, что у
/// выпуска на GitHub, и файл установщика в выпуске есть.
fn accept(
    manifest: &[u8],
    sig: &str,
    key: &str,
    version: &Version,
    release: &Release,
) -> Option<(Manifest, String)> {
    if !crate::signing::verify(manifest, sig, key) {
        tracing::warn!("выпуск {}: подпись не сходится", release.tag_name);
        return None;
    }
    let m: Manifest = serde_json::from_slice(manifest).ok()?;
    if m.product != PRODUCT || Version::parse(&m.version).as_ref() != Some(version) {
        tracing::warn!("выпуск {}: описание не от этого выпуска", release.tag_name);
        return None;
    }
    let url = release.asset(&m.file)?.to_string();
    Some((m, url))
}

/// Есть ли версия новее `current`. Выпуск с неверной подписью пропускается.
pub async fn check(dl: &Downloader, channel: Channel, current: &str) -> Result<Option<Available>> {
    let current = Version::parse(current)
        .ok_or_else(|| Error::Other(format!("своя версия «{current}» не разбирается")))?;
    let releases = dl.get_bytes(RELEASES_URL).await?;
    for (version, release) in candidates(&releases, channel, &current)? {
        let (Some(m_url), Some(s_url)) = (release.asset(MANIFEST), release.asset(SIGNATURE)) else {
            continue;
        };
        let (Ok(manifest), Ok(sig)) = (dl.get_bytes(m_url).await, dl.get_bytes(s_url).await) else {
            continue;
        };
        let sig = String::from_utf8_lossy(&sig);
        if let Some((manifest, url)) = accept(&manifest, &sig, PUBLIC_KEY, &version, &release) {
            return Ok(Some(Available {
                version: manifest.version.clone(),
                beta: version.is_pre(),
                notes: release.body.clone().unwrap_or_default(),
                published: release.published_at.clone(),
                page: release.html_url.clone(),
                size: manifest.size,
                url,
                manifest,
            }));
        }
    }
    Ok(None)
}

/// Установщик уже скачан и цел.
pub fn ready(dir: &Path, update: &Available) -> Option<PathBuf> {
    let path = dir.join(&update.manifest.file);
    intact(&path, &update.manifest).then_some(path)
}

fn intact(path: &Path, m: &Manifest) -> bool {
    std::fs::read(path)
        .is_ok_and(|b| b.len() as u64 == m.size && sha256_hex(&b).eq_ignore_ascii_case(&m.sha256))
}

/// Скачать установщик в `dir` (докачка, как у файлов игры) и сверить с подписанным описанием.
pub async fn download(dl: &Downloader, dir: &Path, update: &Available) -> Result<PathBuf> {
    if let Some(path) = ready(dir, update) {
        return Ok(path);
    }
    let path = dir.join(&update.manifest.file);
    let _ = std::fs::remove_file(&path);
    dl.download(&Task {
        url: update.url.clone(),
        path: path.clone(),
        sha1: None,
        size: Some(update.manifest.size),
    })
    .await?;
    if !intact(&path, &update.manifest) {
        let _ = std::fs::remove_file(&path);
        return Err(Error::Other(
            "скачанный установщик не совпал с подписанным описанием — не ставлю".into(),
        ));
    }
    // Старые установщики больше не нужны.
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.path() != path {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Ok(path)
}

/// Запустить установщик: без вопросов (`/P` — только полоса хода), поверх
/// текущей установки (`/UPDATE`), с перезапуском лаунчера (`/R`). Лаунчер после
/// этого должен закрыться сам: установщик закрывает все его копии.
pub fn install(installer: &Path, update: &Available) -> Result<()> {
    // Файл могли подменить, пока он лежал на диске, — проверяем ещё раз.
    if !intact(installer, &update.manifest) {
        return Err(Error::Other(
            "установщик изменился после загрузки — скачайте обновление заново".into(),
        ));
    }
    std::process::Command::new(installer)
        .args(["/P", "/R", "/UPDATE"])
        .spawn()
        .at(installer)?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    digest.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn versions() {
        assert!(v("1.0.0") > v("0.9.9"));
        assert!(v("v1.0.1") > v("1.0.0"));
        assert!(v("1.0.0") > v("1.0.0-beta.2"));
        assert!(v("1.0.0-beta.10") > v("1.0.0-beta.2"));
        assert!(v("1.0.0-rc.1") > v("1.0.0-beta.9"));
        assert!(v("1.0.0-beta.1") > v("1.0.0-beta"));
        assert!(v("1.10.0") > v("1.9.0"));
        assert_eq!(v("1.2.3"), v("v1.2.3"));
        for bad in ["1.0", "1.0.0.0", "x.1.2", "1.0.0-", "1.0.0-a..b", ""] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
    }

    const RELEASES: &str = r#"[
        {"tag_name":"v1.2.0-beta.1","prerelease":true,"html_url":"b","assets":[{"name":"update.json","browser_download_url":"u3"}]},
        {"tag_name":"v1.1.0","html_url":"s","body":"Новое","assets":[{"name":"update.json","browser_download_url":"u2"},{"name":"update.json.sig","browser_download_url":"g2"},{"name":"MajorLauncher_1.1.0_x64-setup.exe","browser_download_url":"i2"}]},
        {"tag_name":"v1.3.0","draft":true,"assets":[{"name":"update.json","browser_download_url":"u4"}]},
        {"tag_name":"v1.0.0","assets":[{"name":"update.json","browser_download_url":"u1"}]},
        {"tag_name":"v1.0.5","assets":[]},
        {"tag_name":"nightly","assets":[{"name":"update.json","browser_download_url":"u0"}]}
    ]"#;

    #[test]
    fn channels_and_order() {
        let tags = |c| -> Vec<String> {
            candidates(RELEASES.as_bytes(), c, &v("1.0.0"))
                .unwrap()
                .into_iter()
                .map(|(_, r)| r.tag_name)
                .collect()
        };
        // Черновик, без описания, своя же версия и непонятный тег — мимо.
        assert_eq!(tags(Channel::Stable), ["v1.1.0"]);
        assert_eq!(tags(Channel::Beta), ["v1.2.0-beta.1", "v1.1.0"]);
        assert!(
            candidates(RELEASES.as_bytes(), Channel::Beta, &v("1.2.0"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn manifest_must_be_signed_and_match_the_release() {
        let (key, public) = crate::signing::keygen().unwrap();
        let list = candidates(RELEASES.as_bytes(), Channel::Stable, &v("1.0.0")).unwrap();
        let (version, release) = &list[0];
        let manifest = |ver: &str, product: &str, file: &str| {
            serde_json::to_vec(&Manifest {
                product: product.into(),
                version: ver.into(),
                file: file.into(),
                size: 3,
                sha256: "ab".into(),
            })
            .unwrap()
        };
        let file = "MajorLauncher_1.1.0_x64-setup.exe";
        let good = manifest("1.1.0", "MajorLauncher", file);
        let sig = crate::signing::sign(&good, &key).unwrap();
        let (m, url) = accept(&good, &sig, &public, version, release).unwrap();
        assert_eq!((m.file.as_str(), url.as_str()), (file, "i2"));

        // Чужой ключ, правка после подписи, старое описание под новым тегом,
        // другой продукт, файла нет в выпуске — не принимаются.
        let (_, other) = crate::signing::keygen().unwrap();
        assert!(accept(&good, &sig, &other, version, release).is_none());
        let mut edited = good.clone();
        *edited.last_mut().unwrap() = b' ';
        assert!(accept(&edited, &sig, &public, version, release).is_none());
        for bad in [
            manifest("1.0.9", "MajorLauncher", file),
            manifest("1.1.0", "Other", file),
            manifest("1.1.0", "MajorLauncher", "evil.exe"),
        ] {
            let sig = crate::signing::sign(&bad, &key).unwrap();
            assert!(accept(&bad, &sig, &public, version, release).is_none());
        }
    }

    #[test]
    fn manifest_of_a_file_and_integrity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("MajorLauncher_1.1.0_x64-setup.exe");
        std::fs::write(&path, b"abc").unwrap();
        let m = Manifest::for_file(&path, "1.1.0").unwrap();
        assert_eq!(m.size, 3);
        assert_eq!(
            m.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(Manifest::for_file(&path, "один").is_err());
        assert!(intact(&path, &m));
        std::fs::write(&path, b"abd").unwrap();
        assert!(!intact(&path, &m), "тот же размер, другой хеш");
    }

    #[test]
    fn public_key_is_real() {
        assert_eq!(
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, PUBLIC_KEY)
                .map(|k| k.len())
                .ok(),
            Some(32)
        );
    }
}
