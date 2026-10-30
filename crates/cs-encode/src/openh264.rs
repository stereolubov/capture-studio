//! Программный H.264 — OpenH264 от Cisco (§9.1, последний резерв, ADR-0014).
//!
//! Двоичный модуль Cisco нельзя поставлять вместе с программой: по условиям Cisco (оплата
//! патентных отчислений MPEG LA) пользователь скачивает его сам с сервера Cisco. Поэтому
//! `openh264-2.6.0-win64.dll` загружается по кнопке в настройках (`download`) в папку кодеков
//! каталога данных и подключается динамически только после сверки SHA-256 с известным хешем
//! (`openh264-sys2`). Без модуля энкодер «недоступен» с причиной.
//!
//! Кадр NV12 читается из видеопамяти (единственный разрешённый путь GPU→CPU в записи, §4),
//! переводится в I420 и кодируется на CPU. B-кадров у OpenH264 нет: DTS = PTS.

use crate::{EncodeError, EncoderOptions, H264Profile, Packet, RateControl, VideoSettings};
use openh264_sys2::{API, DynamicAPI, ISVCEncoder};
use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_SAMPLE_DESC};

/// Версия модуля Cisco, чей хеш знает `openh264-sys2`.
pub const VERSION: &str = "2.6.0";
const DLL_NAME: &str = "openh264-2.6.0-win64.dll";
const URL_HOST: &str = "ciscobinary.openh264.org";
const URL_PATH: &str = "/openh264-2.6.0-win64.dll.bz2";
/// Уведомление, которое Cisco требует показывать при использовании модуля.
pub const NOTICE: &str = "OpenH264 Video Codec provided by Cisco Systems, Inc.";
/// Предел очереди кадров на подачу: дольше — кадры отбрасываются, счётчик растёт.
const MAX_QUEUED: usize = 4;

static DIR: OnceLock<PathBuf> = OnceLock::new();
static STATE: Mutex<Download> = Mutex::new(Download::Idle);

/// Папка кодеков (каталог данных `codecs`); задаётся при запуске программы.
pub fn set_dir(dir: PathBuf) {
    let _ = DIR.set(dir);
}

fn dll_path() -> Option<PathBuf> {
    DIR.get().map(|d| d.join(DLL_NAME))
}

/// Состояние загрузки модуля Cisco.
#[derive(Debug, Clone, PartialEq)]
pub enum Download {
    Idle,
    /// Доля 0..1 (0 — размер ещё неизвестен).
    Running(f32),
    Failed(String),
}

/// Установлен ли модуль (файл на месте; хеш сверяется при открытии энкодера).
pub fn installed() -> bool {
    dll_path().is_some_and(|p| p.is_file())
}

pub fn download_state() -> Download {
    STATE.lock().map_or(Download::Idle, |s| s.clone())
}

fn set_state(d: Download) {
    if let Ok(mut s) = STATE.lock() {
        *s = d;
    }
}

/// Запускает загрузку модуля с сервера Cisco в фоновом потоке (повторный вызов во время
/// загрузки ничего не делает). Итог — `download_state()` / `installed()`.
pub fn start_download() {
    let Some(path) = dll_path() else {
        set_state(Download::Failed("папка кодеков не задана".into()));
        return;
    };
    {
        let Ok(mut s) = STATE.lock() else { return };
        if matches!(*s, Download::Running(_)) {
            return;
        }
        *s = Download::Running(0.0);
    }
    let spawned = std::thread::Builder::new().name("openh264-download".into()).spawn(move || match download(&path) {
        Ok(()) => {
            tracing::info!("OpenH264 {VERSION} установлен: {}", path.display());
            set_state(Download::Idle);
        }
        Err(e) => {
            tracing::warn!("загрузка OpenH264: {e}");
            set_state(Download::Failed(e));
        }
    });
    if spawned.is_err() {
        set_state(Download::Failed("не удалось запустить загрузку".into()));
    }
}

