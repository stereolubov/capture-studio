//! Логирование в файл с ротацией по размеру: 10 файлов × 10 МБ (§12.1, §14.2).
//!
//! `tracing-appender` умеет только ротацию по времени, поэтому используется
//! собственный писатель (ADR-0005). Объём логов мал, запись синхронная под мьютексом.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_FILES: usize = 10;
const BASE_NAME: &str = "capture-studio";

struct Inner {
    dir: PathBuf,
    file: Option<File>,
    written: u64,
}

impl Inner {
    fn path(&self, index: usize) -> PathBuf {
        if index == 0 {
            self.dir.join(format!("{BASE_NAME}.log"))
        } else {
            self.dir.join(format!("{BASE_NAME}.{index}.log"))
        }
    }

    /// Сдвигает `N.log → N+1.log`, самый старый удаляется.
    fn rotate(&mut self) -> io::Result<()> {
        self.file = None;
        let _ = fs::remove_file(self.path(MAX_FILES - 1));
        for i in (0..MAX_FILES - 1).rev() {
            let from = self.path(i);
            if from.exists() {
                let _ = fs::rename(&from, self.path(i + 1));
            }
        }
        self.open()
    }

    fn open(&mut self) -> io::Result<()> {
        let f = OpenOptions::new().create(true).append(true).open(self.path(0))?;
        self.written = f.metadata().map(|m| m.len()).unwrap_or(0);
        self.file = Some(f);
        Ok(())
    }
}

/// Писатель с ротацией; клонируется дёшево.
#[derive(Clone)]
pub struct RotatingWriter(Arc<Mutex<Inner>>);

impl RotatingWriter {
    /// Открывает журнал в `dir`. Каждый запуск начинает новый файл (старый сдвигается).
    pub fn new(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let mut inner = Inner { dir: dir.to_owned(), file: None, written: 0 };
        inner.rotate()?;
        Ok(Self(Arc::new(Mutex::new(inner))))
    }
}

impl Write for RotatingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut g = self.0.lock().map_err(|_| io::Error::other("log mutex poisoned"))?;
        if g.written + buf.len() as u64 > MAX_FILE_BYTES {
            g.rotate()?;
        }
        let n = match g.file.as_mut() {
            Some(f) => f.write(buf)?,
            None => buf.len(),
        };
        g.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut g = self.0.lock().map_err(|_| io::Error::other("log mutex poisoned"))?;
        match g.file.as_mut() {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

impl<'a> MakeWriter<'a> for RotatingWriter {
    type Writer = RotatingWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Держит журнал открытым; при уничтожении сбрасывает буферы.
pub struct LogGuard {
    writer: Option<RotatingWriter>,
}

impl Drop for LogGuard {
    fn drop(&mut self) {
        if let Some(w) = self.writer.as_mut() {
            let _ = w.flush();
        }
    }
}

/// Инициализирует глобальный подписчик `tracing`.
///
/// `dir = None` — только stderr (тесты, скриншот-режим). `verbose` — уровень debug.
pub fn init_logging(dir: Option<&Path>, verbose: bool) -> LogGuard {
    let level = if verbose { tracing::Level::DEBUG } else { tracing::Level::INFO };
    let builder = tracing_subscriber::fmt().with_max_level(level).with_target(true).with_thread_names(true);
    match dir.map(RotatingWriter::new) {
        Some(Ok(w)) => {
            let _ = builder.with_ansi(false).with_writer(w.clone()).try_init();
            LogGuard { writer: Some(w) }
        }
        Some(Err(e)) => {
            let _ = builder.with_writer(io::stderr).try_init();
            tracing::warn!("не удалось открыть файл лога: {e}");
            LogGuard { writer: None }
        }
        None => {
            let _ = builder.with_writer(io::stderr).try_init();
            LogGuard { writer: None }
        }
    }
}

/// Маскирует секрет для лога: `live_123456789abcd` → `live_****abcd` (§12.3).
pub fn mask_secret(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= 8 {
        return "****".into();
    }
    let prefix_len = s.find('_').map_or(0, |p| s[..=p].chars().count()).min(chars.len() - 4);
    let prefix: String = chars[..prefix_len].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{prefix}****{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masking() {
        assert_eq!(mask_secret("live_123456789abcd"), "live_****abcd");
        assert_eq!(mask_secret("abcdefghijklmn"), "****klmn");
        assert_eq!(mask_secret("short"), "****");
    }

    #[test]
    fn rotation_by_size() {
        let dir = std::env::temp_dir().join(format!("cs-diag-rot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut w = RotatingWriter::new(&dir).expect("writer");
        let chunk = vec![b'x'; 1024 * 1024];
        for _ in 0..25 {
            w.write_all(&chunk).expect("write");
        }
        w.flush().expect("flush");
        let count = fs::read_dir(&dir).expect("dir").count();
        assert!((3..=MAX_FILES).contains(&count), "files: {count}");
        let size = fs::metadata(dir.join("capture-studio.log")).expect("meta").len();
        assert!(size <= MAX_FILE_BYTES);
        drop(w);
        let _ = fs::remove_dir_all(&dir);
    }
}
