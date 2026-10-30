//! AMD AMF (§9.1): H.264 на GPU AMD, вход — NV12-текстура D3D11 без копий.
//!
//! `amfrt64.dll` загружается динамически (её ставит драйвер AMD; без драйвера — «энкодер
//! недоступен»). Интерфейсы AMF — таблицы методов, описанные в C-разделе заголовков AMF SDK
//! (GPUOpen AMF v1.4.36, MIT); здесь объявлены только используемые методы, остальные места
//! заняты заглушками того же размера — порядок сверен с заголовками (ADR-0013).
//!
//! Кадр оборачивается в поверхность AMF (`CreateSurfaceFromDX11Native`) и подаётся в энкодер;
//! текстура держится, пока не вышел её пакет, а число кадров внутри энкодера ограничено
//! (`pipeline_depth`), чтобы конвертер не перезаписал текстуру, которую энкодер ещё читает.
//! B-кадры не опорные, время декодирования — как у NVENC: время подачи со сдвигом в кадр.
//!
//! Объявления интерфейсов переписаны с заголовков AMF SDK, которые распространяются по лицензии MIT:
//!
//! Copyright (c) 2018 Advanced Micro Devices, Inc. All rights reserved.
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy of this software
//! and associated documentation files (the "Software"), to deal in the Software without
//! restriction, including without limitation the rights to use, copy, modify, merge, publish,
//! distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
//! Software is furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all copies or
//! substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
//! BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//! NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//! DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use crate::{EncodeError, EncoderOptions, H264Profile, MAX_B_FRAMES, Packet, RateControl, VideoSettings};
use std::collections::VecDeque;
use std::ffi::c_void;
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{GUID, Interface, PCWSTR, s, w};

/// Сколько кадров AMF держит у себя: B-кадры + запас очереди. Больше в энкодер не подаётся.
pub fn pipeline_depth(opts: &EncoderOptions) -> usize {
    usize::from(opts.b_frames.min(MAX_B_FRAMES)) + 4
}

/// Предел очереди кадров на подачу: дольше — кадры отбрасываются, счётчик растёт.
const MAX_QUEUED: usize = 8;
/// Версия заголовков, по которым написан модуль: 1.4.36.
const HEADER_VERSION: u64 = (1 << 48) | (4 << 32) | (36 << 16);

type AmfResult = i32;
const AMF_OK: AmfResult = 0;
const AMF_OUT_OF_MEMORY: AmfResult = 6;
const AMF_EOF: AmfResult = 23;
const AMF_REPEAT: AmfResult = 24;
const AMF_INPUT_FULL: AmfResult = 25;
const AMF_NEED_MORE_INPUT: AmfResult = 44;

const AMF_DX11_0: i32 = 110;
const AMF_SURFACE_NV12: i32 = 1;
const IID_AMF_BUFFER: GUID =
    GUID::from_values(0xb04b7248, 0xb6f0, 0x4321, [0xb6, 0x91, 0xba, 0xa4, 0x74, 0x0f, 0x9f, 0xcb]);