/// Скачивает `.bz2`, распаковывает во временный файл, сверяет хеш и переименовывает.
fn download(path: &Path) -> Result<(), String> {
    let packed = http_get(URL_HOST, URL_PATH, |done, total| {
        if total > 0 {
            set_state(Download::Running(done as f32 / total as f32));
        }
    })?;
    let mut dll = Vec::new();
    bzip2::read::BzDecoder::new(packed.as_slice()).read_to_end(&mut dll).map_err(|e| format!("распаковка: {e}"))?;
    let dir = path.parent().ok_or("нет папки кодеков")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, &dll).map_err(|e| format!("{}: {e}", tmp.display()))?;
    // Сверка SHA-256 со списком известных сборок Cisco (загрузка библиотеки — только после неё).
    if let Err(e) = DynamicAPI::from_blob_path(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("файл не совпал с известной сборкой Cisco ({e})"));
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// GET по HTTPS через WinHTTP (системные прокси и сертификаты Windows).
fn http_get(host: &str, path: &str, mut progress: impl FnMut(u64, u64)) -> Result<Vec<u8>, String> {
    use windows::Win32::Networking::WinHttp::*;
    use windows::core::{HSTRING, PCWSTR, w};

    /// Закрывает дескриптор WinHTTP при выходе из области.
    struct Handle(*mut std::ffi::c_void);
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: дескриптор открыт этой функцией и закрывается один раз.
                unsafe {
                    let _ = WinHttpCloseHandle(self.0);
                }
            }
        }
    }
    let check = |h: *mut std::ffi::c_void, what: &str| {
        if h.is_null() { Err(format!("{what}: {}", windows::core::Error::from_thread())) } else { Ok(Handle(h)) }
    };
    // SAFETY: последовательность WinHTTP из документации; буферы живут во время вызовов.
    unsafe {
        let session = check(
            WinHttpOpen(w!("CaptureStudio"), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, PCWSTR::null(), PCWSTR::null(), 0),
            "WinHttpOpen",
        )?;
        let _ = WinHttpSetTimeouts(session.0, 10_000, 10_000, 10_000, 30_000);
        let connect =
            check(WinHttpConnect(session.0, &HSTRING::from(host), INTERNET_DEFAULT_HTTPS_PORT, 0), "соединение")?;
        let request = check(
            WinHttpOpenRequest(
                connect.0,
                w!("GET"),
                &HSTRING::from(path),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            ),
            "запрос",
        )?;
        WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(|e| format!("отправка запроса: {e}"))?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|e| format!("ответ сервера: {e}"))?;
        let header = |level: u32| -> u32 {
            let mut v = 0u32;
            let mut len = 4u32;
            let ok = WinHttpQueryHeaders(
                request.0,
                level | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some((&raw mut v).cast()),
                &mut len,
                std::ptr::null_mut(),
            );
            if ok.is_ok() { v } else { 0 }
        };
        let status = header(WINHTTP_QUERY_STATUS_CODE);
        if status != 200 {
            return Err(format!("сервер Cisco ответил {status}"));
        }
        let total = u64::from(header(WINHTTP_QUERY_CONTENT_LENGTH));
        let mut body = Vec::with_capacity(total as usize);
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let mut read = 0u32;
            WinHttpReadData(request.0, buf.as_mut_ptr().cast(), buf.len() as u32, &mut read)
                .map_err(|e| format!("чтение: {e}"))?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&buf[..read as usize]);
            progress(body.len() as u64, total);
        }
        Ok(body)
    }
}

pub struct OpenH264Encoder {
    api: DynamicAPI,
    encoder: *mut ISVCEncoder,
    context: ID3D11DeviceContext,
    staging: ID3D11Texture2D,
    width: usize,
    height: usize,
    /// Кадр I420: Y, затем U и V.
    i420: Vec<u8>,
    queued: VecDeque<(ID3D11Texture2D, i64)>,
    pub name: String,
    pub dropped: u64,
}

// SAFETY: энкодер используется из одного потока (поток энкодера записи); OpenH264 не привязывает
// объект к потоку-создателю.
unsafe impl Send for OpenH264Encoder {}

