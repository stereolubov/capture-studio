//! Аппаратный H.264 через Media Foundation (§9.1, «универсальный аппаратный путь»).
//!
//! Асинхронный аппаратный MFT выбирается по LUID нашего адаптера, получает D3D11-устройство через
//! `IMFDXGIDeviceManager` и читает NV12-текстуры напрямую — кадр не покидает видеопамять (§4).
//! Объект живёт в потоке энкодера (COM-объекты MF не передаются между потоками).

use crate::mfutil::{pack, sample_bytes, startup, variant_bool, variant_u32};
use crate::{EncodeError, Packet, RateControl, VideoSettings};
use std::collections::VecDeque;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::Interface;

/// Максимум кадров, ожидающих приёма энкодером; дальше — отбрасываются старые (§5.2 п.5).
const MAX_PENDING: usize = 4;

pub struct MfH264Encoder {
    transform: IMFTransform,
    events: IMFMediaEventGenerator,
    _manager: IMFDXGIDeviceManager,
    provides_samples: bool,
    output_size: u32,
    need_input: u32,
    pending: VecDeque<(ID3D11Texture2D, i64)>,
    frame_duration: i64,
    draining: bool,
    pub name: String,
    pub dropped: u64,
}

/// Находит аппаратный H.264-энкодер на адаптере устройства.
fn find_encoder(device: &ID3D11Device) -> Result<(IMFTransform, String), EncodeError> {
    let input = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Video, guidSubtype: MFVideoFormat_NV12 };
    let output = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Video, guidSubtype: MFVideoFormat_H264 };
    // SAFETY: массив активаторов выделен MF и освобождается CoTaskMemFree; элементы забираются по значению.
    unsafe {
        let mut attrs = None;
        MFCreateAttributes(&mut attrs, 1)?;
        let attrs = attrs.ok_or_else(|| EncodeError::Unavailable("MFCreateAttributes".into()))?;
        if let Some(luid) = crate::adapter_luid(device) {
            attrs.SetBlob(&MFT_ENUM_ADAPTER_LUID, &luid)?;
        }
        let mut list: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut count = 0u32;
        MFTEnum2(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            Some(&input),
            Some(&output),
            &attrs,
            &mut list,
            &mut count,
        )?;
        let activates: Vec<Option<IMFActivate>> = (0..count as usize).map(|i| std::ptr::read(list.add(i))).collect();
        CoTaskMemFree(Some(list as *const _));
        for a in activates.into_iter().flatten() {
            let mut name = String::new();
            let mut buf = [0u16; 256];
            let mut len = 0u32;
            if a.GetString(&MFT_FRIENDLY_NAME_Attribute, &mut buf, Some(&mut len)).is_ok() {
                name = String::from_utf16_lossy(&buf[..len as usize]);
            }
            match a.ActivateObject::<IMFTransform>() {
                Ok(t) => return Ok((t, name)),
                Err(e) => tracing::warn!("MFT «{name}» не активирован: {e}"),
            }
        }
    }
    Err(EncodeError::Unavailable("нет аппаратного H.264-энкодера Media Foundation на этом GPU".into()))
}

impl MfH264Encoder {
    pub fn new(device: &ID3D11Device, s: &VideoSettings) -> Result<Self, EncodeError> {
        startup();
        let (transform, name) = find_encoder(device)?;
        // SAFETY: последовательность настройки асинхронного MFT по документации MF.
        unsafe {
            let attrs = transform.GetAttributes()?;
            attrs.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1)?;
            let _ = attrs.SetUINT32(&MF_LOW_LATENCY, 0);

            let mut token = 0u32;
            let mut manager = None;
            MFCreateDXGIDeviceManager(&mut token, &mut manager)?;
            let manager = manager.ok_or_else(|| EncodeError::Unavailable("DXGI device manager".into()))?;
            manager.ResetDevice(device, token)?;
            transform.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, manager.as_raw() as usize)?;