fn result_text(r: AmfResult) -> String {
    const NAMES: [&str; 46] = [
        "OK",
        "FAIL",
        "UNEXPECTED",
        "ACCESS_DENIED",
        "INVALID_ARG",
        "OUT_OF_RANGE",
        "OUT_OF_MEMORY",
        "INVALID_POINTER",
        "NO_INTERFACE",
        "NOT_IMPLEMENTED",
        "NOT_SUPPORTED",
        "NOT_FOUND",
        "ALREADY_INITIALIZED",
        "NOT_INITIALIZED",
        "INVALID_FORMAT",
        "WRONG_STATE",
        "FILE_NOT_OPEN",
        "NO_DEVICE",
        "DIRECTX_FAILED",
        "OPENCL_FAILED",
        "GLX_FAILED",
        "XV_FAILED",
        "ALSA_FAILED",
        "EOF",
        "REPEAT",
        "INPUT_FULL",
        "RESOLUTION_CHANGED",
        "RESOLUTION_UPDATED",
        "INVALID_DATA_TYPE",
        "INVALID_RESOLUTION",
        "CODEC_NOT_SUPPORTED",
        "SURFACE_FORMAT_NOT_SUPPORTED",
        "SURFACE_MUST_BE_SHARED",
        "DECODER_NOT_PRESENT",
        "DECODER_SURFACE_ALLOCATION_FAILED",
        "DECODER_NO_FREE_SURFACES",
        "ENCODER_NOT_PRESENT",
        "DEM_ERROR",
        "DEM_PROPERTY_READONLY",
        "DEM_REMOTE_DISPLAY_CREATE_FAILED",
        "DEM_START_ENCODING_FAILED",
        "DEM_QUERY_OUTPUT_FAILED",
        "TAN_CLIPPING_WAS_REQUIRED",
        "TAN_UNSUPPORTED_VERSION",
        "NEED_MORE_INPUT",
        "VULKAN_FAILED",
    ];
    NAMES.get(r as usize).map_or_else(|| format!("AMF_RESULT {r}"), |n| format!("AMF_{n}"))
}

// ---- Раскладка AMF (C-раздел заголовков SDK) ----

/// `AMFVariantStruct`: тип + объединение (наибольший член — 4 float), 24 байта.
#[repr(C)]
#[derive(Clone, Copy)]
struct Variant {
    ty: i32,
    value: VariantValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
union VariantValue {
    boolean: bool,
    int64: i64,
    pair: [i32; 2],
    _vec4: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<Variant>() == 24);

const VARIANT_BOOL: i32 = 1;
const VARIANT_INT64: i32 = 2;
const VARIANT_SIZE: i32 = 5;
const VARIANT_RATE: i32 = 7;

impl Variant {
    fn empty() -> Self {
        Self { ty: 0, value: VariantValue { _vec4: [0.0; 4] } }
    }

    fn int(v: i64) -> Self {
        let mut x = Self::empty();
        x.ty = VARIANT_INT64;
        x.value.int64 = v;
        x
    }

    fn boolean(v: bool) -> Self {
        let mut x = Self::empty();
        x.ty = VARIANT_BOOL;
        x.value.boolean = v;
        x
    }

