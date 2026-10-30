//! Аудио (§8): захват WASAPI, микшер 48 кГц float32, индикаторы, раздача блоков энкодерам.

pub mod drift;
#[cfg(windows)]
pub mod engine;
pub mod fader;
pub mod filters;
pub mod mixer;
#[cfg(windows)]
pub mod wasapi;
