//! Прямой NVENC (§9.1): H.264 на GPU NVIDIA, вход — NV12-текстура D3D11 без копий.
//!
//! `nvEncodeAPI64.dll` загружается динамически (её ставит драйвер; без драйвера — «энкодер
//! недоступен»), API 12.0 — драйвер ≥ 522.25. Синхронный режим: кадр регистрируется один раз
//! (пул конвертера), на время кодирования отображается (`map`), готовые пакеты забираются из
//! кольца выходных буферов по порядку подачи. SPS/PPS повторяются в каждом ключевом кадре —
//! поток Annex B, как у Media Foundation.
//!
//! B-кадры пока не включаются: перепаковка MKV → MP4 рассчитана на поток без перестановки
//! кадров (`ctts` — пункт 2.11 плана), поэтому DTS = PTS.

pub mod sys;

use crate::{EncodeError, Packet, RateControl, VideoSettings};
use std::collections::VecDeque;
use std::ffi::c_void;
use sys::*;
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{Interface, s, w};

/// Профиль H.264.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Profile {
    Baseline,
    Main,
    High,
}

/// Параметры NVENC сверх общих (§9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NvencOptions {
    /// Пресет P1 (быстрее) … P7 (качественнее).
    pub preset: u8,
    pub profile: H264Profile,
    /// Тюнинг «низкая задержка» (трансляция) вместо «высокое качество» (запись).
    pub low_latency: bool,
    /// Анализ наперёд (лучше распределение битрейта, +задержка на `LOOKAHEAD_FRAMES`).
    pub lookahead: bool,
}

impl Default for NvencOptions {
    fn default() -> Self {
        Self { preset: 5, profile: H264Profile::High, low_latency: false, lookahead: false }
    }
}

/// Глубина анализа наперёд, кадров.
pub const LOOKAHEAD_FRAMES: u16 = 8;
/// Минимальная версия API (12.0): `(major << 4) | minor`.
const MIN_API: u32 = 12 << 4;
/// Предел очереди кадров на подачу (как у MF): дольше — кадры отбрасываются, счётчик растёт.
const MAX_QUEUED: usize = 8;

/// Сколько кадров NVENC держит у себя (вход уже отдан, пакета ещё нет): для размера пула
/// текстур конвертера.
pub fn pipeline_depth(opts: &NvencOptions) -> usize {
    usize::from(if opts.lookahead { LOOKAHEAD_FRAMES } else { 0 }) + 2
}

type MaxVersionFn = unsafe extern "C" fn(*mut u32) -> NVENCSTATUS;
type CreateInstanceFn = unsafe extern "C" fn(*mut NV_ENCODE_API_FUNCTION_LIST) -> NVENCSTATUS;

/// Загруженная библиотека и таблица функций.
struct Api {
    lib: HMODULE,
    f: NV_ENCODE_API_FUNCTION_LIST,
}

impl Drop for Api {
    fn drop(&mut self) {
        // SAFETY: библиотека загружена в `load` и больше не используется (энкодер уничтожен).
        unsafe {
            let _ = FreeLibrary(self.lib);
        }
    }
}

fn status_text(s: NVENCSTATUS) -> String {
    const NAMES: [&str; 26] = [
        "SUCCESS",
        "NO_ENCODE_DEVICE",
        "UNSUPPORTED_DEVICE",
        "INVALID_ENCODERDEVICE",
        "INVALID_DEVICE",
        "DEVICE_NOT_EXIST",
        "INVALID_PTR",
        "INVALID_EVENT",
        "INVALID_PARAM",
        "INVALID_CALL",
        "OUT_OF_MEMORY",
        "ENCODER_NOT_INITIALIZED",
        "UNSUPPORTED_PARAM",
        "LOCK_BUSY",
        "NOT_ENOUGH_BUFFER",
        "INVALID_VERSION",
        "MAP_FAILED",
        "NEED_MORE_INPUT",
        "ENCODER_BUSY",
        "EVENT_NOT_REGISTERD",
        "GENERIC",
        "INCOMPATIBLE_CLIENT_KEY",
        "UNIMPLEMENTED",
        "RESOURCE_REGISTER_FAILED",
        "RESOURCE_NOT_REGISTERED",
        "RESOURCE_NOT_MAPPED",
    ];
    NAMES.get(s as usize).map_or_else(|| format!("NVENCSTATUS {s}"), |n| format!("NV_ENC_ERR_{n}"))
}