impl OpenH264Encoder {
    pub fn new(device: &ID3D11Device, s: &VideoSettings, opts: &EncoderOptions) -> Result<Self, EncodeError> {
        let path = dll_path().filter(|p| p.is_file()).ok_or_else(|| {
            EncodeError::Unavailable("OpenH264 не установлен (скачать: Настройки → Вывод → OpenH264)".into())
        })?;
        let api = DynamicAPI::from_blob_path(&path)
            .map_err(|e| EncodeError::Unavailable(format!("OpenH264 не загружен ({e})")))?;
        let (w, h) = (s.width as usize, s.height as usize);
        // SAFETY: создание staging-текстуры нашего устройства и энкодера по codec_api.h;
        // структуры параметров заполняются после GetDefaultParams.
        unsafe {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: s.width,
                Height: s.height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_NV12,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut staging = None;
            device.CreateTexture2D(&desc, None, Some(&mut staging))?;
            let staging = staging.ok_or_else(|| EncodeError::Failed("staging-текстура".into()))?;

            let mut encoder: *mut ISVCEncoder = std::ptr::null_mut();
            if api.WelsCreateSVCEncoder(&mut encoder) != 0 || encoder.is_null() {
                return Err(EncodeError::Unavailable("WelsCreateSVCEncoder".into()));
            }
            let enc = Self {
                api,
                encoder,
                context: device.GetImmediateContext()?,
                staging,
                width: w,
                height: h,
                i420: vec![0; w * h * 3 / 2],
                queued: VecDeque::new(),
                name: format!("OpenH264 {VERSION} (CPU)"),
                dropped: 0,
            };
            let vt = &**enc.encoder;
            let mut quiet = openh264_sys2::WELS_LOG_ERROR;
            if let Some(set) = vt.SetOption {
                set(enc.encoder, openh264_sys2::ENCODER_OPTION_TRACE_LEVEL, (&raw mut quiet).cast());
            }
            let mut p: openh264_sys2::SEncParamExt = std::mem::zeroed();
            let get_defaults = vt.GetDefaultParams.ok_or_else(|| EncodeError::Failed("OpenH264 без функций".into()))?;
            get_defaults(enc.encoder, &mut p);
            fill_params(&mut p, s, opts);
            let init = vt.InitializeExt.ok_or_else(|| EncodeError::Failed("OpenH264 без функций".into()))?;
            let r = init(enc.encoder, &p);
            if r != 0 {
                return Err(EncodeError::Unavailable(format!("инициализация OpenH264: код {r}")));
            }
            let mut format = openh264_sys2::videoFormatI420;
            if let Some(set) = vt.SetOption {
                set(enc.encoder, openh264_sys2::ENCODER_OPTION_DATAFORMAT, (&raw mut format).cast());
            }
            tracing::info!(
                "видеоэнкодер: {}, {}×{} {} кбит/с, потоков {} — нагрузка на CPU",
                enc.name,
                s.width,
                s.height,
                s.bitrate_kbps,
                p.iMultipleThreadIdc
            );
            tracing::info!("{NOTICE}");
            Ok(enc)
        }
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
            self.read_frame(&tex)?;
            self.encode(pts, out)?;
        }
        Ok(false)
    }

    /// Задержки внутри OpenH264 нет: дописывает очередь.
    pub fn drain(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        self.pump(out).map(|_| ())
    }

    /// NV12 из видеопамяти → I420 в `self.i420`.
    fn read_frame(&mut self, tex: &ID3D11Texture2D) -> Result<(), EncodeError> {
        let (w, h) = (self.width, self.height);
        // SAFETY: копия в staging-текстуру того же размера и формата; чтение в пределах RowPitch
        // между Map/Unmap.
        unsafe {
            self.context.CopyResource(&self.staging, tex);
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
            let pitch = m.RowPitch as usize;
            let base = m.pData.cast::<u8>().cast_const();
            nv12_to_i420(std::slice::from_raw_parts(base, pitch * (h + h / 2)), pitch, w, h, &mut self.i420);
            self.context.Unmap(&self.staging, 0);
        }
        Ok(())
    }

    fn encode(&mut self, pts: i64, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        let (w, h) = (self.width, self.height);
        // SAFETY: картинка ссылается на `self.i420` до возврата EncodeFrame; NAL-блоки
        // копируются до следующего вызова (буфер принадлежит энкодеру).
        unsafe {
            let mut pic: openh264_sys2::SSourcePicture = std::mem::zeroed();
            pic.iColorFormat = openh264_sys2::videoFormatI420;
            pic.iPicWidth = w as i32;
            pic.iPicHeight = h as i32;
            pic.iStride = [w as i32, (w / 2) as i32, (w / 2) as i32, 0];
            let y = self.i420.as_mut_ptr();
            pic.pData = [y, y.add(w * h), y.add(w * h + w * h / 4), std::ptr::null_mut()];
            pic.uiTimeStamp = pts / 10_000;
            let mut info: openh264_sys2::SFrameBSInfo = std::mem::zeroed();
            let encode =
                (**self.encoder).EncodeFrame.ok_or_else(|| EncodeError::Failed("OpenH264 без функций".into()))?;
            let r = encode(self.encoder, &pic, &mut info);
            if r != 0 {
                return Err(EncodeError::Failed(format!("кодирование OpenH264: код {r}")));
            }
            if info.eFrameType == openh264_sys2::videoFrameTypeSkip {
                return Ok(());
            }
            let mut data = Vec::with_capacity(info.iFrameSizeInBytes.max(0) as usize);
            for layer in &info.sLayerInfo[..info.iLayerNum.clamp(0, 128) as usize] {
                let lens = std::slice::from_raw_parts(layer.pNalLengthInByte, layer.iNalCount.max(0) as usize);
                let size: usize = lens.iter().map(|&l| l.max(0) as usize).sum();
                data.extend_from_slice(std::slice::from_raw_parts(layer.pBsBuf, size));
            }
            if data.is_empty() {
                return Ok(());
            }
            let keyframe = info.eFrameType == openh264_sys2::videoFrameTypeIDR;
            out.push(Packet { data, pts, dts: pts, keyframe });
        }
        Ok(())
    }
}