            // Тип выхода задаётся раньше входа (требование аппаратных энкодеров).
            let out = MFCreateMediaType()?;
            out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            out.SetUINT32(&MF_MT_AVG_BITRATE, s.bitrate_kbps * 1000)?;
            out.SetUINT64(&MF_MT_FRAME_SIZE, pack(s.width, s.height))?;
            out.SetUINT64(&MF_MT_FRAME_RATE, pack(s.fps_num, s.fps_den))?;
            out.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
            out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            out.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_High.0 as u32)?;
            transform.SetOutputType(0, &out, 0)?;

            let inp = MFCreateMediaType()?;
            inp.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            inp.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
            inp.SetUINT64(&MF_MT_FRAME_SIZE, pack(s.width, s.height))?;
            inp.SetUINT64(&MF_MT_FRAME_RATE, pack(s.fps_num, s.fps_den))?;
            inp.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            transform.SetInputType(0, &inp, 0)?;

            // Управление битрейтом и GOP через ICodecAPI (не все драйверы поддерживают все параметры).
            if let Ok(api) = transform.cast::<ICodecAPI>() {
                let (mode, quality) = match s.rate_control {
                    RateControl::Cbr => (eAVEncCommonRateControlMode_CBR.0 as u32, None),
                    RateControl::Vbr => (eAVEncCommonRateControlMode_PeakConstrainedVBR.0 as u32, None),
                    RateControl::Cqp(q) => (eAVEncCommonRateControlMode_Quality.0 as u32, Some(q)),
                };
                let set = |guid: &windows::core::GUID, v: windows::Win32::System::Variant::VARIANT, what: &str| {
                    if let Err(e) = api.SetValue(guid, &v) {
                        tracing::debug!("ICodecAPI {what}: {e}");
                    }
                };
                set(&CODECAPI_AVEncCommonRateControlMode, variant_u32(mode), "rate control");
                set(&CODECAPI_AVEncCommonMeanBitRate, variant_u32(s.bitrate_kbps * 1000), "bitrate");
                if let Some(q) = quality {
                    // Шкала MF 0..100 (выше — лучше); CQ 1..51 переводится линейно.
                    set(&CODECAPI_AVEncCommonQuality, variant_u32(100 - (q.min(51) * 100 / 51)), "quality");
                }
                set(&CODECAPI_AVEncMPVGOPSize, variant_u32(s.gop()), "GOP");
                // Без B-кадров: PTS = DTS, проще синхронизация и мультиплексирование (M2d — B-кадры).
                set(&CODECAPI_AVEncMPVDefaultBPictureCount, variant_u32(0), "B-frames");
                set(&CODECAPI_AVLowLatencyMode, variant_bool(false), "low latency");
            }

            let info = transform.GetOutputStreamInfo(0)?;
            let provides_samples = info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0;
            transform.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
            let events: IMFMediaEventGenerator = transform.cast()?;
            tracing::info!("видеоэнкодер: {name} (MF), {}×{} {} кбит/с", s.width, s.height, s.bitrate_kbps);
            Ok(Self {
                transform,
                events,
                _manager: manager,
                provides_samples,
                output_size: info.cbSize.max(4 * 1024 * 1024),
                need_input: 0,
                pending: VecDeque::new(),
                frame_duration: s.frame_duration(),
                draining: false,
                name,
                dropped: 0,
            })
        }
    }

    /// Ставит кадр в очередь на кодирование (`pts` — 100 нс от начала записи).
    pub fn push(&mut self, texture: ID3D11Texture2D, pts: i64) {
        if self.pending.len() >= MAX_PENDING {
            self.pending.pop_front();
            self.dropped += 1;
        }
        self.pending.push_back((texture, pts));
    }

    fn feed(&mut self) -> Result<(), EncodeError> {
        while self.need_input > 0 {
            let Some((tex, pts)) = self.pending.pop_front() else { break };
            // SAFETY: буфер ссылается на NV12-текстуру нашего устройства; сэмпл держит ссылку.
            unsafe {
                let buf = MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &tex, 0, false)?;
                let sample = MFCreateSample()?;
                sample.AddBuffer(&buf)?;
                sample.SetSampleTime(pts)?;
                sample.SetSampleDuration(self.frame_duration)?;
                self.transform.ProcessInput(0, &sample, 0)?;
            }
            self.need_input -= 1;
        }
        Ok(())
    }

    fn take_output(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        // SAFETY: ProcessOutput с одним буфером; при собственных сэмплах MFT поле pSample заполняет MF.
        unsafe {
            let own = if self.provides_samples {
                None
            } else {
                let s = MFCreateSample()?;
                s.AddBuffer(&MFCreateMemoryBuffer(self.output_size)?)?;
                Some(s)
            };
            let mut buffers = [MFT_OUTPUT_DATA_BUFFER {
                dwStreamID: 0,
                pSample: std::mem::ManuallyDrop::new(own),
                dwStatus: 0,
                pEvents: std::mem::ManuallyDrop::new(None),
            }];
            let mut status = 0u32;
            let r = self.transform.ProcessOutput(0, &mut buffers, &mut status);
            let sample = std::mem::ManuallyDrop::take(&mut buffers[0].pSample);
            drop(std::mem::ManuallyDrop::take(&mut buffers[0].pEvents));
            match r {
                Ok(()) => {}
                Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                    let t = self.transform.GetOutputAvailableType(0, 0)?;
                    self.transform.SetOutputType(0, &t, 0)?;
                    return Ok(());
                }
                Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(()),
                Err(e) => return Err(e.into()),
            }
            if let Some(sample) = sample {
                let data = sample_bytes(&sample)?;
                let pts = sample.GetSampleTime().unwrap_or(0);
                let dts = sample.GetUINT64(&MFSampleExtension_DecodeTimestamp).map_or(pts, |v| v as i64);
                let keyframe = sample.GetUINT32(&MFSampleExtension_CleanPoint).unwrap_or(0) != 0;
                out.push(Packet { data, pts, dts, keyframe });
            }
        }
        Ok(())
    }

    /// Обрабатывает события MFT без ожидания: подача кадров и получение пакетов.
    pub fn pump(&mut self, out: &mut Vec<Packet>) -> Result<bool, EncodeError> {
        let mut drained = false;
        loop {
            // SAFETY: неблокирующий запрос события асинхронного MFT.
            let ev = match unsafe { self.events.GetEvent(MF_EVENT_FLAG_NO_WAIT) } {
                Ok(ev) => ev,
                Err(e) if e.code() == MF_E_NO_EVENTS_AVAILABLE => break,
                Err(e) => return Err(e.into()),
            };
            // SAFETY: чтение типа события.
            let kind = unsafe { ev.GetType()? };
            match kind as i32 {
                x if x == METransformNeedInput.0 => {
                    self.need_input += 1;
                    self.feed()?;
                }
                x if x == METransformHaveOutput.0 => self.take_output(out)?,
                x if x == METransformDrainComplete.0 => drained = true,
                _ => {}
            }
        }
        self.feed()?;
        Ok(drained)
    }

    /// Завершение: подаёт оставшиеся кадры и забирает все пакеты (до 3 с).
    pub fn drain(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !self.pending.is_empty() && std::time::Instant::now() < deadline {
            self.pump(out)?;
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        if !self.draining {
            self.draining = true;
            // SAFETY: штатное сообщение завершения потока MFT.
            unsafe {
                self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0)?;
                self.transform.ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0)?;
            }
        }
        while std::time::Instant::now() < deadline {
            if self.pump(out)? {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Ok(())
    }
}
