# Зависимости

Правило (§0 п.5): каждый крейт — с обоснованием. Проект под MIT (ADR-0007): GPL/AGPL запрещены, LGPL — только динамически через ADR.
Проверка — `cargo deny check` (см. `deny.toml`).

| Крейт | Где | Зачем | Лицензия | Альтернатива |
|---|---|---|---|---|
| `egui` 0.37 (без `default_fonts`-зависимостей кроме встроенных) | cs-app, cs-gpu | Immediate-mode UI (§5.2 п.2) | MIT/Apache-2.0 | Slint, iced — через ADR с замерами |
| `windows` 0.62 | cs-app, cs-gpu | Win32/D3D11/DXGI/DComp/DWM — типизированные COM-обёртки | MIT/Apache-2.0 | `winapi` (не развивается) |
| `windows-sys` 0.61 | cs-core, cs-diag | Лёгкие FFI-функции без COM (QPC, ReplaceFileW, метрики процесса) | MIT/Apache-2.0 | `windows` (тяжелее компилируется) |
| `serde` | cs-core | Сериализация конфигов | MIT/Apache-2.0 | — |
| `toml` 1.x | cs-core | Формат конфигов (§12.2) | MIT/Apache-2.0 | — |
| `thiserror` | библиотеки | Типизированные ошибки (§15) | MIT/Apache-2.0 | ручные `impl Error` |
| `anyhow` | cs-app | Ошибки верхнего уровня (§15) | MIT/Apache-2.0 | — |
| `tracing`, `tracing-subscriber` (fmt) | cs-diag и др. | Логи (§14.2) | MIT | `log` + `env_logger` |
| `parking_lot` | cs-core | `RwLock` для `AppState` (§5.2 п.6) — подготовка к M1 | MIT/Apache-2.0 | `std::sync` |
| `crossbeam-channel` | cs-core | Ограниченные очереди между потоками (§5.2 п.5) — подготовка к M1 | MIT/Apache-2.0 | `std::sync::mpsc` |
| `uuid` (v4, serde) | cs-core | Идентификаторы сцен/источников | MIT/Apache-2.0 | счётчик (неуникален между коллекциями) |
| `fluent-bundle`, `unic-langid` | cs-app | Локализация (§11.7) | MIT/Apache-2.0 | — (Fluent указан в ТЗ) |
| `png` | cs-app | Запись/чтение PNG для скриншот-тестов | MIT/Apache-2.0 | `image` (тяжелее; понадобится в M1 для источника «Изображение») |

**Не используется:** `tracing-appender` — нет ротации по размеру, нужной по §12.1 (ADR-0005).
`static_assertions` — заменён встроенными `const _: () = assert!(...)` (без зависимости).

## Ассеты

| Ассет | Лицензия | Размер |
|---|---|---|
| IBM Plex Sans 400/500/600 (TTF) | SIL OFL 1.1 (`assets/fonts/OFL-IBMPlex.txt`) | 3 × ~200 КБ |
| JetBrains Mono 400/500 (TTF) | SIL OFL 1.1 (`assets/fonts/OFL-JetBrainsMono.txt`) | 2 × ~270 КБ |

Шрифты встроены в бинарник целиком (кириллица + латиница + символы); сабсет не потребовался —
exe 5.8 МБ при бюджете 10 МБ.