    /// `AMFSize` / `AMFRate`: пара 32-битных чисел.
    fn pair(ty: i32, a: u32, b: u32) -> Self {
        let mut x = Self::empty();
        x.ty = ty;
        x.value.pair = [a as i32, b as i32];
        x
    }
}

type This = *mut c_void;

/// `AMFInterface` + `AMFPropertyStorage`: 13 методов.
#[repr(C)]
struct StorageVtbl {
    _acquire: usize,
    release: unsafe extern "system" fn(This) -> i32,
    query_interface: unsafe extern "system" fn(This, *const GUID, *mut This) -> AmfResult,
    set_property: unsafe extern "system" fn(This, PCWSTR, Variant) -> AmfResult,
    get_property: unsafe extern "system" fn(This, PCWSTR, *mut Variant) -> AmfResult,
    /// HasProperty … RemoveObserver.
    _rest: [usize; 8],
}

/// `AMFContextVtbl`.
#[repr(C)]
struct ContextVtbl {
    base: StorageVtbl,
    terminate: unsafe extern "system" fn(This) -> AmfResult,
    /// InitDX9, GetDX9Device, LockDX9, UnlockDX9.
    _dx9: [usize; 4],
    init_dx11: unsafe extern "system" fn(This, *mut c_void, i32) -> AmfResult,
    /// GetDX11Device … CreateSurfaceFromDX9Native (индексы 19–48).
    _middle: [usize; 30],
    create_surface_from_dx11_native: unsafe extern "system" fn(This, *mut c_void, *mut This, *mut c_void) -> AmfResult,
}

/// `AMFComponentVtbl` (`AMFPropertyStorageEx` + компонент).
#[repr(C)]
struct ComponentVtbl {
    base: StorageVtbl,
    /// GetPropertiesInfoCount, GetPropertyInfoAt, GetPropertyInfo, ValidateProperty.
    _info: [usize; 4],
    init: unsafe extern "system" fn(This, i32, i32, i32) -> AmfResult,
    _reinit: usize,
    terminate: unsafe extern "system" fn(This) -> AmfResult,
    drain: unsafe extern "system" fn(This) -> AmfResult,
    _flush: usize,
    submit_input: unsafe extern "system" fn(This, This) -> AmfResult,
    query_output: unsafe extern "system" fn(This, *mut This) -> AmfResult,
}

/// `AMFDataVtbl` (общая часть поверхности и буфера).
#[repr(C)]
struct DataVtbl {
    base: StorageVtbl,
    /// GetMemoryType, Duplicate, Convert, Interop, GetDataType, IsReusable.
    _misc: [usize; 6],
    set_pts: unsafe extern "system" fn(This, i64),
    get_pts: unsafe extern "system" fn(This) -> i64,
    _duration: [usize; 2],
}

/// `AMFBufferVtbl`.
#[repr(C)]
struct BufferVtbl {
    data: DataVtbl,
    _set_size: usize,
    get_size: unsafe extern "system" fn(This) -> usize,
    get_native: unsafe extern "system" fn(This) -> *mut c_void,
}

/// `AMFFactoryVtbl` (без `AMFInterface`: фабрика не считает ссылки).
#[repr(C)]
struct FactoryVtbl {
    create_context: unsafe extern "system" fn(This, *mut This) -> AmfResult,
    create_component: unsafe extern "system" fn(This, This, PCWSTR, *mut This) -> AmfResult,
}

/// Таблица методов объекта AMF (первое поле объекта — указатель на неё).
///
/// # Safety
/// `obj` — живой объект AMF с таблицей вида `T`.
unsafe fn vt<T>(obj: This) -> &'static T {
    // SAFETY: по условию функции.
    unsafe { &**obj.cast::<*const T>() }
}

/// Владеющая ссылка на объект AMF (`Release` при уничтожении).
struct Obj(This);

impl Obj {
    fn raw(&self) -> This {
        self.0
    }
}

impl Drop for Obj {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: каждый объект AMF начинается с `AMFInterface`; ссылка наша.
            unsafe {
                (vt::<StorageVtbl>(self.0).release)(self.0);
            }
        }
    }
}

type InitFn = unsafe extern "system" fn(u64, *mut This) -> AmfResult;
type QueryVersionFn = unsafe extern "system" fn(*mut u64) -> AmfResult;

struct Library(HMODULE);

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: библиотека загружена в `new` и больше не используется (объекты AMF освобождены).
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}

/// Кадр, отданный энкодеру и ждущий пакета.
struct InFlight {
    pts: i64,
    /// Держит текстуру живой и не даёт конвертеру её переиспользовать, пока энкодер читает.
    _texture: ID3D11Texture2D,
}

pub struct AmfEncoder {
    // Порядок полей = порядок уничтожения: энкодер, контекст, затем библиотека.
    encoder: Obj,
    context: Obj,
    _lib: Library,
    in_flight: VecDeque<InFlight>,
    queued: VecDeque<(ID3D11Texture2D, i64)>,
    depth: usize,
    /// Сдвиг времени декодирования (кадр при B-кадрах, иначе 0).
    dts_shift: i64,
    draining: bool,
    pub name: String,
    pub dropped: u64,
}

// SAFETY: объекты AMF используются из одного потока (поток энкодера записи); AMF не привязывает
// их к потоку-создателю.
unsafe impl Send for AmfEncoder {}