impl Drop for OpenH264Encoder {
    fn drop(&mut self) {
        // SAFETY: энкодер создан в `new`; после Uninitialize уничтожается библиотечной функцией.
        unsafe {
            if let Some(uninit) = (**self.encoder).Uninitialize {
                uninit(self.encoder);
            }
            self.api.WelsDestroySVCEncoder(self.encoder);
        }
    }
}

/// Параметры OpenH264 поверх значений по умолчанию.
fn fill_params(p: &mut openh264_sys2::SEncParamExt, s: &VideoSettings, opts: &EncoderOptions) {
    let bps = (s.bitrate_kbps.max(100) * 1000) as i32;
    let fps = s.fps_num.max(1) as f32 / s.fps_den.max(1) as f32;
    p.iUsageType = openh264_sys2::CAMERA_VIDEO_REAL_TIME;
    p.iPicWidth = s.width as i32;
    p.iPicHeight = s.height as i32;
    p.fMaxFrameRate = fps;
    p.iTargetBitrate = bps;
    p.iMaxBitrate = match s.rate_control {
        RateControl::Cbr => bps,
        _ => bps * 3 / 2,
    };
    match s.rate_control {
        RateControl::Cbr | RateControl::Vbr => p.iRCMode = openh264_sys2::RC_BITRATE_MODE,
        RateControl::Cqp(q) => {
            let q = q.clamp(1, 51) as i32;
            p.iRCMode = openh264_sys2::RC_OFF_MODE;
            (p.iMinQp, p.iMaxQp) = (q, q);
        }
    }
    // Пропуск кадров ломает равномерность записи: лучше превысить битрейт.
    p.bEnableFrameSkip = false;
    p.uiIntraPeriod = s.gop();
    p.eSpsPpsIdStrategy = openh264_sys2::CONSTANT_ID;
    p.iComplexityMode = match opts.preset.clamp(1, 7) {
        1 | 2 => openh264_sys2::LOW_COMPLEXITY,
        3..=5 => openh264_sys2::MEDIUM_COMPLEXITY,
        _ => openh264_sys2::HIGH_COMPLEXITY,
    };
    // CABAC (Main/High) у OpenH264 есть; Baseline — CAVLC.
    p.iEntropyCodingModeFlag = i32::from(opts.profile != H264Profile::Baseline);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 4) as u16;
    p.iMultipleThreadIdc = threads;
    p.iSpatialLayerNum = 1;
    let layer = &mut p.sSpatialLayers[0];
    layer.iVideoWidth = s.width as i32;
    layer.iVideoHeight = s.height as i32;
    layer.fFrameRate = fps;
    layer.iSpatialBitrate = bps;
    layer.iMaxSpatialBitrate = p.iMaxBitrate;
    layer.uiProfileIdc = match opts.profile {
        H264Profile::Baseline => openh264_sys2::PRO_BASELINE,
        H264Profile::Main => openh264_sys2::PRO_MAIN,
        H264Profile::High => openh264_sys2::PRO_HIGH,
    };
    // Несколько потоков работают по срезам: срезов столько же.
    layer.sSliceArgument.uiSliceMode =
        if threads > 1 { openh264_sys2::SM_FIXEDSLCNUM_SLICE } else { openh264_sys2::SM_SINGLE_SLICE };
    layer.sSliceArgument.uiSliceNum = u32::from(threads);
}

