//! Глобальные настройки `settings.toml`: язык, тема, окно, последний профиль.

use super::Versioned;
use super::migrate::{Migration, rename_key};
use serde::{Deserialize, Serialize};

/// Тема интерфейса (§11.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePref {
    #[default]
    Dark,
    Light,
    System,
}

/// Приоритет процесса (§5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessPriority {
    Normal,
    #[default]
    AboveNormal,
    High,
}

/// Положение и размер окна в логических пикселях.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowPlacement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Код языка: `ru` (основной) или `en`.
    pub language: String,
    pub theme: ThemePref,
    pub window: Option<WindowPlacement>,
    /// Позиция мини-режима (§11.6, «положение запоминается»).
    pub mini_position: Option<(i32, i32)>,
    pub last_profile: String,
    pub last_scene_collection: String,
    pub start_minimized: bool,
    pub confirm_stop_stream: bool,
    pub check_updates: bool,
    pub process_priority: ProcessPriority,
    /// Папка записей; `None` — «Видео\Capture Studio».
    pub recordings_dir: Option<String>,
    pub preview_enabled: bool,
    pub verbose_log: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "ru".into(),
            theme: ThemePref::Dark,
            window: None,
            mini_position: None,
            last_profile: "Основной".into(),
            last_scene_collection: "Основная".into(),
            start_minimized: false,
            confirm_stop_stream: false,
            check_updates: false,
            process_priority: ProcessPriority::AboveNormal,
            recordings_dir: None,
            preview_enabled: true,
            verbose_log: false,
        }
    }
}

/// v0 → v1: формат прототипов до M0 (`lang`, `dark_theme = true/false`).
fn v0_to_v1(t: &mut toml::Table) {
    rename_key(t, "lang", "language");
    if let Some(dark) = t.remove("dark_theme").and_then(|v| v.as_bool()) {
        t.insert("theme".into(), toml::Value::String(if dark { "dark" } else { "light" }.into()));
    }
}

impl Versioned for Settings {
    const SCHEMA_VERSION: u32 = 1;
    fn migrations() -> &'static [Migration] {
        &[v0_to_v1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigError, from_toml_str};

    #[test]
    fn migrate_v0() {
        let s: Settings = from_toml_str("lang = \"en\"\ndark_theme = false\n").expect("migrate");
        assert_eq!(s.language, "en");
        assert_eq!(s.theme, ThemePref::Light);
        // Остальные поля — по умолчанию.
        assert_eq!(s.last_profile, Settings::default().last_profile);
    }

    #[test]
    fn current_version_untouched() {
        let s: Settings = from_toml_str("schema_version = 1\nlanguage = \"ru\"\ntheme = \"system\"\n").expect("parse");
        assert_eq!(s.theme, ThemePref::System);
    }

    #[test]
    fn newer_version_rejected() {
        let r = from_toml_str::<Settings>("schema_version = 99\n");
        assert!(matches!(r, Err(ConfigError::TooNew { found: 99, .. })));
    }

    #[test]
    fn unknown_keys_ignored() {
        let s: Settings = from_toml_str("schema_version = 1\nfuture_option = 5\n").expect("parse");
        assert_eq!(s, Settings::default());
    }
}
