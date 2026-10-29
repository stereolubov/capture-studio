//! Микшер (§8.2): блоки по 10 мс (480 кадров @ 48 кГц), float32 стерео.
//!
//! Каждый канал читает свою очередь сэмплов от источника. Недостача добивается тишиной
//! (loopback молчит, когда ничего не играет), избыток сверх 100 мс отбрасывается до 20 мс —
//! простая компенсация дрейфа часов устройства относительно мастер-часов (ADR-0010).
//! Фильтры канала (§8.3) — до фейдера. После фейдера считаются индикаторы (пик с удержанием 1.5 с, RMS) и сумма по дорожкам 1–6.

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

/// Очередь сэмплов канала с отметками времени захвата.
///
/// Каждый пакет захвата добавляет отметку «абсолютный номер кадра ↔ время QPC (100 нс)».
/// Время любого кадра очереди интерполируется от ближайшей отметки, поэтому дыры в данных
/// и отбрасывание при компенсации дрейфа не смещают метки (§5.2 п.4).
#[derive(Debug, Default)]
pub struct AudioQueue {
    /// Interleaved стерео 48 кГц.
    pub samples: VecDeque<f32>,
    /// Абсолютный номер первого кадра очереди.
    head: u64,
    /// (абсолютный номер кадра, время захвата в 100 нс).
    marks: VecDeque<(u64, u64)>,
}

impl AudioQueue {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    /// Добавляет кадры пакета; `capture_100ns` — время захвата первого кадра.
    pub fn push(&mut self, data: &[f32], capture_100ns: Option<u64>) {
        if data.is_empty() {
            return;
        }
        if let Some(t) = capture_100ns {
            self.marks.push_back((self.head + self.frames() as u64, t));
        }
        self.samples.extend(data.iter().copied());
    }

    /// Убирает `frames` кадров из начала очереди.
    pub fn discard_front(&mut self, frames: usize) {
        let frames = frames.min(self.frames());
        self.samples.drain(..frames * 2);
        self.head += frames as u64;
        while self.marks.len() >= 2 && self.marks[1].0 <= self.head {
            self.marks.pop_front();
        }
    }

    /// Время захвата кадра сразу после последнего в очереди (100 нс), если известно.
    pub fn tail_time(&self) -> Option<u64> {
        let &(idx, t) = self.marks.back()?;
        let end = self.head + self.frames() as u64;
        Some(t + end.saturating_sub(idx) * 10_000_000 / u64::from(SAMPLE_RATE))
    }

    /// Время захвата первого кадра очереди (100 нс), если известно.
    pub fn head_time(&self) -> Option<u64> {
        let &(idx, t) = self.marks.iter().take_while(|(i, _)| *i <= self.head).last()?;
        if self.samples.is_empty() {
            return None;
        }
        Some(t + (self.head - idx) * 10_000_000 / u64::from(SAMPLE_RATE))
    }
}

impl From<VecDeque<f32>> for AudioQueue {
    fn from(samples: VecDeque<f32>) -> Self {
        Self { samples, head: 0, marks: VecDeque::new() }
    }
}

/// Управление каналом (меняется из UI).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelControl {
    /// Положение фейдера, дБ (`-inf` — тишина).
    pub fader_db: f32,
    pub muted: bool,
    /// Битовая маска дорожек 1–6.
    pub tracks: u8,
    /// Смещение синхронизации, мс (§8.2: −950…+2000): «+» — звук канала позже.
    pub sync_offset_ms: i32,
}

impl Default for ChannelControl {
    fn default() -> Self {
        Self { fader_db: 0.0, muted: false, tracks: 0b1, sync_offset_ms: 0 }
    }
}

/// Допуск выравнивания канала по времени, кадров (полблока = 5 мс).
const ALIGN_TOLERANCE: i64 = BLOCK_FRAMES as i64 / 2;
/// Жёсткий предел очереди канала (на случай отсутствия меток времени), кадров.
const HARD_CAP_FRAMES: usize = SAMPLE_RATE as usize * 3;

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
    pub filters: crate::filters::FilterChain,
    pub meter: Meter,
    hold_age: [u32; 2],
    pub underruns: u64,
    pub dropped_frames: u64,
}