/// NV12 (Y, затем чередующиеся UV; строки через `pitch`) → I420 (Y, U, V плотно).
fn nv12_to_i420(src: &[u8], pitch: usize, w: usize, h: usize, dst: &mut [u8]) {
    let (y_plane, chroma) = dst.split_at_mut(w * h);
    let (u_plane, v_plane) = chroma.split_at_mut(w * h / 4);
    for row in 0..h {
        y_plane[row * w..(row + 1) * w].copy_from_slice(&src[row * pitch..row * pitch + w]);
    }
    let uv = &src[pitch * h..];
    for row in 0..h / 2 {
        let line = &uv[row * pitch..row * pitch + w];
        let (u, v) = (&mut u_plane[row * w / 2..(row + 1) * w / 2], &mut v_plane[row * w / 2..(row + 1) * w / 2]);
        for (i, [cb, cr]) in line.as_chunks::<2>().0.iter().enumerate() {
            u[i] = *cb;
            v[i] = *cr;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nv12_splits_into_planes() {
        // 4×2, шаг строки 6 (с хвостом): Y = 0..8, UV = (100,200), (101,201).
        let pitch = 6;
        let mut src = vec![0xEEu8; pitch * 3];
        for r in 0..2 {
            for c in 0..4 {
                src[r * pitch + c] = (r * 4 + c) as u8;
            }
        }
        src[2 * pitch..2 * pitch + 4].copy_from_slice(&[100, 200, 101, 201]);
        let mut dst = vec![0u8; 12];
        nv12_to_i420(&src, pitch, 4, 2, &mut dst);
        assert_eq!(dst, [0, 1, 2, 3, 4, 5, 6, 7, 100, 101, 200, 201]);
    }

    #[test]
    fn params_follow_settings() {
        // SAFETY: обнулённая структура — допустимое состояние для заполнения.
        let mut p: openh264_sys2::SEncParamExt = unsafe { std::mem::zeroed() };
        let s = VideoSettings {
            width: 1280,
            height: 720,
            fps_num: 30,
            fps_den: 1,
            bitrate_kbps: 3000,
            keyframe_interval_s: 2,
            rate_control: RateControl::Cqp(23),
        };
        fill_params(&mut p, &s, &EncoderOptions::default());
        assert_eq!((p.uiIntraPeriod, p.iRCMode, p.iMinQp, p.iMaxQp), (60, openh264_sys2::RC_OFF_MODE, 23, 23));
        assert!(!p.bEnableFrameSkip);
        assert_eq!(p.sSpatialLayers[0].uiProfileIdc, openh264_sys2::PRO_HIGH);
    }
}
