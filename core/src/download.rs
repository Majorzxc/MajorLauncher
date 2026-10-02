//! Загрузка файлов: параллельно, с докачкой после обрыва, повторами
//! и проверкой хеша. Файл сначала качается в `*.part` и переименовывается
//! только после проверки — недокачанный или подменённый файл не попадёт в игру.

use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::{StreamExt, TryStreamExt, stream};
use reqwest::StatusCode;
use reqwest::header::RANGE;
use tokio::io::AsyncWriteExt;

use crate::error::{Error, IoContext, Result};
use crate::verify::sha1_file;

/// Запасные зеркала (раздел 5.14 ТЗ). Только для ассетов и библиотек, и только
/// для файлов с хешем из официальных данных Mojang: подменённый файл не пройдёт
/// проверку. Зеркало BMCLAPI проверено 02.10.2026 — отдаёт те же файлы.
const MIRRORS: &[(&str, &str)] = &[
    (
        "https://resources.download.minecraft.net/",
        "https://bmclapi2.bangbang93.com/assets/",
    ),
    (
        "https://libraries.minecraft.net/",
        "https://bmclapi2.bangbang93.com/maven/",
    ),
];

/// Что скачать и куда. `sha1` и `size` известны почти всегда: их даёт Mojang.
#[derive(Debug, Clone)]
pub struct Task {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DownloadOptions {
    /// Сколько файлов качать одновременно.
    pub parallel: usize,
    /// Пробовать запасные зеркала, если официальный сервер недоступен.
    pub mirrors: bool,
    /// Попыток на каждый адрес.
    pub retries: u32,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            parallel: 16,
            mirrors: true,
            retries: 3,
        }
    }
}

#[derive(Clone)]
pub struct Downloader {
    client: reqwest::Client,
    opts: DownloadOptions,
}

fn network(url: &str) -> impl FnOnce(reqwest::Error) -> Error + '_ {
    move |source| Error::Network {
        url: url.to_string(),
        source,
    }
}

fn mirror_for(url: &str) -> Option<String> {
    MIRRORS
        .iter()
        .find_map(|(from, to)| url.strip_prefix(from).map(|rest| format!("{to}{rest}")))
}

fn part_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

