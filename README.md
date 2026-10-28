# Capture Studio

Лёгкое нативное Windows-приложение на Rust для захвата экрана и игр, записи и трансляции
(«OBS, но лёгкий, быстрый и понятный с первого взгляда»). Техническое задание —
`../TZ_Capture_Studio.md`, план и статус — [`docs/PLAN.md`](docs/PLAN.md).

**Текущий этап: M0 завершён** — каркас и UI-оболочка на мок-данных. Захват, кодирование и
трансляция — этапы M1–M4.

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
capture-studio.exe                       # в M0 — то же, что --mock-ui
capture-studio.exe --mock-ui --verbose
capture-studio.exe --data-dir D:\cs-data # своя папка данных
capture-studio.exe --screenshot 1c settings.png --theme light --scale 1.5 --lang en
```

Данные — `%APPDATA%\CaptureStudio\` или `.\data\`, если рядом с exe лежит файл `portable`.

## Структура

| Крейт | Назначение |
|---|---|
| `cs-app` | точка входа, Win32-окно, egui-UI (экраны 1a–1f), темы, i18n |
| `cs-core` | модель сцен, состояние, команды/события, конфиги, часы, мок-движок |
| `cs-gpu` | D3D11-устройство, рендерер egui, swapchain (DirectComposition); компоновщик — M1 |
| `cs-diag` | логи, метрики процесса; дампы и бенчмарк — позже |
| `cs-audio` | шкала фейдера; WASAPI и микшер — M2 |
| `cs-capture`, `cs-encode`, `cs-output`, `cs-hotkeys` | интерфейсы; реализации — M1–M3 |
| `cs-hook`, `cs-hook-proto`, `cs-inject` | захват игр (M4); протокол v1 уже зафиксирован |

Документация: [`docs/adr`](docs/adr) — архитектурные решения, [`docs/DEPENDENCIES.md`](docs/DEPENDENCIES.md),
[`docs/OPEN_QUESTIONS.md`](docs/OPEN_QUESTIONS.md), [`docs/reports/M0.md`](docs/reports/M0.md).

## Лицензия

MIT — см. [`LICENSE`](LICENSE). Шрифты IBM Plex Sans и JetBrains Mono — SIL OFL 1.1.
