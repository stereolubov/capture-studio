//! Аудио (§8): захват WASAPI, микшер 48 кГц float32, индикаторы, раздача блоков энкодерам.

#[cfg(windows)]
pub mod engine;
pub mod fader;
pub mod mixer;
pub mod resample;
#[cfg(windows)]
pub mod wasapi;
