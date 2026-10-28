//! Микшер (§8.2): блоки по 10 мс (480 кадров @ 48 кГц), float32 стерео.
//!
//! Каждый канал читает свою очередь сэмплов от источника. Недостача добивается тишиной
//! (loopback молчит, когда ничего не играет), избыток сверх 100 мс отбрасывается до 20 мс —
//! простая компенсация дрейфа часов устройства относительно мастер-часов (ADR-0010).
//! После фейдера считаются индикаторы (пик с удержанием 1.5 с, RMS) и сумма по дорожкам 1–6.

use std::collections::VecDeque;

/// Частота микшера.
pub const SAMPLE_RATE: u32 = 48_000;
/// Кадров в блоке (10 мс).
pub const BLOCK_FRAMES: usize = 480;
/// Число дорожек записи (§8.2).
pub const TRACKS: usize = 6;
/// Тишина на шкале индикаторов.
pub const SILENCE_DB: f32 = -60.0;

const MAX_BUFFERED_FRAMES: usize = SAMPLE_RATE as usize / 10; // 100 мс
const TRIM_TO_FRAMES: usize = SAMPLE_RATE as usize / 50; // 20 мс
const PEAK_HOLD_BLOCKS: u32 = 150; // 1.5 с

/// Управление каналом (меняется из UI).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelControl {
    /// Положение фейдера, дБ (`-inf` — тишина).
    pub fader_db: f32,
    pub muted: bool,
    /// Битовая маска дорожек 1–6.
    pub tracks: u8,
}

impl Default for ChannelControl {
    fn default() -> Self {
        Self { fader_db: 0.0, muted: false, tracks: 0b1 }
    }
}

/// Показания индикатора канала (dBFS, после фейдера).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meter {
    pub peak: [f32; 2],
    pub rms: [f32; 2],
    pub peak_hold: [f32; 2],
}

impl Default for Meter {
    fn default() -> Self {
        Self { peak: [SILENCE_DB; 2], rms: [SILENCE_DB; 2], peak_hold: [SILENCE_DB; 2] }
    }
}

/// Готовый блок микса: по дорожкам, interleaved стерео, `BLOCK_FRAMES * 2` сэмплов.
#[derive(Debug, Clone)]
pub struct MixBlock {
    /// Номер блока от начала работы микшера (время = index * 10 мс).
    pub index: u64,
    pub tracks: [Vec<f32>; TRACKS],
}

fn to_db(v: f32) -> f32 {
    if v <= 1e-6 { SILENCE_DB } else { (20.0 * v.log10()).max(SILENCE_DB) }
}

/// Состояние одного канала внутри микшера.
#[derive(Default)]
pub struct ChannelState {
    pub control: ChannelControl,
    pub meter: Meter,
    hold_age: [u32; 2],
    pub underruns: u64,
    pub dropped_frames: u64,
}

/// Микшер без потоков и устройств (чистая логика — покрыта тестами).
pub struct Mixer {
    pub channels: Vec<ChannelState>,
    scratch: Vec<f32>,
    index: u64,
}

impl Mixer {
    pub fn new(channels: usize) -> Self {
        Self {
            channels: (0..channels).map(|_| ChannelState::default()).collect(),
            scratch: vec![0.0; BLOCK_FRAMES * 2],
            index: 0,
        }
    }

