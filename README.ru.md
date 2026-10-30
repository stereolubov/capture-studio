# Capture Studio

*[English version](README.md)*

Лёгкое нативное Windows-приложение на Rust для захвата экрана и игр, записи и трансляции
(«OBS, но лёгкий, быстрый и понятный с первого взгляда»). *Capture Studio* — рабочее название.

> **Статус: ранняя разработка.** Завершён этап **M1** — компоновщик, живое превью, захват экрана и окна,
> изображение, цвет, текст, редактирование сцен. Аудио, кодирование и запись — M2, трансляция — M3,
> захват игр — M4. План — [`docs/PLAN.md`](docs/PLAN.md).

## Сборка

Нужны Rust (версия фиксирована в `rust-toolchain.toml`, ставится rustup автоматически) и
MSVC Build Tools с CMake (компонент «C++ CMake tools», им собирается libopus). Windows SDK
(`fxc.exe`) — необязательно: без него используется
скомпилированный байт-код шейдеров из репозитория.

```
cargo build --release -p cs-app          # target/release/capture-studio.exe
cargo test --workspace                   # включая скриншот-тесты (WARP, без GPU)
```

## Запуск

```
capture-studio.exe                       # настоящий движок: сцены, источники, превью
capture-studio.exe --mock-ui --verbose   # UI на мок-данных без захвата
capture-studio.exe --data-dir D:\cs-data # своя папка данных
capture-studio.exe --screenshot 1c settings.png --theme light --scale 1.5 --lang en
```

Данные — `%APPDATA%\CaptureStudio\` или `.\data\`, если рядом с exe лежит файл `portable`.

## Структура

| Крейт | Назначение |
|---|---|
| `cs-app` | точка входа, Win32-окно, egui-UI, движок (рендер-поток), темы, i18n |
| `cs-core` | модель сцен, состояние, команды/события, конфиги с миграциями, часы |
| `cs-gpu` | D3D11-устройство, компоновщик, рендерер egui, текст DirectWrite, swapchain |
| `cs-capture` | видеоисточники: WGC, DXGI Duplication, изображение, цвет, текст |
| `cs-diag` | логи, метрики процесса |
| `cs-audio`, `cs-encode`, `cs-output` | аудио, энкодеры, запись и трансляция (M2–M3) |
| `cs-hotkeys` | глобальные горячие клавиши |
| `cs-hook`, `cs-hook-proto`, `cs-inject` | хук захвата игр (M4) |

Документация: [`docs/adr`](docs/adr) — архитектурные решения, [`docs/DEPENDENCIES.md`](docs/DEPENDENCIES.md),
[`docs/OPEN_QUESTIONS.md`](docs/OPEN_QUESTIONS.md), [`docs/reports`](docs/reports) — отчёты этапов.

## Лицензия

MIT — см. [`LICENSE`](LICENSE).

## Сторонние компоненты

Capture Studio построен на открытых проектах. Большое спасибо их авторам:

- [egui](https://github.com/emilk/egui), Emil Ernerfeldt — интерфейс (MIT / Apache-2.0);
- [windows-rs](https://github.com/microsoft/windows-rs), Microsoft — привязки к Windows API (MIT / Apache-2.0);
- [rubato](https://github.com/HEnquist/rubato), Henrik Enquist — ресемплинг звука и компенсация дрейфа
  часов (MIT / Apache-2.0);
- [nnnoiseless](https://github.com/jneem/nnnoiseless), Joe Neeman — порт на Rust
  [RNNoise](https://github.com/xiph/rnnoise) (Jean-Marc Valin, Mozilla, Xiph.Org Foundation) —
  шумоподавление (BSD-3-Clause);
- заголовок NVIDIA Video Codec SDK `nvEncodeAPI.h` — привязки к энкодеру NVENC (MIT);
- заголовки [AMD AMF SDK](https://github.com/GPUOpen-LibrariesAndSDKs/AMF), Advanced Micro Devices —
  объявления интерфейсов энкодера AMF (MIT);
- заголовки [Intel® Video Processing Library (oneVPL)](https://github.com/intel/libvpl), Intel —
  привязки к энкодеру Quick Sync Video (MIT);
- [OpenH264](https://www.openh264.org/), Cisco — программный энкодер H.264. OpenH264 Video Codec
  provided by Cisco Systems, Inc. Двоичный модуль скачивается с сервера Cisco по вашему запросу и не
  входит в Capture Studio; привязки — [openh264-rs](https://crates.io/crates/openh264-sys2),
  Ralf Biedert (BSD-2-Clause), загрузка — [libloading](https://github.com/nagisa/rust_libloading),
  Simonas Kazlauskas (ISC);
- [Opus](https://opus-codec.org/), Xiph.Org, Skype, Octasic, Jean-Marc Valin, Timothy B. Terriberry и
  другие — аудиоэнкодер Opus (BSD-3-Clause), сборка через
  [opus-head-sys](https://github.com/xnorpx/rust-opus), Marcus Asteborg (MIT / Apache-2.0);
- [bzip2-rs](https://github.com/trifectatechfoundation/bzip2-rs) и
  [libbzip2-rs](https://github.com/trifectatechfoundation/libbzip2-rs), Trifecta Tech Foundation, на
  основе bzip2 Julian Seward — распаковка загруженного OpenH264 (MIT / Apache-2.0, bzip2-1.0.6);
- [image](https://github.com/image-rs/image), [fluent-rs](https://github.com/projectfluent/fluent-rs),
  [serde](https://github.com/serde-rs/serde), [toml](https://github.com/toml-rs/toml),
  [tracing](https://github.com/tokio-rs/tracing), [RustCrypto sha2](https://github.com/RustCrypto/hashes)
  и другие крейты (MIT / Apache-2.0);
- шрифты [IBM Plex Sans](https://github.com/IBM/plex) и [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono)
  (SIL Open Font License 1.1).

Полный список с авторами и тексты уведомлений лицензий — в [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