impl AmfEncoder {
    pub fn new(device: &ID3D11Device, s: &VideoSettings, opts: &EncoderOptions) -> Result<Self, EncodeError> {
        let vendor = crate::adapter_vendor(device);
        if vendor != 0x1002 {
            return Err(EncodeError::Unavailable(format!("AMF работает на GPU AMD, а сцена — на GPU {vendor:04x}")));
        }
        // SAFETY: загрузка библиотеки драйвера и вызовы по таблицам методов из заголовков AMF;
        // каждый полученный объект сразу оборачивается в `Obj`.
        unsafe {
            let lib = Library(
                LoadLibraryW(w!("amfrt64.dll"))
                    .map_err(|_| EncodeError::Unavailable("нет драйвера AMD с AMF (amfrt64.dll)".into()))?,
            );
            let (Some(init), Some(query)) =
                (GetProcAddress(lib.0, s!("AMFInit")), GetProcAddress(lib.0, s!("AMFQueryVersion")))
            else {
                return Err(EncodeError::Unavailable("amfrt64.dll без точек входа AMF".into()));
            };
            let init: InitFn = std::mem::transmute(init);
            let query: QueryVersionFn = std::mem::transmute(query);
            let mut runtime = 0u64;
            if query(&mut runtime) != AMF_OK {
                return Err(EncodeError::Unavailable("AMFQueryVersion".into()));
            }
            // Просим не новее своей версии заголовков: старая среда выполнения новее не примет.
            let mut factory: This = std::ptr::null_mut();
            let r = init(runtime.min(HEADER_VERSION), &mut factory);
            if r != AMF_OK || factory.is_null() {
                return Err(EncodeError::Unavailable(format!("AMFInit: {}", result_text(r))));
            }
            let f = vt::<FactoryVtbl>(factory);

            let mut ctx: This = std::ptr::null_mut();
            let r = (f.create_context)(factory, &mut ctx);
            let context = Obj(ctx);
            if r != AMF_OK || ctx.is_null() {
                return Err(EncodeError::Unavailable(format!("контекст AMF: {}", result_text(r))));
            }
            let r = (vt::<ContextVtbl>(ctx).init_dx11)(ctx, device.as_raw(), AMF_DX11_0);
            if r != AMF_OK {
                return Err(EncodeError::Unavailable(format!("AMF и D3D11-устройство: {}", result_text(r))));
            }
            let mut comp: This = std::ptr::null_mut();
            let r = (f.create_component)(factory, ctx, w!("AMFVideoEncoderVCE_AVC"), &mut comp);
            let encoder = Obj(comp);
            if r != AMF_OK || comp.is_null() {
                // Нет блока кодирования или все сессии заняты (§9.1).
                return Err(match r {
                    AMF_OUT_OF_MEMORY => EncodeError::SessionLimit,
                    _ => EncodeError::Unavailable(format!("энкодер AMF H.264: {}", result_text(r))),
                });
            }

            let mut enc = Self {
                encoder,
                context,
                _lib: lib,
                in_flight: VecDeque::new(),
                queued: VecDeque::new(),
                depth: pipeline_depth(opts),
                dts_shift: 0,
                draining: false,
                name: String::new(),
                dropped: 0,
            };
            let b_frames = enc.configure(s, opts)?;
            let r = (vt::<ComponentVtbl>(comp).init)(comp, AMF_SURFACE_NV12, s.width as i32, s.height as i32);
            if r != AMF_OK {
                return Err(EncodeError::Unavailable(format!("инициализация AMF: {}", result_text(r))));
            }
            enc.dts_shift = if b_frames > 0 { s.frame_duration() } else { 0 };
            enc.name = format!("AMD AMF H.264 ({})", quality_name(opts.preset));
            tracing::info!(
                "видеоэнкодер: {}, {}×{} {} кбит/с, B-кадров {b_frames}{}",
                enc.name,
                s.width,
                s.height,
                s.bitrate_kbps,
                if opts.low_latency { ", низкая задержка" } else { "" }
            );
            Ok(enc)
        }
    }

    fn set(&self, name: PCWSTR, v: Variant) -> AmfResult {
        let comp = self.encoder.raw();
        // SAFETY: компонент жив; имя — статическая строка UTF-16 с нулём.
        unsafe { (vt::<StorageVtbl>(comp).set_property)(comp, name, v) }
    }

