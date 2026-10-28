# Capture Studio

*[Русская версия](README.ru.md)*

A lightweight native Windows app written in Rust for screen and game capture, recording and live
streaming — "like OBS, but light, fast and clear at first glance". *Capture Studio* is a working title.

> **Status: early development.** Milestone **M1** is complete: scene compositor, live preview, display
> and window capture, image / color / text sources, scene editing. Audio, encoding and recording arrive
> in M2, streaming in M3, game capture in M4. See the [roadmap](docs/PLAN.md).

## Goals

- **Don't get in the way of the game.** The whole video path stays on the GPU (one D3D11 device,
  zero-copy from capture to encoder); capture must not noticeably lower the game's frame rate.
- **Work with any 3D app.** DirectX 9–12, Vulkan, OpenGL; windowed, borderless and exclusive fullscreen;
  automatic fallback between capture methods (hook → Windows.Graphics.Capture → DXGI duplication).
- **Light.** ~5 MB executable, starts in under a second, low RAM/CPU use.
- **Minimal but informative UI.** Everything important about the stream is visible at a glance.

## Features so far (M1)

- Borderless window with Windows 11 Snap Layouts, per-monitor DPI, dark/light/system themes, RU/EN.
- D3D11 compositor rendered on a high-resolution timer thread; fade/cut transitions; GPU timing.
- Live scene preview shown in the UI without copying frames to the CPU.
- Sources: display capture (WGC or DXGI Desktop Duplication), window capture (WGC, re-attaches after
  restart), image (PNG/JPEG/WebP), color, text (DirectWrite, outline).
- Scene editing: add/edit sources, drag & drop ordering, visibility/lock, move/scale/crop with the
  mouse and edge/center snapping, context menu, undo/redo, auto-save.
- Global scene hotkeys (F1–F4 by default).

Measured on M1 acceptance (1080p60 canvas, 4K display capture): 60.0 fps with 0 dropped frames,
0.02 ms GPU composition time, 0.1% CPU, 66 MB RAM.

## Building

Requires Rust (the version is pinned in `rust-toolchain.toml`; rustup installs it automatically) and the
MSVC Build Tools. The Windows SDK (`fxc.exe`) is optional — precompiled shader bytecode is in the repo.

```
cargo build --release -p cs-app          # target/release/capture-studio.exe
cargo test --workspace                   # includes screenshot tests (WARP, no GPU needed)
```

## Running

```
capture-studio.exe                       # real engine: scenes, sources, preview
capture-studio.exe --mock-ui --verbose   # UI with mock data, no capture
capture-studio.exe --data-dir D:\cs-data # custom data folder
capture-studio.exe --screenshot 1c settings.png --theme light --scale 1.5 --lang en
```

Data lives in `%APPDATA%\CaptureStudio\`, or in `.\data\` when a file named `portable` is next to the exe.

## Repository layout

| Crate | Purpose |
|---|---|
| `cs-app` | entry point, Win32 window, egui UI, engine (render thread), themes, i18n |
| `cs-core` | scene model, app state, commands/events, configs with migrations, clock |
| `cs-gpu` | D3D11 device, compositor, egui renderer, DirectWrite text, swapchain |
| `cs-capture` | video sources: WGC, DXGI duplication, image, color, text |
| `cs-diag` | logging, process metrics |
| `cs-audio`, `cs-encode`, `cs-output` | audio, encoders, recording/streaming (M2–M3) |
| `cs-hotkeys` | global hotkeys |
| `cs-hook`, `cs-hook-proto`, `cs-inject` | game capture hook (M4) |

Documentation (in Russian): [architecture decisions](docs/adr), [dependencies](docs/DEPENDENCIES.md),
[open questions](docs/OPEN_QUESTIONS.md), [milestone reports](docs/reports).

## License

MIT — see [`LICENSE`](LICENSE). Bundled fonts IBM Plex Sans and JetBrains Mono are under the SIL OFL 1.1.