    /// Собирает следующий блок из очередей каналов (`queues[i]` — interleaved стерео 48 кГц).
    pub fn mix(&mut self, queues: &mut [&mut VecDeque<f32>]) -> MixBlock {
        let mut tracks: [Vec<f32>; TRACKS] = std::array::from_fn(|_| vec![0.0; BLOCK_FRAMES * 2]);
        for (ch, q) in self.channels.iter_mut().zip(queues.iter_mut()) {
            // Компенсация дрейфа: не даём задержке канала расти.
            let buffered = q.len() / 2;
            if buffered > MAX_BUFFERED_FRAMES {
                let drop = (buffered - TRIM_TO_FRAMES) * 2;
                q.drain(..drop);
                ch.dropped_frames += (drop / 2) as u64;
            }
            let have = (q.len() / 2).min(BLOCK_FRAMES);
            if have < BLOCK_FRAMES && have > 0 {
                ch.underruns += 1;
            }
            let gain = if ch.control.muted { 0.0 } else { crate::fader::db_to_gain(ch.control.fader_db) };
            let s = &mut self.scratch;
            s.fill(0.0);
            for (dst, src) in s.iter_mut().zip(q.drain(..have * 2)) {
                *dst = src * gain;
            }
            // Индикаторы после фейдера.
            for c in 0..2 {
                let mut peak = 0.0f32;
                let mut sum = 0.0f32;
                for f in 0..BLOCK_FRAMES {
                    let v = s[f * 2 + c].abs();
                    peak = peak.max(v);
                    sum += v * v;
                }
                let peak_db = to_db(peak);
                ch.meter.peak[c] = peak_db;
                ch.meter.rms[c] = to_db((sum / BLOCK_FRAMES as f32).sqrt());
                ch.hold_age[c] += 1;
                if peak_db >= ch.meter.peak_hold[c] || ch.hold_age[c] > PEAK_HOLD_BLOCKS {
                    ch.meter.peak_hold[c] = peak_db;
                    ch.hold_age[c] = 0;
                }
            }
            for (t, track) in tracks.iter_mut().enumerate() {
                if ch.control.tracks & (1 << t) != 0 {
                    for (d, v) in track.iter_mut().zip(s.iter()) {
                        *d += v;
                    }
                }
            }
        }
        let block = MixBlock { index: self.index, tracks };
        self.index += 1;
        block
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, amp: f32, frames: usize) -> VecDeque<f32> {
        (0..frames)
            .flat_map(|i| {
                let v = amp * (2.0 * std::f32::consts::PI * freq * i as f32 / SAMPLE_RATE as f32).sin();
                [v, v]
            })
            .collect()
    }

    #[test]
    fn levels_and_gain() {
        let mut m = Mixer::new(1);
        let mut a = sine(1000.0, 0.5, BLOCK_FRAMES);
        let b = m.mix(&mut [&mut a]);
        // Пик синуса 0.5 → −6.02 dBFS, RMS → −9.03 dBFS.
        assert!((m.channels[0].meter.peak[0] + 6.02).abs() < 0.1);
        assert!((m.channels[0].meter.rms[0] + 9.03).abs() < 0.1);
        assert!(b.tracks[0].iter().any(|v| v.abs() > 0.4));
        assert!(b.tracks[1].iter().all(|v| *v == 0.0), "дорожка 2 не назначена");

        m.channels[0].control.fader_db = -6.0;
        let mut a = sine(1000.0, 0.5, BLOCK_FRAMES);
        m.mix(&mut [&mut a]);
        assert!((m.channels[0].meter.peak[0] + 12.04).abs() < 0.15);

        m.channels[0].control.muted = true;
        let mut a = sine(1000.0, 0.5, BLOCK_FRAMES);
        let b = m.mix(&mut [&mut a]);
        assert_eq!(m.channels[0].meter.peak[0], SILENCE_DB);
        assert!(b.tracks[0].iter().all(|v| *v == 0.0));
    }

    #[test]
    fn underrun_is_silence_and_drift_is_trimmed() {
        let mut m = Mixer::new(2);
        let (mut a, mut b2) = (VecDeque::new(), sine(440.0, 0.1, BLOCK_FRAMES * 30));
        let b = m.mix(&mut [&mut a, &mut b2]);
        assert_eq!(b.tracks[0].len(), BLOCK_FRAMES * 2);
        // 300 мс в очереди → обрезано до 20 мс, затем забран блок 10 мс.
        assert_eq!(b2.len() / 2, TRIM_TO_FRAMES - BLOCK_FRAMES);
        assert!(m.channels[1].dropped_frames > 0);
        assert_eq!(b.index, 0);
        assert_eq!(m.mix(&mut [&mut a, &mut b2]).index, 1);
    }

    #[test]
    fn channels_sum_into_tracks() {
        let mut m = Mixer::new(2);
        m.channels[1].control.tracks = 0b11;
        let mut a: VecDeque<f32> = (0..BLOCK_FRAMES * 2).map(|_| 0.25).collect();
        let mut c = a.clone();
        let b = m.mix(&mut [&mut a, &mut c]);
        assert!((b.tracks[0][0] - 0.5).abs() < 1e-6);
        assert!((b.tracks[1][0] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn peak_hold_decays_after_1_5s() {
        let mut m = Mixer::new(1);
        let mut a = sine(1000.0, 0.9, BLOCK_FRAMES);
        m.mix(&mut [&mut a]);
        let held = m.channels[0].meter.peak_hold[0];
        for _ in 0..100 {
            m.mix(&mut [&mut VecDeque::new()]);
        }
        assert_eq!(m.channels[0].meter.peak_hold[0], held, "удерживается 1 с");
        for _ in 0..60 {
            m.mix(&mut [&mut VecDeque::new()]);
        }
        assert_eq!(m.channels[0].meter.peak_hold[0], SILENCE_DB, "сброшено после 1.5 с");
    }
}
