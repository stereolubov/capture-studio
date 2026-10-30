//! Intel Quick Sync Video через oneVPL (§9.1): H.264 на GPU Intel, вход — D3D11-текстура.
//!
//! Диспетчер `libvpl.dll` (MIT, ставит драйвер Intel) загружается динамически и сам находит среду
//! выполнения своего GPU (`libmfx64-gen.dll` для Gen12+/Arc, `libmfxhw64.dll` для старых).
//! Сессия создаётся на адаптере нашего D3D11-устройства (сверка LUID) и получает это устройство.
//!
//! Энкодеру нужны размеры, кратные 16, поэтому кадр копируется на GPU в собственный пул
//! выровненных текстур (`CopySubresourceRegion` — видеопамять → видеопамять, к CPU кадр не
//! попадает); пул описан внешним распределителем (`mfxFrameAllocator`): MemId → текстура.
//! Пакеты забираются по порядку подачи через `SyncOperation`, B-кадры не опорные, время
//! декодирования — как у NVENC: время подачи со сдвигом в кадр.

pub mod sys;

use crate::{EncodeError, EncoderOptions, H264Profile, MAX_B_FRAMES, Packet, RateControl, VideoSettings};
use std::collections::VecDeque;
use std::ffi::c_void;
use sys::*;
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_DECODER, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{Interface, PCSTR, w};

/// Предел очереди кадров на подачу: дольше — кадры отбрасываются, счётчик растёт.
const MAX_QUEUED: usize = 8;
/// Ожидание готовности пакета, мс.
const SYNC_WAIT_MS: u32 = 1000;

fn status_text(s: mfxStatus) -> String {
    let name = match s {
        MFX_ERR_UNKNOWN => "ERR_UNKNOWN",
        MFX_ERR_NULL_PTR => "ERR_NULL_PTR",
        MFX_ERR_UNSUPPORTED => "ERR_UNSUPPORTED",
        MFX_ERR_MEMORY_ALLOC => "ERR_MEMORY_ALLOC",
        MFX_ERR_NOT_ENOUGH_BUFFER => "ERR_NOT_ENOUGH_BUFFER",
        MFX_ERR_INVALID_HANDLE => "ERR_INVALID_HANDLE",
        MFX_ERR_LOCK_MEMORY => "ERR_LOCK_MEMORY",
        MFX_ERR_NOT_INITIALIZED => "ERR_NOT_INITIALIZED",
        MFX_ERR_NOT_FOUND => "ERR_NOT_FOUND",
        MFX_ERR_MORE_DATA => "ERR_MORE_DATA",
        MFX_ERR_MORE_SURFACE => "ERR_MORE_SURFACE",
        MFX_ERR_ABORTED => "ERR_ABORTED",
        MFX_ERR_DEVICE_LOST => "ERR_DEVICE_LOST",
        MFX_ERR_INCOMPATIBLE_VIDEO_PARAM => "ERR_INCOMPATIBLE_VIDEO_PARAM",
        MFX_ERR_INVALID_VIDEO_PARAM => "ERR_INVALID_VIDEO_PARAM",
        MFX_ERR_UNDEFINED_BEHAVIOR => "ERR_UNDEFINED_BEHAVIOR",
        MFX_ERR_DEVICE_FAILED => "ERR_DEVICE_FAILED",
        MFX_ERR_GPU_HANG => "ERR_GPU_HANG",
        MFX_WRN_IN_EXECUTION => "WRN_IN_EXECUTION",
        MFX_WRN_DEVICE_BUSY => "WRN_DEVICE_BUSY",
        MFX_WRN_INCOMPATIBLE_VIDEO_PARAM => "WRN_INCOMPATIBLE_VIDEO_PARAM",
        _ => return format!("mfxStatus {s}"),
    };
    format!("MFX_{name}")
}

