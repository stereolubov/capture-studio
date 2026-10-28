//! Диагностика: логи (`tracing`), метрики процесса, (позже) дампы и бенчмарк-режим (§14).

pub mod log;
pub mod process;

pub use log::{LogGuard, init_logging};
