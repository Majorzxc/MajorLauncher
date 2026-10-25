//! Отмена долгого дела — например, подготовки к запуску игры по кнопке
//! «Отменить». Загрузки обрываются сразу (недокачанное остаётся в `*.part`
//! и докачается в следующий раз), установщик Forge останавливается между
//! шагами или прямо посреди шага — его процесс Java завершается.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Ошибка «отменено», если отмена уже нажата.
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Выполнить `fut`, но бросить его при отмене. Брошенная загрузка
    /// оставляет только `*.part`, а готовые файлы пишутся переименованием —
    /// битых файлов не остаётся.
    pub async fn guard<T>(&self, fut: impl Future<Output = Result<T>>) -> Result<T> {
        tokio::select! {
            r = fut => r,
            _ = self.wait() => Err(Error::Cancelled),
        }
    }

    async fn wait(&self) {
        while !self.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// Подождать, не занимая поток (таймер рантайма tokio — и у окна, и у `mlcli`).
pub async fn sleep(d: Duration) {
    tokio::time::sleep(d).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn guard_stops_long_work() {
        let cancel = Cancel::default();
        let c = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            c.cancel();
        });
        let r: Result<()> = cancel
            .guard(async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                Ok(())
            })
            .await;
        assert!(matches!(r, Err(Error::Cancelled)));
        assert!(cancel.check().is_err());
        assert_eq!(Cancel::default().guard(async { Ok(5) }).await.unwrap(), 5);
    }
}