/// Функции диспетчера.
struct Api {
    lib: HMODULE,
    load: MFXLoadFn,
    unload: MFXUnloadFn,
    create_config: MFXCreateConfigFn,
    set_filter: MFXSetConfigFilterPropertyFn,
    enum_impl: MFXEnumImplementationsFn,
    release_desc: MFXDispReleaseImplDescriptionFn,
    create_session: MFXCreateSessionFn,
    close: MFXCloseFn,
    set_handle: MFXVideoCORE_SetHandleFn,
    set_allocator: MFXVideoCORE_SetFrameAllocatorFn,
    sync: MFXVideoCORE_SyncOperationFn,
    query_io_surf: MFXVideoENCODE_QueryIOSurfFn,
    init: MFXVideoENCODE_InitFn,
    encode_close: MFXVideoENCODE_CloseFn,
    get_video_param: MFXVideoENCODE_GetVideoParamFn,
    encode: MFXVideoENCODE_EncodeFrameAsyncFn,
}

impl Drop for Api {
    fn drop(&mut self) {
        // SAFETY: библиотека загружена в `load` и больше не используется (сессия закрыта).
        unsafe {
            let _ = FreeLibrary(self.lib);
        }
    }
}

fn load() -> Result<Api, EncodeError> {
    // SAFETY: загрузка диспетчера; указатели функций приводятся к сигнатурам из заголовков oneVPL.
    unsafe {
        let lib = LoadLibraryW(w!("libvpl.dll"))
            .map_err(|_| EncodeError::Unavailable("нет драйвера Intel с oneVPL (libvpl.dll)".into()))?;
        macro_rules! sym {
            ($ty:ty, $name:literal) => {
                match GetProcAddress(lib, PCSTR(concat!($name, "\0").as_ptr())) {
                    Some(f) => std::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(f),
                    None => {
                        let _ = FreeLibrary(lib);
                        return Err(EncodeError::Unavailable(concat!("libvpl.dll без ", $name).into()));
                    }
                }
            };
        }
        Ok(Api {
            load: sym!(MFXLoadFn, "MFXLoad"),
            unload: sym!(MFXUnloadFn, "MFXUnload"),
            create_config: sym!(MFXCreateConfigFn, "MFXCreateConfig"),
            set_filter: sym!(MFXSetConfigFilterPropertyFn, "MFXSetConfigFilterProperty"),
            enum_impl: sym!(MFXEnumImplementationsFn, "MFXEnumImplementations"),
            release_desc: sym!(MFXDispReleaseImplDescriptionFn, "MFXDispReleaseImplDescription"),
            create_session: sym!(MFXCreateSessionFn, "MFXCreateSession"),
            close: sym!(MFXCloseFn, "MFXClose"),
            set_handle: sym!(MFXVideoCORE_SetHandleFn, "MFXVideoCORE_SetHandle"),
            set_allocator: sym!(MFXVideoCORE_SetFrameAllocatorFn, "MFXVideoCORE_SetFrameAllocator"),
            sync: sym!(MFXVideoCORE_SyncOperationFn, "MFXVideoCORE_SyncOperation"),
            query_io_surf: sym!(MFXVideoENCODE_QueryIOSurfFn, "MFXVideoENCODE_QueryIOSurf"),
            init: sym!(MFXVideoENCODE_InitFn, "MFXVideoENCODE_Init"),
            encode_close: sym!(MFXVideoENCODE_CloseFn, "MFXVideoENCODE_Close"),
            get_video_param: sym!(MFXVideoENCODE_GetVideoParamFn, "MFXVideoENCODE_GetVideoParam"),
            encode: sym!(MFXVideoENCODE_EncodeFrameAsyncFn, "MFXVideoENCODE_EncodeFrameAsync"),
            lib,
        })
    }
}

/// Пул текстур энкодера для внешнего распределителя: MemId = индекс + 1.
struct Pool {
    textures: Vec<ID3D11Texture2D>,
}