fn load() -> Result<Api, EncodeError> {
    // SAFETY: загрузка системной библиотеки драйвера; указатели функций приводятся к сигнатурам
    // из nvEncodeAPI.h; таблица заполняется библиотекой.
    unsafe {
        let lib = LoadLibraryW(w!("nvEncodeAPI64.dll"))
            .map_err(|_| EncodeError::Unavailable("нет драйвера NVIDIA с NVENC".into()))?;
        let api = Api { lib, f: std::mem::zeroed() };
        let max_version = GetProcAddress(lib, s!("NvEncodeAPIGetMaxSupportedVersion"));
        let create = GetProcAddress(lib, s!("NvEncodeAPICreateInstance"));
        let (Some(max_version), Some(create)) = (max_version, create) else {
            return Err(EncodeError::Unavailable("nvEncodeAPI64.dll без точек входа NVENC".into()));
        };
        let max_version: MaxVersionFn = std::mem::transmute(max_version);
        let create: CreateInstanceFn = std::mem::transmute(create);
        let mut version = 0u32;
        if max_version(&mut version) != _NVENCSTATUS_NV_ENC_SUCCESS || version < MIN_API {
            return Err(EncodeError::Unavailable(format!(
                "устаревший драйвер NVIDIA (NVENC API {}.{}), нужна версия ≥ 522.25",
                version >> 4,
                version & 0xf
            )));
        }
        let mut api = api;
        api.f.version = NV_ENCODE_API_FUNCTION_LIST_VER;
        let r = create(&mut api.f);
        if r != _NVENCSTATUS_NV_ENC_SUCCESS {
            return Err(EncodeError::Unavailable(format!("NvEncodeAPICreateInstance: {}", status_text(r))));
        }
        Ok(api)
    }
}

/// Вызов функции из таблицы: отсутствующая функция или код ошибки → `EncodeError`.
macro_rules! nv {
    ($api:expr, $name:ident ( $($arg:expr),* $(,)? )) => {{
        match $api.f.$name {
            Some(func) => {
                let r = func($($arg),*);
                if r == _NVENCSTATUS_NV_ENC_SUCCESS { Ok(()) } else { Err(r) }
            }
            None => Err(_NVENCSTATUS_NV_ENC_ERR_INVALID_PTR),
        }
    }};
}

/// Кадр, отданный энкодеру и ждущий пакета.
struct InFlight {
    slot: usize,
    mapped: NV_ENC_INPUT_PTR,
    /// Держит текстуру живой, пока энкодер её читает.
    _texture: ID3D11Texture2D,
}

pub struct NvencEncoder {
    api: Api,
    encoder: *mut c_void,
    bitstreams: Vec<NV_ENC_OUTPUT_PTR>,
    /// Зарегистрированные текстуры пула конвертера (сырой указатель → регистрация).
    registered: Vec<(usize, NV_ENC_REGISTERED_PTR)>,
    in_flight: VecDeque<InFlight>,
    queued: VecDeque<(ID3D11Texture2D, i64)>,
    next_slot: usize,
    frame_idx: u32,
    /// Отставание пакетов при анализе наперёд: «кадр принят» не значит, что готовы пакеты всех
    /// поданных кадров — последние `lookahead` ещё в анализе (как в NvEncoder из SDK).
    lookahead: usize,
    width: u32,
    height: u32,
    pub name: String,
    pub dropped: u64,
}