    /// Свойство, которое драйвер вправе не поддержать: отказ — в журнал отладки.
    fn try_set(&self, name: PCWSTR, v: Variant) {
        let r = self.set(name, v);
        if r != AMF_OK {
            // SAFETY: `name` — строка из `w!`.
            tracing::debug!("AMF {}: {}", unsafe { name.display() }, result_text(r));
        }
    }

    /// Свойства до `Init`. Возвращает принятое число B-кадров.
    fn configure(&self, s: &VideoSettings, opts: &EncoderOptions) -> Result<u8, EncodeError> {
        // Назначение задаётся первым: оно сбрасывает остальные свойства к своим значениям.
        let usage = if opts.low_latency { 2 } else { 0 }; // LOW_LATENCY / TRANSCODING
        let r = self.set(w!("Usage"), Variant::int(usage));
        if r != AMF_OK {
            return Err(EncodeError::Unavailable(format!("AMF Usage: {}", result_text(r))));
        }
        let profile = match opts.profile {
            H264Profile::Baseline => 66,
            H264Profile::Main => 77,
            H264Profile::High => 100,
        };
        self.try_set(w!("Profile"), Variant::int(profile));
        self.try_set(w!("QualityPreset"), Variant::int(quality_preset(opts.preset)));
        self.try_set(w!("FrameSize"), Variant::pair(VARIANT_SIZE, s.width, s.height));
        self.try_set(w!("FrameRate"), Variant::pair(VARIANT_RATE, s.fps_num.max(1), s.fps_den.max(1)));
        // SPS/PPS перед каждым ключевым кадром — поток Annex B, как у остальных энкодеров.
        self.try_set(w!("IDRPeriod"), Variant::int(i64::from(s.gop())));
        self.try_set(w!("HeaderInsertionSpacing"), Variant::int(i64::from(s.gop())));
        self.try_set(w!("EnableVBAQ"), Variant::boolean(true));
        self.try_set(w!("RateControlPreanalysisEnable"), Variant::int(i64::from(opts.lookahead)));

        let bps = i64::from(s.bitrate_kbps.max(100)) * 1000;
        match s.rate_control {
            RateControl::Cbr => {
                self.try_set(w!("RateControlMethod"), Variant::int(1));
                self.try_set(w!("TargetBitrate"), Variant::int(bps));
                self.try_set(w!("PeakBitrate"), Variant::int(bps));
                self.try_set(w!("VBVBufferSize"), Variant::int(bps));
                self.try_set(w!("FillerDataEnable"), Variant::boolean(true));
                self.try_set(w!("EnforceHRD"), Variant::boolean(true));
            }
            RateControl::Vbr => {
                self.try_set(w!("RateControlMethod"), Variant::int(2)); // PEAK_CONSTRAINED_VBR
                self.try_set(w!("TargetBitrate"), Variant::int(bps));
                self.try_set(w!("PeakBitrate"), Variant::int(bps * 3 / 2));
            }
            RateControl::Cqp(q) => {
                let q = i64::from(q.clamp(1, 51));
                self.try_set(w!("RateControlMethod"), Variant::int(0));
                self.try_set(w!("QPI"), Variant::int((q - 2).max(1)));
                self.try_set(w!("QPP"), Variant::int(q));
                self.try_set(w!("QPB"), Variant::int(q));
            }
        }

        // B-кадры поддерживает не каждый блок кодирования: отказ — кодируем без них.
        let wanted = opts.b_frames.min(MAX_B_FRAMES);
        let mut b_frames = 0;
        if wanted > 0 {
            if self.set(w!("BPicturesPattern"), Variant::int(i64::from(wanted))) == AMF_OK {
                b_frames = wanted;
                self.try_set(w!("BReferenceEnable"), Variant::boolean(false));
            } else {
                tracing::info!("AMF: B-кадры не поддерживаются этим GPU — без них");
            }
        }
        Ok(b_frames)
    }

    /// Ставит кадр в очередь на кодирование (`pts` — 100 нс от начала записи).
    pub fn push(&mut self, texture: ID3D11Texture2D, pts: i64) {
        if self.queued.len() >= MAX_QUEUED {
            self.queued.pop_front();
            self.dropped += 1;
        }
        self.queued.push_back((texture, pts));
    }