// Распределитель: кадры энкодера приложение создаёт само, внутренние кадры (опорные и т. п.)
// библиотека заводит своим распределителем, получив MFX_ERR_UNSUPPORTED.
unsafe extern "C" fn alloc_cb(_: mfxHDL, _: *mut mfxFrameAllocRequest, _: *mut mfxFrameAllocResponse) -> mfxStatus {
    MFX_ERR_UNSUPPORTED
}

unsafe extern "C" fn lock_cb(_: mfxHDL, _: mfxMemId, _: *mut mfxFrameData) -> mfxStatus {
    MFX_ERR_UNSUPPORTED
}

unsafe extern "C" fn unlock_cb(_: mfxHDL, _: mfxMemId, _: *mut mfxFrameData) -> mfxStatus {
    MFX_ERR_NONE
}

/// MemId → пара (текстура, индекс подресурса 0) — так библиотека получает D3D11-кадр.
unsafe extern "C" fn get_hdl_cb(pthis: mfxHDL, mid: mfxMemId, handle: *mut mfxHDL) -> mfxStatus {
    // SAFETY: `pthis` — наш `Pool`, живущий дольше сессии; `handle` для D3D11 указывает на mfxHDLPair.
    unsafe {
        let pool = &*pthis.cast::<Pool>();
        let Some(tex) = (mid as usize).checked_sub(1).and_then(|i| pool.textures.get(i)) else {
            return MFX_ERR_INVALID_HANDLE;
        };
        let pair = handle.cast::<mfxHDLPair>();
        (*pair).first = tex.as_raw();
        (*pair).second = std::ptr::null_mut();
    }
    MFX_ERR_NONE
}

unsafe extern "C" fn free_cb(_: mfxHDL, _: *mut mfxFrameAllocResponse) -> mfxStatus {
    MFX_ERR_NONE
}

/// Выходной буфер с отданной на кодирование задачей.
struct Slot {
    data: Vec<u8>,
    bs: mfxBitstream,
    sync: mfxSyncPoint,
}

pub struct QsvEncoder {
    api: Api,
    loader: mfxLoader,
    session: mfxSession,
    context: ID3D11DeviceContext,
    /// Распределитель и пул держатся в куче: библиотека хранит указатели на них.
    _allocator: Box<mfxFrameAllocator>,
    pool: Box<Pool>,
    /// Не растёт после создания пула: библиотека держит указатели на поверхности.
    surfaces: Vec<mfxFrameSurface1>,
    slots: Vec<Slot>,
    /// Слоты с задачами по порядку подачи.
    in_flight: VecDeque<usize>,
    queued: VecDeque<(ID3D11Texture2D, i64)>,
    /// Времена поданных кадров по порядку подачи: из них — время декодирования пакетов.
    input_pts: VecDeque<i64>,
    dts_shift: i64,
    pub name: String,
    pub dropped: u64,
}

// SAFETY: сессия используется из одного потока (поток энкодера записи); oneVPL не привязывает
// её к потоку-создателю.
unsafe impl Send for QsvEncoder {}

const fn align16(v: u32) -> u16 {
    (v.div_ceil(16) * 16) as u16
}

impl QsvEncoder {
    fn fail(what: &str, s: mfxStatus) -> EncodeError {
        EncodeError::Failed(format!("{what}: {}", status_text(s)))
    }