// SAFETY: сессия NVENC используется из одного потока (поток энкодера записи); указатели
// драйвера не привязаны к потоку-создателю.
unsafe impl Send for NvencEncoder {}

impl NvencEncoder {
    /// Ошибка с текстом драйвера (`nvEncGetLastErrorString`) — для лога и сообщения пользователю.
    fn fail(&self, what: &str, s: NVENCSTATUS) -> EncodeError {
        // SAFETY: строка драйвера живёт до следующего вызова API этой сессии; копируется сразу.
        let detail = unsafe {
            self.api
                .f
                .nvEncGetLastErrorString
                .filter(|_| !self.encoder.is_null())
                .map(|f| f(self.encoder))
                .filter(|p| !p.is_null())
                .map(|p| std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned())
                .filter(|d| !d.is_empty())
        };
        EncodeError::Failed(match detail {
            Some(d) => format!("{what}: {} ({d})", status_text(s)),
            None => format!("{what}: {}", status_text(s)),
        })
    }

    pub fn new(device: &ID3D11Device, s: &VideoSettings, opts: &NvencOptions) -> Result<Self, EncodeError> {
        let api = load()?;
        let mut enc = Self {
            api,
            encoder: std::ptr::null_mut(),
            bitstreams: Vec::new(),
            registered: Vec::new(),
            in_flight: VecDeque::new(),
            queued: VecDeque::new(),
            next_slot: 0,
            frame_idx: 0,
            lookahead: if opts.lookahead { usize::from(LOOKAHEAD_FRAMES) } else { 0 },
            width: s.width,
            height: s.height,
            name: String::new(),
            dropped: 0,
        };
        // SAFETY: структуры заполняются по nvEncodeAPI.h (нулевая инициализация + версии);
        // указатели на них живут до возврата из вызовов.
        unsafe {
            let mut open: NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS = std::mem::zeroed();
            open.version = NV_ENC_OPEN_ENCODE_SESSION_EX_PARAMS_VER;
            open.deviceType = _NV_ENC_DEVICE_TYPE_NV_ENC_DEVICE_TYPE_DIRECTX;
            open.device = device.as_raw();
            open.apiVersion = NVENCAPI_VERSION;
            if let Err(r) = nv!(enc.api, nvEncOpenEncodeSessionEx(&mut open, &mut enc.encoder)) {
                // Лимит параллельных сессий драйвер сообщает нехваткой памяти/устройства (§9.1).
                return Err(match r {
                    _NVENCSTATUS_NV_ENC_ERR_OUT_OF_MEMORY | _NVENCSTATUS_NV_ENC_ERR_NO_ENCODE_DEVICE => {
                        EncodeError::SessionLimit
                    }
                    _ => EncodeError::Unavailable(format!("сессия NVENC: {}", status_text(r))),
                });
            }

            let preset = match opts.preset.clamp(1, 7) {
                1 => NV_ENC_PRESET_P1_GUID,
                2 => NV_ENC_PRESET_P2_GUID,
                3 => NV_ENC_PRESET_P3_GUID,
                4 => NV_ENC_PRESET_P4_GUID,
                5 => NV_ENC_PRESET_P5_GUID,
                6 => NV_ENC_PRESET_P6_GUID,
                _ => NV_ENC_PRESET_P7_GUID,
            };
            let tuning = if opts.low_latency {
                NV_ENC_TUNING_INFO_NV_ENC_TUNING_INFO_LOW_LATENCY
            } else {
                NV_ENC_TUNING_INFO_NV_ENC_TUNING_INFO_HIGH_QUALITY
            };
            let mut preset_cfg: NV_ENC_PRESET_CONFIG = std::mem::zeroed();
            preset_cfg.version = NV_ENC_PRESET_CONFIG_VER;
            preset_cfg.presetCfg.version = NV_ENC_CONFIG_VER;
            nv!(
                enc.api,
                nvEncGetEncodePresetConfigEx(enc.encoder, NV_ENC_CODEC_H264_GUID, preset, tuning, &mut preset_cfg)
            )
            .map_err(|r| enc.fail("пресет NVENC", r))?;

            let mut cfg = preset_cfg.presetCfg;
            cfg.version = NV_ENC_CONFIG_VER;
            cfg.profileGUID = match opts.profile {
                H264Profile::Baseline => NV_ENC_H264_PROFILE_BASELINE_GUID,
                H264Profile::Main => NV_ENC_H264_PROFILE_MAIN_GUID,
                H264Profile::High => NV_ENC_H264_PROFILE_HIGH_GUID,
            };
            cfg.gopLength = s.gop();
            cfg.frameIntervalP = 1; // без B-кадров (см. описание модуля)
            let rc = &mut cfg.rcParams;
            let kbps = s.bitrate_kbps.max(100) * 1000;
            match s.rate_control {
                RateControl::Cbr => {
                    rc.rateControlMode = _NV_ENC_PARAMS_RC_MODE_NV_ENC_PARAMS_RC_CBR;
                    rc.averageBitRate = kbps;
                    rc.maxBitRate = kbps;
                    rc.vbvBufferSize = kbps; // буфер на 1 с — как рекомендуют для трансляций
                    rc.vbvInitialDelay = kbps;
                }
                RateControl::Vbr => {
                    rc.rateControlMode = _NV_ENC_PARAMS_RC_MODE_NV_ENC_PARAMS_RC_VBR;
                    rc.averageBitRate = kbps;
                    rc.maxBitRate = kbps * 3 / 2;
                }
                RateControl::Cqp(q) => {
                    let q = q.clamp(1, 51);
                    rc.rateControlMode = _NV_ENC_PARAMS_RC_MODE_NV_ENC_PARAMS_RC_CONSTQP;
                    rc.constQP = NV_ENC_QP { qpInterP: q, qpInterB: q, qpIntra: q.saturating_sub(2).max(1) };
                }
            }
            // Адаптивное квантование: ровнее качество на плоских и детальных участках.
            rc.set_enableAQ(1);
            // Пресеты «высокое качество» сами включают анализ наперёд — только если его попросили.
            rc.set_enableLookahead(u32::from(opts.lookahead));
            rc.lookaheadDepth = if opts.lookahead { LOOKAHEAD_FRAMES } else { 0 };
            let h264 = &mut cfg.encodeCodecConfig.h264Config;
            h264.idrPeriod = s.gop();
            h264.set_repeatSPSPPS(1);

            let mut init: NV_ENC_INITIALIZE_PARAMS = std::mem::zeroed();
            init.version = NV_ENC_INITIALIZE_PARAMS_VER;
            init.encodeGUID = NV_ENC_CODEC_H264_GUID;
            init.presetGUID = preset;
            init.encodeWidth = s.width;
            init.encodeHeight = s.height;
            init.darWidth = s.width;
            init.darHeight = s.height;
            init.frameRateNum = s.fps_num.max(1);
            init.frameRateDen = s.fps_den.max(1);
            init.enablePTD = 1;
            init.encodeConfig = &mut cfg;
            init.maxEncodeWidth = s.width;
            init.maxEncodeHeight = s.height;
            init.tuningInfo = tuning;
            nv!(enc.api, nvEncInitializeEncoder(enc.encoder, &mut init))
                .map_err(|r| enc.fail("инициализация NVENC", r))?;

            for _ in 0..pipeline_depth(opts) + MAX_QUEUED {
                let mut b: NV_ENC_CREATE_BITSTREAM_BUFFER = std::mem::zeroed();
                b.version = NV_ENC_CREATE_BITSTREAM_BUFFER_VER;
                nv!(enc.api, nvEncCreateBitstreamBuffer(enc.encoder, &mut b))
                    .map_err(|r| enc.fail("выходной буфер NVENC", r))?;
                enc.bitstreams.push(b.bitstreamBuffer);
            }
        }
        enc.name = format!("NVIDIA NVENC H.264 (P{})", opts.preset.clamp(1, 7));
        tracing::info!(
            "видеоэнкодер: {}, {}×{} {} кбит/с{}{}",
            enc.name,
            s.width,
            s.height,
            s.bitrate_kbps,
            if opts.low_latency { ", низкая задержка" } else { "" },
            if opts.lookahead { ", lookahead" } else { "" }
        );
        Ok(enc)
    }

