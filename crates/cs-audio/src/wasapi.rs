//! WASAPI (§8.1): звук рабочего стола (loopback устройства вывода) и микрофон (shared mode).
//!
//! Поток захвата на источник: опрос каждые 5 мс (loopback не присылает событий, когда ничего
//! не играет), преобразование в float32 стерео 48 кГц, запись в очередь канала.
//! Устройство по умолчанию отслеживается раз в 2 с; потеря устройства — переподключение.

use crate::mixer::AudioQueue;
use crate::resample::Resampler;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE};
use windows::Win32::Media::Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ,
};

/// Направление устройства.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Устройство вывода (для loopback — «звук рабочего стола»).
    Render,
    /// Устройство ввода (микрофон).
    Capture,
}

/// Аудиоустройство.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

fn flow(d: Direction) -> EDataFlow {
    match d {
        Direction::Render => eRender,
        Direction::Capture => eCapture,
    }
}

fn com_init() {
    // SAFETY: инициализация COM текущего потока (MTA); повторный вызов безопасен.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn enumerator() -> windows::core::Result<IMMDeviceEnumerator> {
    // SAFETY: создание системного COM-объекта.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

fn device_id(dev: &IMMDevice) -> String {
    // SAFETY: строка выделена системой и освобождается CoTaskMemFree.
    unsafe {
        match dev.GetId() {
            Ok(p) => {
                let s = p.to_string().unwrap_or_default();
                CoTaskMemFree(Some(p.0 as *const _));
                s
            }
            Err(_) => String::new(),
        }
    }
}

fn device_name(dev: &IMMDevice) -> String {
    // SAFETY: чтение свойства устройства; PROPVARIANT освобождается своим Drop.
    unsafe {
        dev.OpenPropertyStore(STGM_READ)
            .and_then(|store| store.GetValue(&PKEY_Device_FriendlyName))
            .map(|v| v.to_string())
            .unwrap_or_default()
    }
}

/// Активные устройства заданного направления.
pub fn devices(direction: Direction) -> Vec<DeviceInfo> {
    com_init();
    let Ok(en) = enumerator() else { return vec![] };
    // SAFETY: перечисление через COM; индексы в пределах GetCount.
    unsafe {
        let Ok(coll) = en.EnumAudioEndpoints(flow(direction), DEVICE_STATE_ACTIVE) else { return vec![] };
        let n = coll.GetCount().unwrap_or(0);
        (0..n)
            .filter_map(|i| coll.Item(i).ok())
            .map(|d| DeviceInfo { id: device_id(&d), name: device_name(&d) })
            .collect()
    }
}

/// Идентификатор устройства по умолчанию.
pub fn default_device(direction: Direction) -> Option<DeviceInfo> {
    com_init();
    let en = enumerator().ok()?;
    // SAFETY: запрос устройства по умолчанию.
    let d = unsafe { en.GetDefaultAudioEndpoint(flow(direction), eConsole) }.ok()?;
    Some(DeviceInfo { id: device_id(&d), name: device_name(&d) })
}

/// Формат данных устройства.
#[derive(Clone, Copy)]
enum SampleFormat {
    F32,
    I16,
    I32,
}

struct Stream {
    _client: IAudioClient,
    capture: IAudioCaptureClient,
    format: SampleFormat,
    channels: usize,
    resampler: Resampler,
    device_id: String,
}

fn open(direction: Direction, device: Option<&str>) -> windows::core::Result<(Stream, String)> {
    let en = enumerator()?;
    // SAFETY: COM-вызовы WASAPI; указатель формата освобождается CoTaskMemFree после копирования полей.
    unsafe {
        let dev = match device {
            Some(id) => en.GetDevice(&windows::core::HSTRING::from(id))?,
            None => en.GetDefaultAudioEndpoint(flow(direction), eConsole)?,
        };
        let name = device_name(&dev);
        let client: IAudioClient = dev.Activate(CLSCTX_ALL, None)?;
        let fmt_ptr = client.GetMixFormat()?;
        let fmt = *fmt_ptr;
        let format = if fmt.wFormatTag == WAVE_FORMAT_IEEE_FLOAT as u16 {
            SampleFormat::F32
        } else if fmt.wFormatTag == WAVE_FORMAT_EXTENSIBLE as u16 {
            let ext = *(fmt_ptr as *const WAVEFORMATEXTENSIBLE);
            let sub = ext.SubFormat;
            if sub == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT {
                SampleFormat::F32
            } else if sub == KSDATAFORMAT_SUBTYPE_PCM && fmt.wBitsPerSample == 32 {
                SampleFormat::I32
            } else {
                SampleFormat::I16
            }
        } else {
            SampleFormat::I16
        };
        let flags = if direction == Direction::Render { AUDCLNT_STREAMFLAGS_LOOPBACK } else { 0 };
        // Буфер 100 мс (в единицах 100 нс): запас на опрос раз в 5 мс.
        let r = client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 1_000_000, 0, fmt_ptr, None);
        CoTaskMemFree(Some(fmt_ptr as *const _));
        r?;
        let capture: IAudioCaptureClient = client.GetService()?;
        // Метки loopback уже идут по часам позиции устройства вывода (момент воспроизведения
        // по данным WASAPI), поэтому поправка на задержку вывода не нужна — проверено syncgen.
        client.Start()?;
        let stream = Stream {
            capture,
            format,
            channels: usize::from(fmt.nChannels.max(1)),
            resampler: Resampler::new(fmt.nSamplesPerSec, crate::mixer::SAMPLE_RATE),
            device_id: device_id(&dev),
            _client: client,
        };
        Ok((stream, name))
    }
}