impl ChannelState {
    /// Смещение звука канала на шкале микшера, 100 нс: ручное смещение синхронизации минус
    /// задержка фильтров (её отрабатываем, беря звук канала раньше).
    pub fn offset_100ns(&self) -> i64 {
        let latency = self.filters.latency_frames() as i64 * 10_000_000 / i64::from(SAMPLE_RATE);
        i64::from(self.control.sync_offset_ms) * 10_000 - latency
    }
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

    /// Номер следующего блока.
    pub fn index(&self) -> u64 {
        self.index
    }

    /// Блок без выравнивания по времени (для каналов без меток).
    pub fn mix(&mut self, queues: &mut [&mut AudioQueue]) -> MixBlock {
        self.mix_aligned(queues, None)
    }

    /// Собирает следующий блок из очередей каналов (`queues[i]` — interleaved стерео 48 кГц).
    ///
    /// `target` — время шкалы блока (100 нс): каждый канал выравнивается так, чтобы в блок попал
    /// звук, захваченный в момент `target − смещение канала` (отставший канал догоняет, опередивший
    /// ждёт). Без меток времени — простая компенсация дрейфа по длине очереди.
    pub fn mix_aligned(&mut self, queues: &mut [&mut AudioQueue], target: Option<u64>) -> MixBlock {
        let mut tracks: [Vec<f32>; TRACKS] = std::array::from_fn(|_| vec![0.0; BLOCK_FRAMES * 2]);
        for (ch, q) in self.channels.iter_mut().zip(queues.iter_mut()) {
            let mut wait = false;
            match (target, q.head_time()) {
                (Some(t), Some(head)) => {
                    let desired = t as i64 - ch.offset_100ns();
                    let diff = (head as i64 - desired) * i64::from(SAMPLE_RATE) / 10_000_000;
                    if diff < -ALIGN_TOLERANCE {
                        // Звук канала старше нужного — пропускаем лишнее.
                        let drop = ((-diff) as usize).min(q.frames());
                        q.discard_front(drop);
                        ch.dropped_frames += drop as u64;
                    } else if diff > ALIGN_TOLERANCE {
                        // Звук канала ещё «из будущего» — в этот блок канал не попадает.
                        wait = true;
                    }
                    if q.frames() > HARD_CAP_FRAMES {
                        let drop = q.frames() - HARD_CAP_FRAMES;
                        q.discard_front(drop);
                        ch.dropped_frames += drop as u64;
                    }
                }
                _ => {
                    // Компенсация дрейфа: не даём задержке канала расти.
                    let buffered = q.frames();
                    if buffered > MAX_BUFFERED_FRAMES {
                        let drop = buffered - TRIM_TO_FRAMES;
                        q.discard_front(drop);
                        ch.dropped_frames += drop as u64;
                    }
                }
            }
            let have = if wait { 0 } else { q.frames().min(BLOCK_FRAMES) };
            if have < BLOCK_FRAMES && have > 0 {
                ch.underruns += 1;
            }
            let gain = if ch.control.muted { 0.0 } else { crate::fader::db_to_gain(ch.control.fader_db) };
            let s = &mut self.scratch;
            s.fill(0.0);
            for (dst, src) in s.iter_mut().zip(q.samples.iter().take(have * 2)) {
                *dst = *src;
            }
            q.discard_front(have);
            // Фильтры работают и на тишине: огибающие гейта/компрессора не обрываются.
            ch.filters.process(s);
            for v in s.iter_mut() {
                *v *= gain;
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

    fn sine(freq: f32, amp: f32, frames: usize) -> AudioQueue {
        (0..frames)
            .flat_map(|i| {
                let v = amp * (2.0 * std::f32::consts::PI * freq * i as f32 / SAMPLE_RATE as f32).sin();
                [v, v]
            })
            .collect::<VecDeque<f32>>()
            .into()
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
    fn filters_run_before_fader() {
        use cs_core::config::profile::{AudioFilter, AudioFilterKind};
        let mut m = Mixer::new(1);
        m.channels[0].filters =
            crate::filters::FilterChain::new(&[AudioFilter { enabled: true, kind: AudioFilterKind::Gain { db: 6.0 } }]);
        m.channels[0].control.fader_db = -6.0;
        let mut a = sine(1000.0, 0.5, BLOCK_FRAMES);
        m.mix(&mut [&mut a]);
        // +6 дБ фильтра и −6 дБ фейдера: пик синуса 0.5 остаётся −6.02 dBFS.
        assert!((m.channels[0].meter.peak[0] + 6.02).abs() < 0.1);
    }

    #[test]
    fn underrun_is_silence_and_drift_is_trimmed() {
        let mut m = Mixer::new(2);
        let (mut a, mut b2) = (AudioQueue::default(), sine(440.0, 0.1, BLOCK_FRAMES * 30));
        let b = m.mix(&mut [&mut a, &mut b2]);
        assert_eq!(b.tracks[0].len(), BLOCK_FRAMES * 2);
        // 300 мс в очереди → обрезано до 20 мс, затем забран блок 10 мс.
        assert_eq!(b2.frames(), TRIM_TO_FRAMES - BLOCK_FRAMES);
        assert!(m.channels[1].dropped_frames > 0);
        assert_eq!(b.index, 0);
        assert_eq!(m.mix(&mut [&mut a, &mut b2]).index, 1);
    }

    #[test]
    fn channels_sum_into_tracks() {
        let mut m = Mixer::new(2);
        m.channels[1].control.tracks = 0b11;
        let mut a: AudioQueue = (0..BLOCK_FRAMES * 2).map(|_| 0.25).collect::<VecDeque<f32>>().into();
        let mut c: AudioQueue = (0..BLOCK_FRAMES * 2).map(|_| 0.25).collect::<VecDeque<f32>>().into();
        let b = m.mix(&mut [&mut a, &mut c]);
        assert!((b.tracks[0][0] - 0.5).abs() < 1e-6);
        assert!((b.tracks[1][0] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn head_time_follows_marks_across_gaps_and_trims() {
        let mut q = AudioQueue::default();
        q.push(&[0.0; 960], Some(1_000_000)); // 480 кадров с t = 0.1 с
        q.push(&[0.0; 960], Some(1_200_000)); // следующий пакет на 10 мс позже ожидаемого (дыра)
        assert_eq!(q.head_time(), Some(1_000_000));
        q.discard_front(240); // +5 мс
        assert_eq!(q.head_time(), Some(1_050_000));
        q.discard_front(240); // переход на второй пакет: берётся его отметка
        assert_eq!(q.head_time(), Some(1_200_000));
        q.discard_front(480);
        assert_eq!(q.head_time(), None);
    }

    fn stamped(start_100ns: u64, frames: usize, value: f32) -> AudioQueue {
        let mut q = AudioQueue::default();
        q.push(&vec![value; frames * 2], Some(start_100ns));
        q
    }

    #[test]
    fn channels_align_by_capture_time_and_offset() {
        let mut m = Mixer::new(2);
        // Канал 0 захвачен с t = 1.000 с, канал 1 — с t = 0.980 с (на 20 мс старше).
        let mut a = stamped(10_000_000, 4800, 0.1);
        let mut b = stamped(9_800_000, 4800, 0.2);
        m.mix_aligned(&mut [&mut a, &mut b], Some(10_000_000));
        // Канал 1 догнал: отброшено 20 мс (960 кадров) + забран блок.
        assert_eq!(b.frames(), 4800 - 960 - BLOCK_FRAMES);
        assert_eq!(a.frames(), 4800 - BLOCK_FRAMES);

        // Смещение +30 мс у канала 0: в блок идёт звук на 30 мс старше шкалы — его ещё нет
        // в очереди (начало 1.000 с), значит канал ждёт и в блок не попадает.
        let mut m = Mixer::new(1);
        m.channels[0].control.sync_offset_ms = 30;
        let mut a = stamped(10_000_000, 4800, 0.5);
        let blk = m.mix_aligned(&mut [&mut a], Some(10_000_000));
        assert!(blk.tracks[0].iter().all(|v| *v == 0.0));
        assert_eq!(a.frames(), 4800);
    }

    #[test]
    fn peak_hold_decays_after_1_5s() {
        let mut m = Mixer::new(1);
        let mut a = sine(1000.0, 0.9, BLOCK_FRAMES);
        m.mix(&mut [&mut a]);
        let held = m.channels[0].meter.peak_hold[0];
        for _ in 0..100 {
            m.mix(&mut [&mut AudioQueue::default()]);
        }
        assert_eq!(m.channels[0].meter.peak_hold[0], held, "удерживается 1 с");
        for _ in 0..60 {
            m.mix(&mut [&mut AudioQueue::default()]);
        }
        assert_eq!(m.channels[0].meter.peak_hold[0], SILENCE_DB, "сброшено после 1.5 с");
    }
}