    /// Ставит кадр в очередь на кодирование (`pts` — 100 нс от начала записи).
    pub fn push(&mut self, texture: ID3D11Texture2D, pts: i64) {
        if self.queued.len() >= MAX_QUEUED {
            self.queued.pop_front();
            self.dropped += 1;
        }
        self.queued.push_back((texture, pts));
    }

    /// Кодирует кадры из очереди; готовые пакеты — в `out`.
    pub fn pump(&mut self, out: &mut Vec<Packet>) -> Result<bool, EncodeError> {
        while let Some((tex, pts)) = self.queued.pop_front() {
            self.encode(Some((tex, pts)), out)?;
        }
        Ok(false)
    }

    /// Дописывает очередь и забирает все оставшиеся пакеты (конец записи).
    pub fn drain(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        self.pump(out)?;
        self.encode(None, out)
    }

    fn registration(&mut self, tex: &ID3D11Texture2D) -> Result<NV_ENC_REGISTERED_PTR, EncodeError> {
        let key = tex.as_raw() as usize;
        if let Some((_, r)) = self.registered.iter().find(|(k, _)| *k == key) {
            return Ok(*r);
        }
        // SAFETY: регистрация NV12-текстуры нашего устройства; снимается в Drop.
        unsafe {
            let mut reg: NV_ENC_REGISTER_RESOURCE = std::mem::zeroed();
            reg.version = NV_ENC_REGISTER_RESOURCE_VER;
            reg.resourceType = _NV_ENC_INPUT_RESOURCE_TYPE_NV_ENC_INPUT_RESOURCE_TYPE_DIRECTX;
            reg.width = self.width;
            reg.height = self.height;
            reg.resourceToRegister = tex.as_raw();
            reg.bufferFormat = _NV_ENC_BUFFER_FORMAT_NV_ENC_BUFFER_FORMAT_NV12;
            reg.bufferUsage = _NV_ENC_BUFFER_USAGE_NV_ENC_INPUT_IMAGE;
            nv!(self.api, nvEncRegisterResource(self.encoder, &mut reg))
                .map_err(|r| self.fail("регистрация текстуры", r))?;
            self.registered.push((key, reg.registeredResource));
            Ok(reg.registeredResource)
        }
    }