    pub fn new(device: &ID3D11Device, s: &VideoSettings, opts: &EncoderOptions) -> Result<Self, EncodeError> {
        let vendor = crate::adapter_vendor(device);
        if vendor != 0x8086 {
            return Err(EncodeError::Unavailable(format!("QSV работает на GPU Intel, а сцена — на GPU {vendor:04x}")));
        }
        let api = load()?;
        // SAFETY: последовательность из документации oneVPL: диспетчер → сессия на нашем адаптере →
        // устройство и распределитель → параметры → Init. Структуры обнуляются и заполняются по
        // заголовкам; указатели на них живут до возврата из вызовов.
        unsafe {
            let loader = (api.load)();
            if loader.is_null() {
                return Err(EncodeError::Unavailable("MFXLoad".into()));
            }
            let mut enc = Self {
                session: std::ptr::null_mut(),
                loader,
                context: device.GetImmediateContext()?,
                _allocator: Box::new(std::mem::zeroed()),
                pool: Box::new(Pool { textures: Vec::new() }),
                surfaces: Vec::new(),
                slots: Vec::new(),
                in_flight: VecDeque::new(),
                queued: VecDeque::new(),
                input_pts: VecDeque::new(),
                dts_shift: 0,
                name: String::new(),
                dropped: 0,
                api,
            };
            enc.create_session(device)?;
            let r = (enc.api.set_handle)(enc.session, MFX_HANDLE_D3D11_DEVICE, device.as_raw());
            if r != MFX_ERR_NONE {
                return Err(EncodeError::Unavailable(format!("oneVPL и D3D11-устройство: {}", status_text(r))));
            }
            let a = &mut *enc._allocator;
            a.pthis = (&raw const *enc.pool).cast_mut().cast::<c_void>();
            a.Alloc = Some(alloc_cb);
            a.Lock = Some(lock_cb);
            a.Unlock = Some(unlock_cb);
            a.GetHDL = Some(get_hdl_cb);
            a.Free = Some(free_cb);
            let r = (enc.api.set_allocator)(enc.session, a);
            if r != MFX_ERR_NONE {
                return Err(Self::fail("распределитель oneVPL", r));
            }

            let mut co2: mfxExtCodingOption2 = std::mem::zeroed();
            co2.Header.BufferId = MFX_EXTBUFF_CODING_OPTION2 as u32;
            co2.Header.BufferSz = std::mem::size_of::<mfxExtCodingOption2>() as u32;
            co2.BRefType = MFX_B_REF_OFF as u16;
            let mut ext = [(&raw mut co2).cast::<mfxExtBuffer>()];
            let mut par = video_param(s, opts);
            let b_frames = opts.b_frames.min(MAX_B_FRAMES);
            if b_frames > 0 {
                par.ExtParam = ext.as_mut_ptr();
                par.NumExtParam = 1;
            }

            let mut request: mfxFrameAllocRequest = std::mem::zeroed();
            let r = (enc.api.query_io_surf)(enc.session, &mut par, &mut request);
            if r < MFX_ERR_NONE {
                return Err(EncodeError::Unavailable(format!("параметры QSV: {}", status_text(r))));
            }
            let r = (enc.api.init)(enc.session, &mut par);
            if r < MFX_ERR_NONE {
                return Err(match r {
                    MFX_ERR_DEVICE_FAILED | MFX_ERR_MEMORY_ALLOC => EncodeError::SessionLimit,
                    _ => EncodeError::Unavailable(format!("инициализация QSV: {}", status_text(r))),
                });
            }
            if r > MFX_ERR_NONE {
                tracing::info!("QSV скорректировал параметры: {}", status_text(r));
            }

            // Принятые параметры: размер выходного буфера и число B-кадров.
            let mut got: mfxVideoParam = std::mem::zeroed();
            let r = (enc.api.get_video_param)(enc.session, &mut got);
            if r != MFX_ERR_NONE {
                return Err(Self::fail("параметры QSV", r));
            }
            let m = &got.__bindgen_anon_1.mfx;
            let rc = &m.__bindgen_anon_1.__bindgen_anon_1;
            let buffer = usize::from(rc.BufferSizeInKB) * usize::from(m.BRCParamMultiplier.max(1)) * 1000;
            let buffer = buffer.max(
                usize::from(m.FrameInfo.__bindgen_anon_1.__bindgen_anon_1.Width)
                    * usize::from(m.FrameInfo.__bindgen_anon_1.__bindgen_anon_1.Height)
                    * 3
                    / 2,
            );
            let accepted_b = rc.GopRefDist.saturating_sub(1);
            enc.dts_shift = if accepted_b > 0 { s.frame_duration() } else { 0 };

            enc.create_pool(device, &par, usize::from(request.NumFrameSuggested.max(request.NumFrameMin)) + 2)?;
            let depth = usize::from(par.AsyncDepth.max(1)) + 1;
            for _ in 0..depth {
                enc.slots.push(Slot { data: vec![0; buffer], bs: std::mem::zeroed(), sync: std::ptr::null_mut() });
            }
            enc.name = format!(
                "Intel QSV H.264 (TU{})",
                par.__bindgen_anon_1.mfx.__bindgen_anon_1.__bindgen_anon_1.TargetUsage
            );
            tracing::info!(
                "видеоэнкодер: {}, {}×{} {} кбит/с, B-кадров {accepted_b}, пул {} текстур",
                enc.name,
                s.width,
                s.height,
                s.bitrate_kbps,
                enc.pool.textures.len()
            );
            Ok(enc)
        }
    }