impl Downloader {
    pub fn new(opts: DownloadOptions) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(format!(
                "MajorLauncher/{} (+https://github.com/Majorzxc/MajorLauncher)",
                crate::VERSION
            ))
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Other(format!("не удалось создать HTTP-клиент: {e}")))?;
        Ok(Self { client, opts })
    }

    /// Скачать небольшой файл в память (описания версий, списки). Без зеркал:
    /// именно из этих данных берутся хеши, поэтому только официальный источник.
    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let mut last = None;
        for attempt in 0..self.opts.retries {
            match self.get_once(url).await {
                Ok(bytes) => return Ok(bytes),
                Err(
                    e @ Error::HttpStatus {
                        status: 400..=499, ..
                    },
                ) => return Err(e),
                Err(e) => last = Some(e),
            }
            tokio::time::sleep(backoff(attempt)).await;
        }
        Err(last.expect("хотя бы одна попытка"))
    }

    async fn get_once(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self.client.get(url).send().await.map_err(network(url))?;
        if !resp.status().is_success() {
            return Err(Error::HttpStatus {
                url: url.to_string(),
                status: resp.status().as_u16(),
            });
        }
        Ok(resp.bytes().await.map_err(network(url))?.to_vec())
    }

    /// Скачать в память и сверить хеш.
    pub async fn get_verified(&self, url: &str, sha1: &str) -> Result<Vec<u8>> {
        let bytes = self.get_bytes(url).await?;
        let actual = crate::verify::sha1_bytes(&bytes);
        if !actual.eq_ignore_ascii_case(sha1) {
            return Err(Error::HashMismatch {
                path: PathBuf::from(url),
                expected: sha1.to_string(),
                actual,
            });
        }
        Ok(bytes)
    }

    /// Скачать все файлы. `on_done` вызывается после каждого проверенного файла.
    /// Первая неисправимая ошибка останавливает загрузку остальных.
    pub async fn download_all(
        &self,
        tasks: Vec<Task>,
        on_done: &(dyn Fn(&Task) + Sync),
    ) -> Result<()> {
        stream::iter(tasks.into_iter().map(Ok))
            .try_for_each_concurrent(self.opts.parallel.max(1), |task| async move {
                self.download(&task).await?;
                on_done(&task);
                Ok(())
            })
            .await
    }

    /// Скачать один файл: официальный адрес, затем зеркало; на каждый — несколько попыток.
    pub async fn download(&self, task: &Task) -> Result<()> {
        let mut urls = vec![task.url.clone()];
        if self.opts.mirrors
            && task.sha1.is_some()
            && let Some(mirror) = mirror_for(&task.url)
        {
            urls.push(mirror);
        }

        let mut last = None;
        for url in &urls {
            for attempt in 0..self.opts.retries {
                match self.try_download(url, task).await {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        // 404 и прочие 4xx не исправятся повтором — сразу к зеркалу.
                        let give_up = matches!(
                            e,
                            Error::HttpStatus {
                                status: 400..=499,
                                ..
                            }
                        );
                        tracing::warn!("попытка {} для {url}: {e}", attempt + 1);
                        last = Some(e);
                        if give_up {
                            break;
                        }
                        tokio::time::sleep(backoff(attempt)).await;
                    }
                }
            }
        }
        Err(last.expect("хотя бы одна попытка"))
    }

    async fn try_download(&self, url: &str, task: &Task) -> Result<()> {
        let path = &task.path;
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await.at(dir)?;
        }
        let part = part_path(path);

        // Докачка: сколько уже лежит в .part от прошлой попытки или запуска.
        let mut have = tokio::fs::metadata(&part)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        if task.size.is_some_and(|size| have > size) {
            tokio::fs::remove_file(&part).await.at(&part)?;
            have = 0;
        }

        // Если .part уже полный (оборвалось на проверке или переименовании) — в сеть не ходим.
        let complete = have > 0 && task.size == Some(have);
        if !complete {
            let mut req = self.client.get(url);
            if have > 0 {
                req = req.header(RANGE, format!("bytes={have}-"));
            }
            let resp = req.send().await.map_err(network(url))?;
            let status = resp.status();
            let append = match status {
                StatusCode::PARTIAL_CONTENT => true,
                s if s.is_success() => false,
                StatusCode::RANGE_NOT_SATISFIABLE => {
                    // Сервер не принял докачку — начнём заново со следующей попытки.
                    let _ = tokio::fs::remove_file(&part).await;
                    return Err(Error::HttpStatus {
                        url: url.to_string(),
                        status: status.as_u16(),
                    });
                }
                s => {
                    return Err(Error::HttpStatus {
                        url: url.to_string(),
                        status: s.as_u16(),
                    });
                }
            };

            let mut file = if append {
                tokio::fs::OpenOptions::new().append(true).open(&part).await
            } else {
                tokio::fs::File::create(&part).await
            }
            .at(&part)?;

            let mut body = resp.bytes_stream();
            while let Some(chunk) = body.next().await {
                let chunk = chunk.map_err(network(url))?;
                file.write_all(&chunk).await.at(&part)?;
            }
            file.flush().await.at(&part)?;
        }

        if let Some(expected) = &task.sha1 {
            let check = part.clone();
            let actual = tokio::task::spawn_blocking(move || sha1_file(&check))
                .await
                .map_err(|e| Error::Other(format!("проверка хеша прервана: {e}")))??;
            if !actual.eq_ignore_ascii_case(expected) {
                let _ = tokio::fs::remove_file(&part).await;
                return Err(Error::HashMismatch {
                    path: path.clone(),
                    expected: expected.clone(),
                    actual,
                });
            }
        }

        tokio::fs::rename(&part, path).await.at(path)?;
        Ok(())
    }
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(500 << attempt.min(4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrors_only_for_known_hosts() {
        assert_eq!(
            mirror_for("https://resources.download.minecraft.net/0d/0d00").as_deref(),
            Some("https://bmclapi2.bangbang93.com/assets/0d/0d00")
        );
        assert_eq!(
            mirror_for("https://libraries.minecraft.net/org/a.jar").as_deref(),
            Some("https://bmclapi2.bangbang93.com/maven/org/a.jar")
        );
        assert_eq!(
            mirror_for("https://piston-data.mojang.com/v1/objects/x"),
            None
        );
    }

    #[test]
    fn part_file_is_next_to_target() {
        assert_eq!(
            part_path(Path::new("a/b/c.jar")),
            PathBuf::from("a/b/c.jar.part")
        );
    }

    use std::sync::{Arc, Mutex};
    use tokio::io::AsyncReadExt;

    /// Мини-сервер HTTP: отдаёт `body`, понимает `Range: bytes=N-` и запоминает,
    /// с какого байта у него просили файл.
    async fn serve(body: Vec<u8>) -> (String, Arc<Mutex<Vec<u64>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let ranges = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&ranges);
        tokio::spawn(async move {
            loop {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut req = Vec::new();
                let mut buf = [0u8; 1024];
                while !req.ends_with(b"\r\n\r\n") {
                    let n = sock.read(&mut buf).await.unwrap();
                    req.extend_from_slice(&buf[..n]);
                }
                let req = String::from_utf8_lossy(&req).to_lowercase();
                let start = req
                    .lines()
                    .find_map(|l| l.strip_prefix("range: bytes="))
                    .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok());
                seen.lock().unwrap().push(start.unwrap_or(0) as u64);
                let head = match start {
                    Some(s) => format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {s}-{}/{}\r\nConnection: close\r\n\r\n",
                        body.len() - s,
                        body.len() - 1,
                        body.len()
                    ),
                    None => format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    ),
                };
                sock.write_all(head.as_bytes()).await.unwrap();
                sock.write_all(&body[start.unwrap_or(0)..]).await.unwrap();
            }
        });
        (format!("http://{addr}/file.bin"), ranges)
    }

    fn downloader() -> Downloader {
        Downloader::new(DownloadOptions {
            parallel: 1,
            mirrors: false,
            retries: 1,
        })
        .unwrap()
    }

    #[tokio::test]
    async fn resumes_from_partial_file() {
        let body: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        let (url, ranges) = serve(body.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.bin");
        // Как будто прошлая загрузка оборвалась на 40 000 байт.
        std::fs::write(part_path(&path), &body[..40_000]).unwrap();

        let task = Task {
            url,
            path: path.clone(),
            sha1: Some(crate::verify::sha1_bytes(&body)),
            size: Some(body.len() as u64),
        };
        downloader().download(&task).await.unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), body);
        assert!(!part_path(&path).exists());
        assert_eq!(
            *ranges.lock().unwrap(),
            vec![40_000],
            "докачан только остаток"
        );
    }

    #[tokio::test]
    async fn rejects_file_with_wrong_hash() {
        let (url, _) = serve(b"not what we expected".to_vec()).await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file.bin");
        let task = Task {
            url,
            path: path.clone(),
            sha1: Some("0000000000000000000000000000000000000000".into()),
            size: None,
        };

        let err = downloader().download(&task).await.unwrap_err();
        assert!(matches!(err, Error::HashMismatch { .. }));
        assert!(!path.exists(), "подменённый файл не попал на место");
        assert!(!part_path(&path).exists());
    }
}
