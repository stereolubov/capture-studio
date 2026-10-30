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
| [AMD AMF SDK](https://github.com/GPUOpen-LibrariesAndSDKs/AMF) headers (v1.4.36) — interface declarations in `crates/cs-encode/src/amf.rs` are written from them | Advanced Micro Devices, Inc. | MIT (notice below) | AMF hardware encoder |
| [Intel® Video Processing Library](https://github.com/intel/libvpl) headers (v2.15.0) — Rust bindings in `crates/cs-encode/src/qsv/sys.rs` are generated from them | Intel Corporation | MIT (notice below) | Quick Sync Video hardware encoder |
| [openh264-sys2](https://crates.io/crates/openh264-sys2) (openh264-rs) | Ralf Biedert; bindings generated from OpenH264 headers © Cisco Systems | BSD-2-Clause (notice below) | Bindings to the OpenH264 software encoder (dynamic loading only) |
| [libloading](https://github.com/nagisa/rust_libloading) | Simonas Kazlauskas | ISC | Loading the OpenH264 module |
| [sha2](https://github.com/RustCrypto/hashes) and its dependencies (`digest`, `block-buffer`, `crypto-common`, `generic-array`, `typenum`, `cpufeatures`) | RustCrypto Developers, Bartłomiej Kamiński, Paho Lurie-Gregg, Andre Bogus | MIT OR Apache-2.0 (`generic-array`: MIT) | Verifying the OpenH264 module hash |
| [bzip2](https://github.com/trifectatechfoundation/bzip2-rs) | Alex Crichton, Trifecta Tech Foundation | MIT OR Apache-2.0 | Unpacking the OpenH264 download |
| [libbz2-rs-sys](https://github.com/trifectatechfoundation/libbzip2-rs) | Trifecta Tech Foundation; based on bzip2 by Julian R Seward | bzip2-1.0.6 (notice below) | bzip2 decompression |

**OpenH264 Video Codec provided by Cisco Systems, Inc.** The OpenH264 binary module
(`openh264-2.6.0-win64.dll`) is not distributed with Capture Studio: it is downloaded from Cisco's
server at the user's request and is licensed by Cisco under its own terms
([BINARY_LICENSE.txt](https://www.openh264.org/BINARY_LICENSE.txt)).
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

## AMD AMF SDK headers (MIT)

```
Copyright (c) 2018 Advanced Micro Devices, Inc. All rights reserved.

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

## Intel® Video Processing Library headers (MIT)

```
Copyright (c) 2020 Intel Corporation

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

## openh264-rs (BSD-2-Clause)

openh264-rs © Ralf Biedert and contributors; the bindings are generated from the OpenH264 headers:

```
Copyright (c) 2013, Cisco Systems
All rights reserved.

Redistribution and use in source and binary forms, with or without modification, are permitted
provided that the following conditions are met:

* Redistributions of source code must retain the above copyright notice, this list of conditions
  and the following disclaimer.

* Redistributions in binary form must reproduce the above copyright notice, this list of
  conditions and the following disclaimer in the documentation and/or other materials provided
  with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR
IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER
IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT
OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## libbzip2-rs / bzip2 (bzip2-1.0.6)

```
The original program, "bzip2", the associated library "libbzip2", and all documentation, are
Copyright (C) 1996-2021 Julian R Seward.
Copyright (C) 2019-2020 Federico Mena Quintero
Copyright (C) 2021 Micah Snyder

This Rust translation, "libbzip2-rs" is a derived work based on "bzip2" and "libbzip2", and is
Copyright (C) 2024-2025 Trifecta Tech Foundation and contributors

Redistribution and use in source and binary forms, with or without modification, are permitted
provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions
   and the following disclaimer.

2. The origin of this software must not be misrepresented; you must not claim that you wrote the
   original software. If you use this software in a product, an acknowledgment in the product
   documentation would be appreciated but is not required.

3. Altered source versions must be plainly marked as such, and must not be misrepresented as
   being the original software.

4. The name of the author may not be used to endorse or promote products derived from this
   software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE AUTHOR "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING,
BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
ARE DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF
THE POSSIBILITY OF SUCH DAMAGE.
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
