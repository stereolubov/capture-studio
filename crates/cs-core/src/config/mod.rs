//! Настройки, профили и хранение данных (§12).
//!
//! * Формат — TOML с полем `schema_version` и цепочкой миграций ([`migrate`]).
//! * Запись атомарная: временный файл + `ReplaceFileW`, предыдущая версия — `.bak`.
//! * Портативный режим: файл `portable` рядом с exe → данные в `.\data\`.

pub mod migrate;
pub mod profile;
pub mod settings;

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub use profile::Profile;
pub use settings::Settings;

/// Ошибки работы с конфигами.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("ошибка ввода-вывода «{path}»: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("неверный формат «{path}»: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("ошибка сериализации: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("версия схемы {found} новее поддерживаемой {supported}")]
    TooNew { found: u32, supported: u32 },
    #[error("не удалось определить папку данных (нет %APPDATA%)")]
    NoDataDir,
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> ConfigError + '_ {
    move |source| ConfigError::Io { path: path.to_owned(), source }
}

/// Раскладка каталога данных (§12.1).
#[derive(Debug, Clone)]
pub struct DataDirs {
    pub root: PathBuf,
    pub portable: bool,
}

impl DataDirs {
    /// Определяет каталог данных: портативный (`<exe>\data`) или `%APPDATA%\CaptureStudio`.
    pub fn detect() -> Result<Self, ConfigError> {
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
            && dir.join("portable").is_file()
        {
            return Ok(Self { root: dir.join("data"), portable: true });
        }
        let appdata = std::env::var_os("APPDATA").ok_or(ConfigError::NoDataDir)?;
        Ok(Self { root: PathBuf::from(appdata).join("CaptureStudio"), portable: false })
    }

    /// Каталог данных по явному пути (тесты, `--data-dir`).
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), portable: false }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.toml")
    }

    pub fn profile_file(&self, name: &str) -> PathBuf {
        self.root.join("profiles").join(name).join("profile.toml")
    }

    pub fn scenes_dir(&self) -> PathBuf {
        self.root.join("scenes")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn crashes_dir(&self) -> PathBuf {
        self.root.join("crashes")
    }

    /// Создаёт все подкаталоги.
    pub fn ensure(&self) -> Result<(), ConfigError> {
        for d in [self.root.clone(), self.root.join("profiles"), self.scenes_dir(), self.logs_dir(), self.crashes_dir()]
        {
            fs::create_dir_all(&d).map_err(io_err(&d))?;
        }
        Ok(())
    }
}

/// Документ с версионируемой схемой.
pub trait Versioned: Serialize + DeserializeOwned + Default {
    /// Текущая версия схемы.
    const SCHEMA_VERSION: u32;
    /// Миграции: элемент `i` переводит документ из версии `i` в `i + 1`.
    fn migrations() -> &'static [migrate::Migration];
}

/// Загружает документ; если файла нет — возвращает значение по умолчанию.
/// Повреждённый основной файл → попытка прочитать `.bak`.
pub fn load<T: Versioned>(path: &Path) -> Result<T, ConfigError> {
    match load_one::<T>(path) {
        Ok(Some(v)) => Ok(v),
        Ok(None) => Ok(T::default()),
        Err(e @ ConfigError::TooNew { .. }) => Err(e),
        Err(e) => {
            let bak = backup_path(path);
            match load_one::<T>(&bak) {
                Ok(Some(v)) => {
                    tracing::warn!("{e}; восстановлено из {}", bak.display());
                    Ok(v)
                }
                _ => Err(e),
            }
        }
    }
}

fn load_one<T: Versioned>(path: &Path) -> Result<Option<T>, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_err(path)(e)),
    };
    from_toml_str::<T>(&text).map(Some).map_err(|e| match e {
        ConfigError::Parse { message, .. } => ConfigError::Parse { path: path.to_owned(), message },
        other => other,
    })
}

/// Разбирает TOML, применяя миграции до текущей версии.
pub fn from_toml_str<T: Versioned>(text: &str) -> Result<T, ConfigError> {
    let parse_err = |message: String| ConfigError::Parse { path: PathBuf::new(), message };
    let mut table: toml::Table = text.parse().map_err(|e: toml::de::Error| parse_err(e.to_string()))?;
    migrate::run(&mut table, T::SCHEMA_VERSION, T::migrations())?;
    T::deserialize(toml::Value::Table(table)).map_err(|e| parse_err(e.to_string()))
}

/// Сохраняет документ атомарно, оставляя предыдущую версию в `.bak`.
pub fn save<T: Versioned>(path: &Path, value: &T) -> Result<(), ConfigError> {
    let mut table = toml::Table::try_from(value)?;
    table.insert(migrate::VERSION_KEY.into(), toml::Value::Integer(i64::from(T::SCHEMA_VERSION)));
    let text = toml::to_string_pretty(&table)?;
    write_atomic(path, text.as_bytes())
}

fn backup_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".bak");
    PathBuf::from(s)
}

/// Атомарная запись файла: временный файл в том же каталоге → замена.
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<(), ConfigError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io_err(dir))?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut f = fs::File::create(&tmp).map_err(io_err(&tmp))?;
        f.write_all(data).map_err(io_err(&tmp))?;
        f.sync_all().map_err(io_err(&tmp))?;
    }
    replace(path, &tmp, &backup_path(path)).map_err(io_err(path))
}

#[cfg(windows)]
fn replace(target: &Path, tmp: &Path, backup: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{REPLACEFILE_IGNORE_MERGE_ERRORS, ReplaceFileW};

    if !target.exists() {
        return fs::rename(tmp, target);
    }
    let wide = |p: &Path| -> Vec<u16> { p.as_os_str().encode_wide().chain(Some(0)).collect() };
    let (t, r, b) = (wide(target), wide(tmp), wide(backup));
    // SAFETY: все три строки — валидные нуль-терминированные UTF-16 буферы,
    // живущие до конца вызова; зарезервированные параметры равны null.
    let ok = unsafe {
        ReplaceFileW(
            t.as_ptr(),
            r.as_ptr(),
            b.as_ptr(),
            REPLACEFILE_IGNORE_MERGE_ERRORS,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok == 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) }
}

#[cfg(not(windows))]
fn replace(target: &Path, tmp: &Path, backup: &Path) -> std::io::Result<()> {
    if target.exists() {
        fs::copy(target, backup)?;
    }
    fs::rename(tmp, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cs-core-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).expect("temp dir");
        d
    }

    #[test]
    fn save_load_roundtrip_with_backup() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("settings.toml");
        let mut s = Settings { language: "en".into(), ..Settings::default() };
        save(&path, &s).expect("save 1");
        s.language = "ru".into();
        save(&path, &s).expect("save 2");
        let loaded: Settings = load(&path).expect("load");
        assert_eq!(loaded, s);
        let bak: Settings = load(&backup_path(&path)).expect("load bak");
        assert_eq!(bak.language, "en");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_falls_back_to_backup() {
        let dir = temp_dir("corrupt");
        let path = dir.join("settings.toml");
        let s = Settings::default();
        save(&path, &s).expect("save 1");
        save(&path, &s).expect("save 2");
        fs::write(&path, "это не toml = = =").expect("corrupt");
        let loaded: Settings = load(&path).expect("fallback");
        assert_eq!(loaded, s);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_gives_default() {
        let dir = temp_dir("missing");
        let loaded: Settings = load(&dir.join("nope.toml")).expect("default");
        assert_eq!(loaded, Settings::default());
        let _ = fs::remove_dir_all(&dir);
    }
}
