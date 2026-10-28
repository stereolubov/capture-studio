# Capture Studio

*[English version](README.md)*

Лёгкое нативное Windows-приложение на Rust для захвата экрана и игр, записи и трансляции
(«OBS, но лёгкий, быстрый и понятный с первого взгляда»). *Capture Studio* — рабочее название.

> **Статус: ранняя разработка.** Завершён этап **M1** — компоновщик, живое превью, захват экрана и окна,
> изображение, цвет, текст, редактирование сцен. Аудио, кодирование и запись — M2, трансляция — M3,
> захват игр — M4. План — [`docs/PLAN.md`](docs/PLAN.md).

## Сборка

Нужны Rust (версия фиксирована в `rust-toolchain.toml`, ставится rustup автоматически) и
MSVC Build Tools. Windows SDK (`fxc.exe`) — необязательно: без него используется
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

MIT — см. [`LICENSE`](LICENSE). Шрифты IBM Plex Sans и JetBrains Mono — SIL OFL 1.1.