    /// Подаёт кадр (`None` — конец потока) и забирает готовые пакеты.
    fn encode(&mut self, frame: Option<(ID3D11Texture2D, i64)>, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        let flush = frame.is_none();
        // SAFETY: структуры по nvEncodeAPI.h; отображённый вход и выходной буфер живут до
        // получения пакета (InFlight держит текстуру).
        let status = unsafe {
            let mut pic: NV_ENC_PIC_PARAMS = std::mem::zeroed();
            pic.version = NV_ENC_PIC_PARAMS_VER;
            match frame {
                Some((tex, pts)) => {
                    let reg = self.registration(&tex)?;
                    let mut map: NV_ENC_MAP_INPUT_RESOURCE = std::mem::zeroed();
                    map.version = NV_ENC_MAP_INPUT_RESOURCE_VER;
                    map.registeredResource = reg;
                    nv!(self.api, nvEncMapInputResource(self.encoder, &mut map))
                        .map_err(|r| self.fail("отображение кадра", r))?;
                    let slot = self.next_slot;
                    self.next_slot = (self.next_slot + 1) % self.bitstreams.len();
                    pic.inputWidth = self.width;
                    pic.inputHeight = self.height;
                    pic.inputPitch = self.width;
                    pic.inputBuffer = map.mappedResource;
                    pic.outputBitstream = self.bitstreams[slot];
                    pic.bufferFmt = map.mappedBufferFmt;
                    pic.pictureStruct = _NV_ENC_PIC_STRUCT_NV_ENC_PIC_STRUCT_FRAME;
                    pic.inputTimeStamp = pts.max(0) as u64;
                    pic.frameIdx = self.frame_idx;
                    self.frame_idx = self.frame_idx.wrapping_add(1);
                    self.in_flight.push_back(InFlight { slot, mapped: map.mappedResource, _texture: tex });
                }
                None => pic.encodePicFlags = _NV_ENC_PIC_FLAGS_NV_ENC_PIC_FLAG_EOS as u32,
            }
            nv!(self.api, nvEncEncodePicture(self.encoder, &mut pic))
        };
        match status {
            Ok(()) => self.collect(out, flush),
            Err(_NVENCSTATUS_NV_ENC_ERR_NEED_MORE_INPUT) => Ok(()),
            Err(r) => Err(self.fail("кодирование кадра", r)),
        }
    }

