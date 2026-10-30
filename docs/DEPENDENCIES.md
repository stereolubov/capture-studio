# Зависимости

Правило (§0 п.5): каждый крейт — с обоснованием. Проект под MIT (ADR-0007): GPL/AGPL запрещены, LGPL — только динамически через ADR.
Проверка — `cargo deny check` (см. `deny.toml`).
Новый крейт или ассет — строка здесь, в [`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md) (авторы,
лицензия, обязательные уведомления: BSD/OFL требуют сохранять копирайт) и, если компонент заметный, —
в разделе «Сторонние компоненты» README (en и ru).

| Крейт | Где | Зачем | Лицензия | Альтернатива |
|---|---|---|---|---|
| `egui` 0.37 (без default-фич: шрифты egui не встраиваются, только свои OFL) | cs-app, cs-gpu | Immediate-mode UI (§5.2 п.2) | MIT/Apache-2.0 | Slint, iced — через ADR с замерами |
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
| `image` 0.25 (только `png`, `jpeg`, `webp`) | cs-capture | Декодирование источника «Изображение» (§6.5) | MIT/Apache-2.0 | WIC (системный, но громоздкий COM-API) |
| `windows-numerics` | cs-gpu | Типы векторов для Direct2D (`DrawTextLayout`) — зависимость `windows` | MIT/Apache-2.0 | — |
| `png` | cs-app | Запись/чтение PNG для скриншот-тестов | MIT/Apache-2.0 | `image` (тяжелее; понадобится в M1 для источника «Изображение») |
| `nnnoiseless` 0.5 (без default-фич: без CLI и `dasp`) | cs-audio | Шумоподавление RNNoise (§8.3), чистый Rust, модель встроена (+1.2 МБ exe) | BSD-3-Clause | привязки к C-библиотеке RNNoise (сборка C, `rnnoise-c` устарел) |
| `rubato` 5 (без default-фич: без FFT-ресемплеров) | cs-audio | Асинхронный sinc-ресемплер канала: частота устройства → 48 кГц и подстройка под дрейф часов (ADR-0010, §8.1) | MIT/Apache-2.0 | свой ресемплер (линейный был до ADR-0010: хуже качество) |
| `audioadapter-buffers` 5 | cs-audio | Обёртка interleaved-буферов для `rubato` 5 без копирования | MIT/Apache-2.0 | — (интерфейс `rubato`) |
| (без крейта) привязки NVENC `cs-encode/src/nvenc/sys.rs` | cs-encode | Сгенерированы `bindgen` из `nvEncodeAPI.h` NVIDIA (nv-codec-headers n12.0.16.1), см. ADR-0011 | MIT (NVIDIA) | `nvidia-video-codec-sdk` (статическая компоновка, CUDA) |
| (без крейта) объявления AMF `cs-encode/src/amf.rs` | cs-encode | Таблицы методов пяти интерфейсов переписаны с C-раздела заголовков AMF SDK 1.4.36, места проверяются тестом, см. ADR-0013 | MIT (AMD) | `bindgen` (C++-заголовки) |
| `openh264-sys2` 0.9 (без default-фич, фича `libloading`) | cs-encode | Программный H.264: привязки и загрузка двоичного модуля Cisco со сверкой SHA-256 (ADR-0014); тянет `libloading` (ISC) и `sha2` (MIT/Apache-2.0) | BSD-2-Clause | свои привязки (bindgen + хеш вручную); сборка из исходников (фича `source`) — без патентного покрытия Cisco |
| `bzip2` 0.6 (бэкенд `libbz2-rs-sys`, чистый Rust) | cs-encode | Распаковка `openh264-*.dll.bz2` с сервера Cisco (ADR-0014) | MIT/Apache-2.0; `libbz2-rs-sys` — bzip2-1.0.6 | свой распаковщик bzip2; Cisco отдаёт модуль только в `.bz2` |
| `opus-head-sys` 0.3 (без default-фич: без DNN DRED/OSCE) | cs-encode | Opus (§9.2): libopus из исходников, статически; сборка через CMake (ADR-0015) | MIT/Apache-2.0; libopus — BSD-3-Clause | `audiopus_sys` (libopus 1.3, тоже CMake), чистые Rust-реализации (не эталонный кодер) |
| (без крейта) привязки oneVPL `cs-encode/src/qsv/sys.rs` | cs-encode | Сгенерированы `bindgen` из заголовков oneVPL 2.15.0 (intel/libvpl), диспетчер `libvpl.dll` ставит драйвер Intel, см. ADR-0013 | MIT (Intel) | статическая сборка диспетчера (C++) |
| `windows-core` 0.62 | cs-audio | Макрос `#[implement]` для COM-обработчика `ActivateAudioInterfaceAsync` (звук приложения, §8.1) — та же версия, что у `windows` | MIT/Apache-2.0 | ручная vtable (небезопасно и громоздко) |

**Не используется:** `tracing-appender` — нет ротации по размеру, нужной по §12.1 (ADR-0005).
`static_assertions` — заменён встроенными `const _: () = assert!(...)` (без зависимости).

## Ассеты

| Ассет | Лицензия | Размер |
|---|---|---|
| IBM Plex Sans 400/500/600 (TTF) | SIL OFL 1.1 (`assets/fonts/OFL-IBMPlex.txt`) | 3 × ~200 КБ |
| JetBrains Mono 400/500 (TTF) | SIL OFL 1.1 (`assets/fonts/OFL-JetBrainsMono.txt`) | 2 × ~270 КБ |

Шрифты встроены в бинарник целиком (кириллица + латиница + символы); сабсет не потребовался —
exe 5.8 МБ при бюджете 10 МБ.
