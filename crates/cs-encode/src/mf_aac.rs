//! AAC-LC через встроенный в Windows энкодер Media Foundation (§9.2): 48 кГц стерео,
//! 96–320 кбит/с (MF поддерживает 96/128/160/192; остальное округляется к ближайшему).

use crate::mfutil::{sample_bytes, startup};
use crate::{EncodeError, Packet};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::CoTaskMemFree;

pub struct MfAacEncoder {
    transform: IMFTransform,
    output_size: u32,
    provides_samples: bool,
    sample_rate: u32,
}

/// Разрешённые энкодером MF значения байт/с (96, 128, 160, 192 кбит/с).
fn nearest_bytes_per_sec(kbps: u32) -> u32 {
    [12_000u32, 16_000, 20_000, 24_000].into_iter().min_by_key(|b| (b * 8 / 1000).abs_diff(kbps)).unwrap_or(20_000)
}

impl MfAacEncoder {
    pub fn new(sample_rate: u32, kbps: u32) -> Result<Self, EncodeError> {
        startup();
        let input = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Audio, guidSubtype: MFAudioFormat_PCM };
        let output = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Audio, guidSubtype: MFAudioFormat_AAC };
        // SAFETY: перечисление и настройка синхронного MFT; массив активаторов освобождается CoTaskMemFree.
        unsafe {
            let mut list: *mut Option<IMFActivate> = std::ptr::null_mut();
            let mut count = 0u32;
            MFTEnumEx(
                MFT_CATEGORY_AUDIO_ENCODER,
                MFT_ENUM_FLAG_SYNCMFT | MFT_ENUM_FLAG_SORTANDFILTER,
                Some(&input),
                Some(&output),
                &mut list,
                &mut count,
            )?;
            let activates: Vec<Option<IMFActivate>> =
                (0..count as usize).map(|i| std::ptr::read(list.add(i))).collect();
            CoTaskMemFree(Some(list as *const _));
            let transform: IMFTransform = activates
                .into_iter()
                .flatten()
                .find_map(|a| a.ActivateObject().ok())
                .ok_or_else(|| EncodeError::Unavailable("нет AAC-энкодера Media Foundation".into()))?;

            let out = MFCreateMediaType()?;
            out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            out.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
            out.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            out.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, sample_rate)?;
            out.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
            out.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, nearest_bytes_per_sec(kbps))?;
            out.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?; // сырые кадры AAC (для MKV/MP4)
            out.SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, 0x29)?;
            transform.SetOutputType(0, &out, 0)?;

            let inp = MFCreateMediaType()?;
            inp.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            inp.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
            inp.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            inp.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, sample_rate)?;
            inp.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
            inp.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
            inp.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, sample_rate * 4)?;
            transform.SetInputType(0, &inp, 0)?;

            let info = transform.GetOutputStreamInfo(0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
            Ok(Self {
                transform,
                output_size: info.cbSize.max(8192),
                provides_samples: info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0,
                sample_rate,
            })
        }
    }

    /// Кодирует блок float32 стерео (interleaved); `pts` — 100 нс от начала записи.
    pub fn encode(&mut self, samples: &[f32], pts: i64, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        let pcm: Vec<u8> = samples.iter().flat_map(|v| ((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
        let frames = (samples.len() / 2) as i64;
        // SAFETY: буфер заполняется в пределах выделенной длины между Lock/Unlock.
        unsafe {
            let buf = MFCreateMemoryBuffer(pcm.len() as u32)?;
            let mut ptr = std::ptr::null_mut();
            buf.Lock(&mut ptr, None, None)?;
            std::ptr::copy_nonoverlapping(pcm.as_ptr(), ptr, pcm.len());
            buf.Unlock()?;
            buf.SetCurrentLength(pcm.len() as u32)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buf)?;
            sample.SetSampleTime(pts)?;
            sample.SetSampleDuration(frames * 10_000_000 / i64::from(self.sample_rate))?;
            self.transform.ProcessInput(0, &sample, 0)?;
        }
        self.collect(out)
    }

    fn collect(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        loop {
            // SAFETY: ProcessOutput с собственным буфером (энкодер AAC MF не выделяет сэмплы сам).
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
                    Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(()),
                    Err(e) => return Err(e.into()),
                }
                if let Some(s) = sample {
                    let data = sample_bytes(&s)?;
                    if !data.is_empty() {
                        let pts = s.GetSampleTime().unwrap_or(0);
                        out.push(Packet { data, pts, dts: pts, keyframe: true });
                    }
                }
            }
        }
    }

    /// Дописывает хвост кодера.
    pub fn drain(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        // SAFETY: штатные сообщения завершения потока.
        unsafe {
            self.transform.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0)?;
            self.transform.ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0)?;
        }
        self.collect(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrate_rounding() {
        assert_eq!(nearest_bytes_per_sec(160), 20_000);
        assert_eq!(nearest_bytes_per_sec(320), 24_000);
        assert_eq!(nearest_bytes_per_sec(96), 12_000);
    }

    /// Кодирование 1 с синуса: AAC-кадры по 1024 сэмпла, монотонные метки времени.
    #[test]
    fn encodes_sine() {
        let mut enc = match MfAacEncoder::new(48_000, 160) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("AAC MF недоступен в этом окружении: {e}");
                return;
            }
        };
        let mut out = Vec::new();
        for b in 0..100i64 {
            let block: Vec<f32> = (0..480)
                .flat_map(|i| {
                    let t = (b * 480 + i) as f32 / 48_000.0;
                    let v = (t * 440.0 * std::f32::consts::TAU).sin() * 0.3;
                    [v, v]
                })
                .collect();
            enc.encode(&block, b * 100_000, &mut out).expect("encode");
        }
        enc.drain(&mut out).expect("drain");
        assert!(out.len() >= 45, "кадров: {}", out.len());
        assert!(out.windows(2).all(|w| w[1].pts > w[0].pts));
    }
}