    /// Подаёт кадры из очереди (не больше `depth` внутри энкодера) и забирает готовые пакеты.
    pub fn pump(&mut self, out: &mut Vec<Packet>) -> Result<bool, EncodeError> {
        self.collect(out)?;
        while self.in_flight.len() < self.depth {
            let Some((tex, pts)) = self.queued.pop_front() else { break };
            if !self.submit(&tex, pts)? {
                // Очередь энкодера полна — кадр подождёт следующего вызова.
                self.queued.push_front((tex, pts));
                break;
            }
            self.in_flight.push_back(InFlight { pts, _texture: tex });
            self.collect(out)?;
        }
        Ok(false)
    }

    /// `false` — энкодер пока не принимает кадр.
    fn submit(&self, tex: &ID3D11Texture2D, pts: i64) -> Result<bool, EncodeError> {
        let (ctx, comp) = (self.context.raw(), self.encoder.raw());
        // SAFETY: поверхность создаётся из NV12-текстуры устройства контекста; своя ссылка на
        // поверхность освобождается после подачи (энкодер держит свою).
        unsafe {
            let mut surf: This = std::ptr::null_mut();
            let r = (vt::<ContextVtbl>(ctx).create_surface_from_dx11_native)(
                ctx,
                tex.as_raw(),
                &mut surf,
                std::ptr::null_mut(),
            );
            let surf = Obj(surf);
            if r != AMF_OK || surf.raw().is_null() {
                return Err(EncodeError::Failed(format!("поверхность AMF: {}", result_text(r))));
            }
            (vt::<DataVtbl>(surf.raw()).set_pts)(surf.raw(), pts);
            match (vt::<ComponentVtbl>(comp).submit_input)(comp, surf.raw()) {
                AMF_OK | AMF_NEED_MORE_INPUT => Ok(true),
                AMF_INPUT_FULL => Ok(false),
                r => Err(EncodeError::Failed(format!("подача кадра AMF: {}", result_text(r)))),
            }
        }
    }

    /// Забирает готовые пакеты без ожидания. `true` — энкодер выдал всё (после `Drain`).
    fn collect(&mut self, out: &mut Vec<Packet>) -> Result<bool, EncodeError> {
        let comp = self.encoder.raw();
        loop {
            // SAFETY: выходные данные — буфер AMF; данные копируются до освобождения ссылок.
            unsafe {
                let mut data: This = std::ptr::null_mut();
                let r = (vt::<ComponentVtbl>(comp).query_output)(comp, &mut data);
                let data = Obj(data);
                match r {
                    AMF_OK if !data.raw().is_null() => {}
                    AMF_OK | AMF_REPEAT => return Ok(false),
                    AMF_EOF => return Ok(true),
                    r => return Err(EncodeError::Failed(format!("пакет AMF: {}", result_text(r)))),
                }
                let mut buf: This = std::ptr::null_mut();
                let r = (vt::<StorageVtbl>(data.raw()).query_interface)(data.raw(), &IID_AMF_BUFFER, &mut buf);
                let buf = Obj(buf);
                if r != AMF_OK || buf.raw().is_null() {
                    return Err(EncodeError::Failed(format!("пакет AMF без буфера: {}", result_text(r))));
                }
                let b = vt::<BufferVtbl>(buf.raw());
                let ptr = (b.get_native)(buf.raw()).cast::<u8>();
                let len = (b.get_size)(buf.raw());
                let bytes = if ptr.is_null() { Vec::new() } else { std::slice::from_raw_parts(ptr, len).to_vec() };
                let pts = (b.data.get_pts)(buf.raw());
                let mut kind = Variant::empty();
                let keyframe = (b.data.base.get_property)(buf.raw(), w!("OutputDataType"), &mut kind) == AMF_OK
                    && kind.ty == VARIANT_INT64
                    && matches!(kind.value.int64, 0 | 1); // IDR, I
                // Пакеты идут в порядке декодирования: k-му пакету — время k-го поданного кадра.
                let dts = self.in_flight.pop_front().map_or(pts, |f| f.pts - self.dts_shift);
                out.push(Packet { data: bytes, pts, dts, keyframe });
            }
        }
    }