    /// Забирает пакеты отданных кадров по порядку подачи: все при `flush`, иначе — кроме
    /// последних `lookahead`, которые ещё в анализе.
    fn collect(&mut self, out: &mut Vec<Packet>, flush: bool) -> Result<(), EncodeError> {
        let keep = if flush { 0 } else { self.lookahead };
        while self.in_flight.len() > keep {
            let Some(f) = self.in_flight.pop_front() else { break };
            // SAFETY: буфер `slot` получил данные этого кадра; блокировка снимается сразу после копии.
            unsafe {
                let mut lock: NV_ENC_LOCK_BITSTREAM = std::mem::zeroed();
                lock.version = NV_ENC_LOCK_BITSTREAM_VER;
                lock.outputBitstream = self.bitstreams[f.slot];
                nv!(self.api, nvEncLockBitstream(self.encoder, &mut lock))
                    .map_err(|r| self.fail("чтение пакета", r))?;
                let data = std::slice::from_raw_parts(
                    lock.bitstreamBufferPtr.cast::<u8>(),
                    lock.bitstreamSizeInBytes as usize,
                )
                .to_vec();
                let pts = lock.outputTimeStamp as i64;
                let keyframe = lock.pictureType == _NV_ENC_PIC_TYPE_NV_ENC_PIC_TYPE_IDR
                    || lock.pictureType == _NV_ENC_PIC_TYPE_NV_ENC_PIC_TYPE_I;
                let _ = nv!(self.api, nvEncUnlockBitstream(self.encoder, lock.outputBitstream));
                let _ = nv!(self.api, nvEncUnmapInputResource(self.encoder, f.mapped));
                out.push(Packet { data, pts, dts: pts, keyframe });
            }
        }
        Ok(())
    }
}

impl Drop for NvencEncoder {
    fn drop(&mut self) {
        if self.encoder.is_null() {
            return;
        }
        // SAFETY: освобождение ресурсов сессии в обратном порядке; после — сессия уничтожается.
        unsafe {
            for f in self.in_flight.drain(..) {
                let _ = nv!(self.api, nvEncUnmapInputResource(self.encoder, f.mapped));
            }
            for (_, r) in self.registered.drain(..) {
                let _ = nv!(self.api, nvEncUnregisterResource(self.encoder, r));
            }
            for b in self.bitstreams.drain(..) {
                let _ = nv!(self.api, nvEncDestroyBitstreamBuffer(self.encoder, b));
            }
            let _ = nv!(self.api, nvEncDestroyEncoder(self.encoder));
        }
    }
}
