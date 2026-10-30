//! Opus (§9.2) через libopus (BSD-3, собирается из исходников `opus-head-sys`, ADR-0015):
//! 48 кГц стерео, кадры по 20 мс. Микшер отдаёт блоки по 10 мс — энкодер копит их до кадра.
//!
//! Задержка кодера (`pre-skip`, ~6.5 мс) пишется в MKV как `CodecDelay`, метки пакетов — время
//! первого отсчёта кадра на входе: плеер вычитает задержку, и звук не уезжает от видео.

use crate::{EncodeError, Packet};
use opus_head_sys as sys;

/// Частота Opus (вход микшера — тоже 48 кГц).
pub const SAMPLE_RATE: u32 = 48_000;
/// Отсчётов на канал в кадре (20 мс).
const FRAME: usize = 960;
/// Длительность кадра, 100 нс.
const FRAME_100NS: i64 = 200_000;
/// Расхождение меток больше этого (пауза, потеря блока) — кадр начинается заново.
const RESYNC_100NS: i64 = 100_000;

pub struct OpusEncoder {
    st: *mut sys::OpusEncoder,
    /// Отсчёты (interleaved стерео), ещё не вошедшие в кадр.
    pending: Vec<f32>,
    /// Время первого отсчёта в `pending`, 100 нс.
    pending_pts: Option<i64>,
    /// Задержка кодера, отсчётов 48 кГц.
    pub pre_skip: u16,
    packet: Vec<u8>,
}

// SAFETY: состояние кодера используется из одного потока (поток аудиоэнкодера).
unsafe impl Send for OpusEncoder {}

impl OpusEncoder {
    /// Кодер «музыка/смешанный сигнал» с битрейтом `kbps` (переменный, сложность 10).
    pub fn new(kbps: u32) -> Result<Self, EncodeError> {
        let mut err = 0;
        // SAFETY: создание кодера libopus; ctl с аргументами нужных типов (opus_int32 / указатель).
        unsafe {
            let st = sys::opus_encoder_create(SAMPLE_RATE as i32, 2, sys::OPUS_APPLICATION_AUDIO as i32, &mut err);
            if st.is_null() || err != sys::OPUS_OK as i32 {
                return Err(EncodeError::Unavailable(format!("opus_encoder_create: {err}")));
            }
            let enc = Self {
                st,
                pending: Vec::with_capacity(FRAME * 4),
                pending_pts: None,
                pre_skip: 0,
                packet: vec![0; 4000],
            };
            let bps = (kbps.clamp(32, 510) * 1000) as i32;
            if sys::opus_encoder_ctl(st, sys::OPUS_SET_BITRATE_REQUEST as i32, bps) != sys::OPUS_OK as i32 {
                return Err(EncodeError::Failed("Opus: битрейт не принят".into()));
            }
            sys::opus_encoder_ctl(st, sys::OPUS_SET_COMPLEXITY_REQUEST as i32, 10i32);
            let mut lookahead = 0i32;
            sys::opus_encoder_ctl(st, sys::OPUS_GET_LOOKAHEAD_REQUEST as i32, &raw mut lookahead);
            let mut enc = enc;
            enc.pre_skip = lookahead.clamp(0, i32::from(u16::MAX)) as u16;
            Ok(enc)
        }
    }

    /// `OpusHead` (RFC 7845 §5.1) — CodecPrivate дорожки MKV.
    pub fn head(&self) -> Vec<u8> {
        let mut h = b"OpusHead".to_vec();
        h.push(1); // версия
        h.push(2); // каналов
        h.extend_from_slice(&self.pre_skip.to_le_bytes());
        h.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        h.extend_from_slice(&0i16.to_le_bytes()); // усиление
        h.push(0); // семейство раскладки: моно/стерео
        h
    }

    /// Задержка кодера, нс (`CodecDelay` в MKV).
    pub fn delay_ns(&self) -> u64 {
        u64::from(self.pre_skip) * 1_000_000_000 / u64::from(SAMPLE_RATE)
    }

