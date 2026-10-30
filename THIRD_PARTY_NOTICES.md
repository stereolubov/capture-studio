# Third-party notices

Capture Studio is MIT-licensed (see [`LICENSE`](LICENSE)) and is built on the work of many
open-source authors. Thank you! This file lists the components linked into the program and the
notices their licenses ask us to keep.

The table covers direct dependencies. Their own dependencies (all under permissive licenses,
checked by `cargo deny`, see [`deny.toml`](deny.toml)) will be listed with full license texts in
release builds.

## Libraries

| Component | Authors | License | Used for |
|---|---|---|---|
| [egui](https://github.com/emilk/egui) | Emil Ernerfeldt and contributors | MIT OR Apache-2.0 | User interface |
| [windows-rs](https://github.com/microsoft/windows-rs) (`windows`, `windows-core`, `windows-sys`, `windows-numerics`) | Microsoft | MIT OR Apache-2.0 | Windows API: Direct3D 11, DXGI, WASAPI, Media Foundation |
| [rubato](https://github.com/HEnquist/rubato), [audioadapter-buffers](https://github.com/HEnquist/audioadapter-buffers-rs) | Henrik Enquist | MIT OR Apache-2.0 | Audio resampling and clock-drift compensation |
| [nnnoiseless](https://github.com/jneem/nnnoiseless) (Rust port of [RNNoise](https://github.com/xiph/rnnoise)) | Joe Neeman; RNNoise: Jean-Marc Valin, Mozilla, Xiph.Org Foundation | BSD-3-Clause (notice below) | Noise suppression filter |
| [image](https://github.com/image-rs/image), [png](https://github.com/image-rs/image-png) | The image-rs Developers | MIT OR Apache-2.0 | Image source, screenshot tests |
| [fluent-bundle](https://github.com/projectfluent/fluent-rs) | Zibi Braniecki, Staś Małolepszy, Caleb Maclennan, Bruce Mitchener | Apache-2.0 OR MIT | Localization |
| [unic-langid](https://github.com/zbraniecki/unic-locale) | Zibi Braniecki | MIT OR Apache-2.0 | Language identifiers |
| [serde](https://github.com/serde-rs/serde) | Erick Tryzelaar, David Tolnay | MIT OR Apache-2.0 | Settings serialization |
| [toml](https://github.com/toml-rs/toml) | toml-rs contributors | MIT OR Apache-2.0 | Settings file format |
| [tracing](https://github.com/tokio-rs/tracing), `tracing-subscriber` | Eliza Weisman, David Barsky, Tokio Contributors | MIT | Logging |
| [anyhow](https://github.com/dtolnay/anyhow), [thiserror](https://github.com/dtolnay/thiserror) | David Tolnay | MIT OR Apache-2.0 | Error handling |
| [parking_lot](https://github.com/Amanieu/parking_lot) | Amanieu d'Antras | MIT OR Apache-2.0 | Synchronization primitives |
| [crossbeam-channel](https://github.com/crossbeam-rs/crossbeam) | The Crossbeam Project Developers | MIT OR Apache-2.0 | Inter-thread queues |
| NVIDIA Video Codec SDK header `nvEncodeAPI.h` (via [nv-codec-headers](https://github.com/FFmpeg/nv-codec-headers)) — Rust bindings in `crates/cs-encode/src/nvenc/sys.rs` are generated from it | NVIDIA Corporation | MIT (notice below) | NVENC hardware encoder |
| [uuid](https://github.com/uuid-rs/uuid) | Ashley Mannix, Dylan DPC, Hunar Roop Kahlon | Apache-2.0 OR MIT | Scene and source identifiers |

## Fonts

| Font | Copyright | License |
|---|---|---|
| IBM Plex Sans | © 2017 IBM Corp., Reserved Font Name "Plex" | SIL Open Font License 1.1 ([`assets/fonts/OFL-IBMPlex.txt`](assets/fonts/OFL-IBMPlex.txt)) |
| JetBrains Mono | © 2020 The JetBrains Mono Project Authors | SIL Open Font License 1.1 ([`assets/fonts/OFL-JetBrainsMono.txt`](assets/fonts/OFL-JetBrainsMono.txt)) |

## NVIDIA nvEncodeAPI.h (MIT)

```
Copyright (c) 2010-2022 NVIDIA Corporation

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

## nnnoiseless / RNNoise (BSD-3-Clause)

```
Copyright (c) 2020, Joe Neeman
Copyright (c) 2017, Mozilla
Copyright (c) 2007-2017, Jean-Marc Valin
Copyright (c) 2005-2017, Xiph.Org Foundation
Copyright (c) 2003-2004, Mark Borgerding

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions
are met:

- Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

- Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

- Neither the name of the Xiph.Org Foundation nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE FOUNDATION
OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```