    /// Сессия на аппаратной реализации D3D11 нашего адаптера (по LUID; без сведений об адаптерах
    /// у старых сред выполнения — первая подходящая).
    unsafe fn create_session(&mut self, device: &ID3D11Device) -> Result<(), EncodeError> {
        // SAFETY: вызовы диспетчера с живым загрузчиком; описание реализации освобождается сразу.
        unsafe {
            let filter = |name: &[u8], value: u32| -> Result<(), EncodeError> {
                let cfg = (self.api.create_config)(self.loader);
                let mut v: mfxVariant = std::mem::zeroed();
                v.Version = struct_version(1, 0);
                v.Type = MFX_VARIANT_TYPE_U32;
                v.Data.U32 = value;
                match (self.api.set_filter)(cfg, name.as_ptr(), v) {
                    MFX_ERR_NONE => Ok(()),
                    r => Err(EncodeError::Unavailable(format!("фильтр oneVPL: {}", status_text(r)))),
                }
            };
            filter(b"mfxImplDescription.Impl\0", MFX_IMPL_TYPE_HARDWARE as u32)?;
            filter(b"mfxImplDescription.AccelerationMode\0", MFX_ACCEL_MODE_VIA_D3D11 as u32)?;

            let luid = crate::adapter_luid(device);
            let mut chosen = None;
            for i in 0..16u32 {
                let mut desc: mfxHDL = std::ptr::null_mut();
                let r = (self.api.enum_impl)(self.loader, i, MFX_IMPLCAPS_DEVICE_ID_EXTENDED, &mut desc);
                if r == MFX_ERR_NOT_FOUND {
                    break;
                }
                if r != MFX_ERR_NONE || desc.is_null() {
                    // Среда выполнения не сообщает адаптер — берём первую реализацию.
                    chosen.get_or_insert(i);
                    continue;
                }
                let id = &*desc.cast::<mfxExtendedDeviceId>();
                let matches = id.LUIDValid != 0 && Some(id.DeviceLUID) == luid;
                let _ = (self.api.release_desc)(self.loader, desc);
                if matches {
                    chosen = Some(i);
                    break;
                }
            }
            let Some(index) = chosen else {
                return Err(EncodeError::Unavailable("нет реализации oneVPL на адаптере сцены".into()));
            };
            match (self.api.create_session)(self.loader, index, &mut self.session) {
                MFX_ERR_NONE => Ok(()),
                r => Err(EncodeError::Unavailable(format!("сессия oneVPL: {}", status_text(r)))),
            }
        }
    }