    /// Дописывает очередь и забирает все оставшиеся пакеты (до 3 с).
    pub fn drain(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !self.queued.is_empty() && std::time::Instant::now() < deadline {
            self.pump(out)?;
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let comp = self.encoder.raw();
        while !self.draining && std::time::Instant::now() < deadline {
            // SAFETY: штатное завершение потока компонента.
            match unsafe { (vt::<ComponentVtbl>(comp).drain)(comp) } {
                AMF_OK => self.draining = true,
                AMF_INPUT_FULL => {
                    self.collect(out)?;
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                r => return Err(EncodeError::Failed(format!("дренаж AMF: {}", result_text(r)))),
            }
        }
        while std::time::Instant::now() < deadline {
            if self.collect(out)? {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Ok(())
    }
}

impl Drop for AmfEncoder {
    fn drop(&mut self) {
        // SAFETY: компонент и контекст живы; после Terminate ссылки освобождаются полями.
        unsafe {
            let comp = self.encoder.raw();
            let _ = (vt::<ComponentVtbl>(comp).terminate)(comp);
            let ctx = self.context.raw();
            let _ = (vt::<ContextVtbl>(ctx).terminate)(ctx);
        }
    }
}

/// Пресет P1–P7 → QualityPreset AMF: SPEED (1), BALANCED (0), QUALITY (2).
fn quality_preset(preset: u8) -> i64 {
    match preset.clamp(1, 7) {
        1 | 2 => 1,
        3..=5 => 0,
        _ => 2,
    }
}

fn quality_name(preset: u8) -> &'static str {
    match quality_preset(preset) {
        1 => "скорость",
        2 => "качество",
        _ => "баланс",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Места методов в таблицах — как в C-разделе заголовков AMF SDK 1.4.36.
    #[test]
    fn vtable_slots_match_headers() {
        let slot = std::mem::size_of::<usize>();
        assert_eq!(std::mem::size_of::<StorageVtbl>(), 13 * slot);
        assert_eq!(std::mem::offset_of!(ContextVtbl, terminate), 13 * slot);
        assert_eq!(std::mem::offset_of!(ContextVtbl, init_dx11), 18 * slot);
        assert_eq!(std::mem::offset_of!(ContextVtbl, create_surface_from_dx11_native), 49 * slot);
        assert_eq!(std::mem::offset_of!(ComponentVtbl, init), 17 * slot);
        assert_eq!(std::mem::offset_of!(ComponentVtbl, terminate), 19 * slot);
        assert_eq!(std::mem::offset_of!(ComponentVtbl, drain), 20 * slot);
        assert_eq!(std::mem::offset_of!(ComponentVtbl, submit_input), 22 * slot);
        assert_eq!(std::mem::offset_of!(ComponentVtbl, query_output), 23 * slot);
        assert_eq!(std::mem::offset_of!(DataVtbl, set_pts), 19 * slot);
        assert_eq!(std::mem::offset_of!(DataVtbl, get_pts), 20 * slot);
        assert_eq!(std::mem::offset_of!(BufferVtbl, get_size), 24 * slot);
        assert_eq!(std::mem::offset_of!(BufferVtbl, get_native), 25 * slot);
    }

    #[test]
    fn variant_layout() {
        assert_eq!(std::mem::offset_of!(Variant, value), 8);
        let v = Variant::pair(VARIANT_SIZE, 1920, 1080);
        // SAFETY: значение записано как пара.
        assert_eq!((v.ty, unsafe { v.value.pair }), (5, [1920, 1080]));
    }

    #[test]
    fn presets_map_to_quality() {
        assert_eq!((1..=7).map(quality_preset).collect::<Vec<_>>(), [1, 1, 0, 0, 0, 2, 2]);
    }
}
