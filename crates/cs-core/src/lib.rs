//! Ядро Capture Studio: модель сцен, состояние приложения, команды/события,
//! конфиги с миграциями и мастер-часы.
//!
//! Крейт не содержит UI- и GPU-кода. UI читает снимок [`state::AppState`] и
//! отправляет [`state::Command`] — никакой логики эфира в UI (§5.2 п.6).

pub mod clock;
pub mod config;
pub mod estimate;
pub mod geometry;
pub mod history;
pub mod mock;
pub mod model;
pub mod state;

pub use state::{AppState, Command, Event};