    /// Пул выровненных NV12-текстур и поверхности oneVPL для них.
    fn create_pool(&mut self, device: &ID3D11Device, par: &mfxVideoParam, count: usize) -> Result<(), EncodeError> {
        // SAFETY: создание текстур нашего устройства; поверхности ссылаются на них через MemId.
        unsafe {
            let info = par.__bindgen_anon_1.mfx.FrameInfo;
            let size = info.__bindgen_anon_1.__bindgen_anon_1;
            let desc = D3D11_TEXTURE2D_DESC {
                Width: u32::from(size.Width),
                Height: u32::from(size.Height),
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_NV12,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_DECODER.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            for i in 0..count {
                let mut tex = None;
                device.CreateTexture2D(&desc, None, Some(&mut tex))?;
                self.pool.textures.push(tex.ok_or_else(|| EncodeError::Failed("текстура пула QSV".into()))?);
                let mut surf: mfxFrameSurface1 = std::mem::zeroed();
                surf.Info = info;
                surf.Data.MemId = (i + 1) as mfxMemId;
                self.surfaces.push(surf);
            }
        }
        Ok(())
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
        self.collect(out, false)?;
        while let Some((tex, pts)) = self.queued.pop_front() {
            let Some(idx) = self.free_surface(out)? else {
                // Все поверхности у энкодера — кадр подождёт.
                self.queued.push_front((tex, pts));
                break;
            };
            // SAFETY: копия NV12 нашего кадра в левый верхний угол текстуры пула (тот же
            // контекст, что у рендера и энкодера, — порядок команд сохраняется).
            unsafe {
                self.context.CopySubresourceRegion(&self.pool.textures[idx], 0, 0, 0, 0, &tex, 0, None);
            }
            self.surfaces[idx].Data.TimeStamp = pts.max(0) as u64;
            self.input_pts.push_back(pts);
            self.encode(Some(idx), out)?;
        }
        self.collect(out, false)?;
        Ok(false)
    }

    /// Поверхность, которую энкодер не держит (`Locked == 0`); если таких нет — дождаться пакета.
    fn free_surface(&mut self, out: &mut Vec<Packet>) -> Result<Option<usize>, EncodeError> {
        for _ in 0..2 {
            // SAFETY: счётчик меняет библиотека из своих потоков — читается без кэширования.
            let free = self.surfaces.iter().position(|s| unsafe { std::ptr::read_volatile(&s.Data.Locked) } == 0);
            if free.is_some() {
                return Ok(free);
            }
            if self.in_flight.is_empty() {
                break;
            }
            self.collect_one(out, true)?;
        }
        Ok(None)
    }

    /// Подаёт поверхность (`None` — конец потока). `false` — энкодер выдал всё (при `None`).
    fn encode(&mut self, surface: Option<usize>, out: &mut Vec<Packet>) -> Result<bool, EncodeError> {
        loop {
            if self.in_flight.len() >= self.slots.len() {
                self.collect_one(out, true)?;
            }
            let slot = (0..self.slots.len())
                .find(|i| !self.in_flight.contains(i))
                .ok_or_else(|| EncodeError::Failed("нет свободного буфера QSV".into()))?;
            let surf = surface.map_or(std::ptr::null_mut(), |i| &raw mut self.surfaces[i]);
            let sl = &mut self.slots[slot];
            sl.bs.Data = sl.data.as_mut_ptr();
            sl.bs.MaxLength = sl.data.len() as u32;
            sl.bs.DataOffset = 0;
            sl.bs.DataLength = 0;
            let mut sync: mfxSyncPoint = std::ptr::null_mut();
            // SAFETY: поверхность и буфер живут в `self` до получения пакета.
            let r = unsafe { (self.api.encode)(self.session, std::ptr::null_mut(), surf, &mut sl.bs, &mut sync) };
            match r {
                MFX_WRN_DEVICE_BUSY => {
                    if self.in_flight.is_empty() {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    } else {
                        self.collect_one(out, true)?;
                    }
                }
                MFX_ERR_MORE_DATA => return Ok(surface.is_some()),
                r if r >= MFX_ERR_NONE && !sync.is_null() => {
                    self.slots[slot].sync = sync;
                    self.in_flight.push_back(slot);
                    return Ok(true);
                }
                r if r >= MFX_ERR_NONE => return Ok(true),
                r => return Err(Self::fail("кодирование кадра QSV", r)),
            }
        }
    }

    /// Забирает готовые пакеты: без ожидания или (при `wait`) все.
    fn collect(&mut self, out: &mut Vec<Packet>, wait: bool) -> Result<(), EncodeError> {
        while !self.in_flight.is_empty() {
            if !self.collect_one(out, wait)? {
                break;
            }
        }
        Ok(())
    }

    /// Пакет самой старой задачи; `false` — ещё не готов (без ожидания).
    fn collect_one(&mut self, out: &mut Vec<Packet>, wait: bool) -> Result<bool, EncodeError> {
        let Some(&slot) = self.in_flight.front() else { return Ok(false) };
        // SAFETY: точка синхронизации выдана этой сессией и ещё не использована.
        let r = unsafe { (self.api.sync)(self.session, self.slots[slot].sync, if wait { SYNC_WAIT_MS } else { 0 }) };
        match r {
            MFX_WRN_IN_EXECUTION if !wait => return Ok(false),
            MFX_ERR_NONE => {}
            r => return Err(Self::fail("ожидание пакета QSV", r)),
        }
        self.in_flight.pop_front();
        let sl = &mut self.slots[slot];
        let (off, len) = (sl.bs.DataOffset as usize, sl.bs.DataLength as usize);
        let data = sl.data[off..off + len].to_vec();
        let pts = sl.bs.TimeStamp as i64;
        let keyframe = u32::from(sl.bs.FrameType) & (MFX_FRAMETYPE_IDR | MFX_FRAMETYPE_I) as u32 != 0;
        // Пакеты идут в порядке декодирования: k-му пакету — время k-го поданного кадра.
        let dts = self.input_pts.pop_front().map_or(pts, |t| t - self.dts_shift);
        out.push(Packet { data, pts, dts, keyframe });
        Ok(true)
    }

    /// Дописывает очередь и забирает все оставшиеся пакеты (конец записи).
    pub fn drain(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !self.queued.is_empty() && std::time::Instant::now() < deadline {
            self.pump(out)?;
        }
        while std::time::Instant::now() < deadline && self.encode(None, out)? {}
        self.collect(out, true)
    }
}

impl Drop for QsvEncoder {
    fn drop(&mut self) {
        // SAFETY: закрытие энкодера и сессии до освобождения пула и распределителя (поля
        // уничтожаются после этого тела).
        unsafe {
            if !self.session.is_null() {
                let _ = (self.api.encode_close)(self.session);
                let _ = (self.api.close)(self.session);
            }
            (self.api.unload)(self.loader);
        }
    }
}

/// Параметры кодирования H.264 для oneVPL.
fn video_param(s: &VideoSettings, opts: &EncoderOptions) -> mfxVideoParam {
    // SAFETY: обнулённая структура — допустимое начальное состояние по документации.
    let mut par: mfxVideoParam = unsafe { std::mem::zeroed() };
    par.AsyncDepth = if opts.low_latency { 1 } else { 4 };
    par.IOPattern = MFX_IOPATTERN_IN_VIDEO_MEMORY as u16;
    // SAFETY: объединения заполняются вариантом для кодирования (`mfx`, кадр — Width/Height).
    unsafe {
        let m = &mut par.__bindgen_anon_1.mfx;
        m.CodecId = MFX_CODEC_AVC as u32;
        m.CodecProfile = match opts.profile {
            H264Profile::Baseline => MFX_PROFILE_AVC_BASELINE,
            H264Profile::Main => MFX_PROFILE_AVC_MAIN,
            H264Profile::High => MFX_PROFILE_AVC_HIGH,
        } as u16;
        let fi = &mut m.FrameInfo;
        fi.FourCC = MFX_FOURCC_NV12 as u32;
        fi.ChromaFormat = MFX_CHROMAFORMAT_YUV420 as u16;
        fi.PicStruct = MFX_PICSTRUCT_PROGRESSIVE as u16;
        fi.FrameRateExtN = s.fps_num.max(1);
        fi.FrameRateExtD = s.fps_den.max(1);
        fi.AspectRatioW = 1;
        fi.AspectRatioH = 1;
        let size = &mut fi.__bindgen_anon_1.__bindgen_anon_1;
        size.Width = align16(s.width);
        size.Height = align16(s.height);
        size.CropW = s.width as u16;
        size.CropH = s.height as u16;

        let e = &mut m.__bindgen_anon_1.__bindgen_anon_1;
        // P1 (быстрее) … P7 (качественнее) → TargetUsage 7 … 1.
        e.TargetUsage = u16::from(8 - opts.preset.clamp(1, 7));
        e.GopPicSize = s.gop().min(u32::from(u16::MAX)) as u16;
        e.GopRefDist = u16::from(opts.b_frames.min(MAX_B_FRAMES)) + 1;
        e.GopOptFlag = MFX_GOP_CLOSED as u16;
        e.IdrInterval = 0; // каждый I-кадр — IDR (с SPS/PPS)
        let kbps = s.bitrate_kbps.max(100);
        // Битрейт в 16-битных полях: большие значения — через множитель.
        let mult = kbps.div_ceil(u32::from(u16::MAX)).max(1);
        match s.rate_control {
            RateControl::Cbr => {
                e.RateControlMethod = MFX_RATECONTROL_CBR as u16;
                m.BRCParamMultiplier = mult as u16;
                e.__bindgen_anon_2.TargetKbps = (kbps / mult) as u16;
                e.__bindgen_anon_3.MaxKbps = (kbps / mult) as u16;
            }
            RateControl::Vbr => {
                let peak = kbps * 3 / 2;
                let mult = peak.div_ceil(u32::from(u16::MAX)).max(1);
                e.RateControlMethod = MFX_RATECONTROL_VBR as u16;
                m.BRCParamMultiplier = mult as u16;
                e.__bindgen_anon_2.TargetKbps = (kbps / mult) as u16;
                e.__bindgen_anon_3.MaxKbps = (peak / mult) as u16;
            }
            RateControl::Cqp(q) => {
                let q = q.clamp(1, 51) as u16;
                e.RateControlMethod = MFX_RATECONTROL_CQP as u16;
                e.__bindgen_anon_1.QPI = q.saturating_sub(2).max(1);
                e.__bindgen_anon_2.QPP = q;
                e.__bindgen_anon_3.QPB = q;
            }
        }
    }
    par
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(rc: RateControl, kbps: u32) -> VideoSettings {
        VideoSettings {
            width: 1920,
            height: 1080,
            fps_num: 60,
            fps_den: 1,
            bitrate_kbps: kbps,
            keyframe_interval_s: 2,
            rate_control: rc,
        }
    }

    #[test]
    fn frame_is_aligned_and_cropped() {
        let par = video_param(&settings(RateControl::Cbr, 6000), &EncoderOptions::default());
        // SAFETY: чтение заполненного варианта объединения.
        let (size, e) = unsafe {
            let m = &par.__bindgen_anon_1.mfx;
            (m.FrameInfo.__bindgen_anon_1.__bindgen_anon_1, m.__bindgen_anon_1.__bindgen_anon_1)
        };
        assert_eq!((size.Width, size.Height, size.CropW, size.CropH), (1920, 1088, 1920, 1080));
        assert_eq!((e.GopPicSize, e.GopRefDist, e.TargetUsage), (120, 1, 3));
    }

    #[test]
    fn big_bitrate_uses_multiplier() {
        let par = video_param(&settings(RateControl::Cbr, 100_000), &EncoderOptions::default());
        // SAFETY: чтение заполненного варианта объединения.
        let (mult, kbps) = unsafe {
            let m = &par.__bindgen_anon_1.mfx;
            (m.BRCParamMultiplier, m.__bindgen_anon_1.__bindgen_anon_1.__bindgen_anon_2.TargetKbps)
        };
        assert_eq!(u32::from(mult) * u32::from(kbps), 100_000);
    }
}
