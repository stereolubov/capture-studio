# Capture Studio

*[Русская версия](README.ru.md)*

A lightweight native Windows app written in Rust for screen and game capture, recording and live
streaming — "like OBS, but light, fast and clear at first glance". *Capture Studio* is a working title.

> **Status: early development.** M0 and M1 are complete. The M2 audio, encoding and recording
> features are implemented; final acceptance and validation on AMD and Intel systems are in progress.
> Live streaming is planned for M3, game capture for M4. See the [roadmap](docs/PLAN.md).

## Goals

- **Don't get in the way of the game.** The whole video path stays on the GPU (one D3D11 device,
  zero-copy from capture to encoder); capture must not noticeably lower the game's frame rate.
- **Work with any 3D app.** DirectX 9–12, Vulkan, OpenGL; windowed, borderless and exclusive fullscreen;
  automatic fallback between capture methods (hook → Windows.Graphics.Capture → DXGI duplication).
- **Light.** ~5 MB executable, starts in under a second, low RAM/CPU use.
- **Minimal but informative UI.** Everything important about the stream is visible at a glance.

## Features implemented (M0–M2; M2 acceptance in progress)

- Borderless window with Windows 11 Snap Layouts, per-monitor DPI, dark/light/system themes, RU/EN.
- D3D11 compositor rendered on a high-resolution timer thread; fade/cut transitions; GPU timing.
- Live scene preview shown in the UI without copying frames to the CPU.
- Sources: display capture (WGC or DXGI Desktop Duplication), window capture (WGC, re-attaches after
  restart), image (PNG/JPEG/WebP), color, text (DirectWrite, outline).
- Scene editing: add/edit sources, drag & drop ordering, visibility/lock, move/scale/crop with the
  mouse and edge/center snapping, context menu, undo/redo, auto-save.
- Global scene hotkeys (F1–F4 by default).
- Audio: WASAPI desktop, microphone and per-app capture; a six-track mixer, device selection, clock-drift
  correction, per-channel sync offsets, and gain, RNNoise, gate, compressor and limiter filters.
- Recording: hardware H.264 via NVIDIA NVENC, AMD AMF, Intel QSV or Media Foundation, with an optional
  Cisco OpenH264 download; encoder probing and automatic selection.
- MKV, MP4 remux and fragmented MP4 recording, AAC and MKV-only Opus, separate audio tracks, pause,
  free-space checks, scaling and 10-bit P010 conversion.
- Final M2 acceptance is still pending: a one-hour run against the acceptance limits and field validation
  of AMF/QSV on AMD/Intel hardware. Live streaming is not implemented yet.

Measured on M1 acceptance (1080p60 canvas, 4K display capture): 60.0 fps with 0 dropped frames,
0.02 ms GPU composition time, 0.1% CPU, 66 MB RAM.

## Building

Requires Rust (the version is pinned in `rust-toolchain.toml`; rustup installs it automatically) and the
MSVC Build Tools with CMake (the "C++ CMake tools" component; it builds libopus). The Windows SDK
(`fxc.exe`) is optional — precompiled shader bytecode is in the repo.

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

MIT — see [`LICENSE`](LICENSE).

## Third-party software

Capture Studio stands on the shoulders of open-source projects. Many thanks to their authors:

- [egui](https://github.com/emilk/egui) by Emil Ernerfeldt — the user interface (MIT / Apache-2.0);
- [windows-rs](https://github.com/microsoft/windows-rs) by Microsoft — Windows API bindings (MIT / Apache-2.0);
- [rubato](https://github.com/HEnquist/rubato) by Henrik Enquist — audio resampling and clock-drift
  compensation (MIT / Apache-2.0);
- [nnnoiseless](https://github.com/jneem/nnnoiseless) by Joe Neeman, a Rust port of
  [RNNoise](https://github.com/xiph/rnnoise) by Jean-Marc Valin, Mozilla and the Xiph.Org Foundation —
  noise suppression (BSD-3-Clause);
- NVIDIA Video Codec SDK header `nvEncodeAPI.h` — bindings for the NVENC encoder (MIT);
- [AMD AMF SDK](https://github.com/GPUOpen-LibrariesAndSDKs/AMF) headers by Advanced Micro Devices —
  interface declarations for the AMF encoder (MIT);
- [Intel® Video Processing Library (oneVPL)](https://github.com/intel/libvpl) headers by Intel —
  bindings for the Quick Sync Video encoder (MIT);
- [OpenH264](https://www.openh264.org/) by Cisco — the software H.264 encoder. OpenH264 Video Codec
  provided by Cisco Systems, Inc. The binary module is downloaded from Cisco on your request and is
  not part of Capture Studio; bindings via [openh264-rs](https://crates.io/crates/openh264-sys2)
  by Ralf Biedert (BSD-2-Clause), loading via [libloading](https://github.com/nagisa/rust_libloading)
  by Simonas Kazlauskas (ISC);
- [Opus](https://opus-codec.org/) by Xiph.Org, Skype, Octasic, Jean-Marc Valin, Timothy B. Terriberry and
  others — the Opus audio encoder (BSD-3-Clause), built via
  [opus-head-sys](https://github.com/xnorpx/rust-opus) by Marcus Asteborg (MIT / Apache-2.0);
- [bzip2-rs](https://github.com/trifectatechfoundation/bzip2-rs) and
  [libbzip2-rs](https://github.com/trifectatechfoundation/libbzip2-rs) by the Trifecta Tech Foundation,
  based on bzip2 by Julian Seward — unpacking the OpenH264 download (MIT / Apache-2.0, bzip2-1.0.6);
- [image](https://github.com/image-rs/image), [fluent-rs](https://github.com/projectfluent/fluent-rs),
  [serde](https://github.com/serde-rs/serde), [toml](https://github.com/toml-rs/toml),
  [tracing](https://github.com/tokio-rs/tracing), [RustCrypto sha2](https://github.com/RustCrypto/hashes)
  and other crates (MIT / Apache-2.0);
- fonts [IBM Plex Sans](https://github.com/IBM/plex) and [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono)
  (SIL Open Font License 1.1).

The full list with authors and the license notices is in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).