impl Stream {
    /// Забирает все доступные пакеты в очередь (стерео 48 кГц) с отметками времени захвата.
    fn read(&mut self, out: &mut AudioQueue) -> windows::core::Result<()> {
        let mut stereo = Vec::new();
        let mut resampled = Vec::new();
        // SAFETY: буфер пакета валиден между GetBuffer и ReleaseBuffer; размер — frames * channels * bytes.
        unsafe {
            loop {
                let n = self.capture.GetNextPacketSize()?;
                if n == 0 {
                    break;
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                let mut qpc = 0u64;
                self.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, Some(&mut qpc))?;
                // Позиция QPC первого кадра пакета в единицах 100 нс (§5.2 п.4: время захвата);
                // при ошибке метки драйвера время пакета неизвестно.
                let stamp = (qpc != 0 && flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 == 0).then_some(qpc);
                let frames_us = frames as usize;
                stereo.clear();
                stereo.reserve(frames_us * 2);
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null();
                let ch = self.channels;
                for f in 0..frames_us {
                    let (l, r) = if silent {
                        (0.0, 0.0)
                    } else {
                        let get = |c: usize| -> f32 {
                            let idx = f * ch + c.min(ch - 1);
                            match self.format {
                                SampleFormat::F32 => *(data as *const f32).add(idx),
                                SampleFormat::I16 => f32::from(*(data as *const i16).add(idx)) / 32768.0,
                                SampleFormat::I32 => *(data as *const i32).add(idx) as f32 / 2_147_483_648.0,
                            }
                        };
                        // Моно → в оба канала; многоканальный → L/R (первые два).
                        (get(0), get(1))
                    };
                    stereo.push(l);
                    stereo.push(r);
                }
                self.capture.ReleaseBuffer(frames)?;
                resampled.clear();
                self.resampler.process(&stereo, &mut resampled);
                out.push(&resampled, stamp);
            }
        }
        Ok(())
    }
}

/// Общее состояние источника захвата (для UI и микшера).
#[derive(Default)]
pub struct SourceShared {
    /// Очередь сэмплов (interleaved стерео 48 кГц) с отметками времени захвата.
    pub queue: Mutex<AudioQueue>,
    /// Имя текущего устройства (`None` — нет устройства).
    pub device_name: Mutex<Option<String>>,
    /// Время захвата (QPC, 100 нс) сэмпла сразу после последнего в очереди; 0 — неизвестно.
    pub queue_end_qpc: std::sync::atomic::AtomicU64,
    pub stop: AtomicBool,
}

/// Предел очереди источника: 3 с (смещение синхронизации канала — до +2 с, §8.2).
const MAX_QUEUE: usize = crate::mixer::SAMPLE_RATE as usize * 6; // 3 с стерео

/// Поток захвата одного источника.
pub fn spawn(
    direction: Direction,
    device: Option<String>,
    shared: Arc<SourceShared>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name(format!("audio-{direction:?}").to_lowercase()).spawn(move || {
        com_init();
        let mut stream: Option<Stream> = None;
        let mut last_check = Instant::now();
        let mut retry_at = Instant::now();
        while !shared.stop.load(Ordering::Acquire) {
            if stream.is_none() && Instant::now() >= retry_at {
                match open(direction, device.as_deref()) {
                    Ok((s, name)) => {
                        tracing::info!("аудио {direction:?}: {name}");
                        *shared.device_name.lock() = Some(name);
                        stream = Some(s);
                    }
                    Err(e) => {
                        tracing::debug!("аудио {direction:?}: нет устройства ({e})");
                        *shared.device_name.lock() = None;
                        retry_at = Instant::now() + Duration::from_secs(2);
                    }
                }
            }
            // Следование за устройством по умолчанию.
            if device.is_none() && last_check.elapsed() >= Duration::from_secs(2) {
                last_check = Instant::now();
                let current = stream.as_ref().map(|s| s.device_id.clone());
                if let Some(def) = default_device(direction)
                    && current.as_deref().is_some_and(|c| c != def.id)
                {
                    tracing::info!("аудио {direction:?}: устройство по умолчанию сменилось → {}", def.name);
                    stream = None;
                    retry_at = Instant::now();
                }
            }
            if let Some(s) = stream.as_mut() {
                let mut q = shared.queue.lock();
                let read = s.read(&mut q);
                if let Err(e) = read {
                    tracing::warn!("аудио {direction:?}: {e}; переподключение");
                    stream = None;
                    *shared.device_name.lock() = None;
                    retry_at = Instant::now() + Duration::from_millis(500);
                }
                let frames = q.frames();
                if frames > MAX_QUEUE / 2 {
                    q.discard_front(frames - MAX_QUEUE / 2);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })
}