    /// Блок float32 стерео (interleaved); `pts` — 100 нс от начала записи.
    pub fn encode(&mut self, samples: &[f32], pts: i64, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        let expected =
            self.pending_pts.map(|p| p + (self.pending.len() / 2) as i64 * 10_000_000 / i64::from(SAMPLE_RATE));
        if expected.is_none_or(|e| (e - pts).abs() > RESYNC_100NS) {
            // Начало или разрыв: недобранный кадр теряется (≤ 20 мс на стыке).
            self.pending.clear();
            self.pending_pts = Some(pts);
        }
        self.pending.extend_from_slice(samples);
        while self.pending.len() >= FRAME * 2 {
            self.encode_frame(out)?;
        }
        Ok(())
    }

    fn encode_frame(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        let pts = self.pending_pts.unwrap_or(0);
        // SAFETY: на входе ровно FRAME отсчётов на канал, выходной буфер — своей длины.
        let n = unsafe {
            sys::opus_encode_float(
                self.st,
                self.pending.as_ptr(),
                FRAME as i32,
                self.packet.as_mut_ptr(),
                self.packet.len() as i32,
            )
        };
        if n < 0 {
            return Err(EncodeError::Failed(format!("opus_encode_float: {n}")));
        }
        out.push(Packet { data: self.packet[..n as usize].to_vec(), pts, dts: pts, keyframe: true });
        self.pending.drain(..FRAME * 2);
        self.pending_pts = Some(pts + FRAME_100NS);
        Ok(())
    }

    /// Дописывает хвост: недобранный кадр дополняется тишиной.
    pub fn drain(&mut self, out: &mut Vec<Packet>) -> Result<(), EncodeError> {
        if !self.pending.is_empty() {
            self.pending.resize(FRAME * 2, 0.0);
            self.encode_frame(out)?;
        }
        Ok(())
    }
}

impl Drop for OpusEncoder {
    fn drop(&mut self) {
        // SAFETY: кодер создан в `new` и больше не используется.
        unsafe { sys::opus_encoder_destroy(self.st) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(block: i64) -> Vec<f32> {
        (0..480)
            .flat_map(|i| {
                let v = ((block * 480 + i) as f32 / 48_000.0 * 440.0 * std::f32::consts::TAU).sin() * 0.3;
                [v, v]
            })
            .collect()
    }

    /// 1 с синуса блоками по 10 мс → 50 кадров по 20 мс с шагом ровно 20 мс.
    #[test]
    fn encodes_frames_of_20ms() {
        let mut enc = OpusEncoder::new(160).expect("opus");
        let mut out = Vec::new();
        for b in 0..100i64 {
            enc.encode(&sine(b), b * 100_000, &mut out).expect("encode");
        }
        enc.drain(&mut out).expect("drain");
        assert_eq!(out.len(), 50);
        assert!(out.iter().enumerate().all(|(i, p)| p.pts == i as i64 * FRAME_100NS && !p.data.is_empty()));
        assert!((240..=480).contains(&enc.pre_skip), "pre-skip {}", enc.pre_skip);
    }

    #[test]
    fn head_is_rfc7845() {
        let enc = OpusEncoder::new(128).expect("opus");
        let h = enc.head();
        assert_eq!(h.len(), 19);
        assert_eq!(&h[..8], b"OpusHead");
        assert_eq!((h[8], h[9]), (1, 2));
        assert_eq!(u16::from_le_bytes([h[10], h[11]]), enc.pre_skip);
        assert_eq!(u32::from_le_bytes([h[12], h[13], h[14], h[15]]), 48_000);
    }

    /// Разрыв меток (пауза) — новый кадр начинается с нового времени, без растяжения.
    #[test]
    fn resyncs_after_gap() {
        let mut enc = OpusEncoder::new(128).expect("opus");
        let mut out = Vec::new();
        for b in 0..3i64 {
            enc.encode(&sine(b), b * 100_000, &mut out).expect("encode");
        }
        for b in 0..2i64 {
            enc.encode(&sine(b), 10_000_000 + b * 100_000, &mut out).expect("encode");
        }
        assert_eq!(out.iter().map(|p| p.pts).collect::<Vec<_>>(), [0, 10_000_000]);
    }
}
