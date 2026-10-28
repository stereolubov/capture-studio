//! Миграции схем конфигов (§12.2).
//!
//! Миграция — функция над «сырой» TOML-таблицей: так старые файлы читаются
//! без сохранения старых Rust-структур.

use super::ConfigError;

/// Ключ версии схемы в каждом файле.
pub const VERSION_KEY: &str = "schema_version";

/// Функция миграции из версии `i` в `i + 1`.
pub type Migration = fn(&mut toml::Table);

/// Применяет миграции от версии файла до `target`. Файл без `schema_version` считается версией 0.
pub fn run(table: &mut toml::Table, target: u32, migrations: &[Migration]) -> Result<(), ConfigError> {
    let found =
        table.get(VERSION_KEY).and_then(toml::Value::as_integer).map_or(0, |v| u32::try_from(v).unwrap_or(u32::MAX));
    if found > target {
        return Err(ConfigError::TooNew { found, supported: target });
    }
    for v in found..target {
        if let Some(m) = migrations.get(v as usize) {
            m(table);
            tracing::info!("конфиг мигрирован: схема {v} → {}", v + 1);
        }
    }
    table.insert(VERSION_KEY.into(), toml::Value::Integer(i64::from(target)));
    Ok(())
}

/// Переименовывает ключ, если он есть.
pub fn rename_key(table: &mut toml::Table, from: &str, to: &str) {
    if let Some(v) = table.remove(from) {
        table.entry(to.to_owned()).or_insert(v);
    }
}
