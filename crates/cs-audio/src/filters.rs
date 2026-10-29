//! Фильтры канала (§8.3): цепочка до фейдера, блоки interleaved стерео 48 кГц.
//!
//! Цепочку собирает поток движка (там же выделяется память), поток микшера только подменяет
//! её на лету и переносит состояние прежней цепочки ([`FilterChain::adopt`]): правка параметра
//! не сбрасывает огибающие и не даёт щелчка. В самой обработке выделений памяти нет.

use crate::fader::db_to_gain;
use cs_core::config::profile::{AudioFilter, AudioFilterKind};
use nnnoiseless::DenoiseState;

/// Пределы усиления, дБ (§8.3).
pub const GAIN_RANGE_DB: (f32, f32) = (-30.0, 30.0);

/// Цепочка фильтров одного канала.
#[derive(Default)]
pub struct FilterChain {
    stages: Vec<Stage>,
}

struct Stage {
    enabled: bool,
    kind: StageKind,
}

enum StageKind {
    Gain(Gain),
    Denoise(Box<Denoise>),
}

impl FilterChain {
    /// Цепочка по настройкам (вызывается вне аудиопотока).
    pub fn new(filters: &[AudioFilter]) -> Self {
        let stages = filters
            .iter()
            .map(|f| Stage {
                enabled: f.enabled,
                kind: match f.kind {
                    AudioFilterKind::Gain { db } => StageKind::Gain(Gain::new(db)),
                    AudioFilterKind::NoiseSuppress => StageKind::Denoise(Box::new(Denoise::new())),
                },
            })
            .collect();
        Self { stages }
    }

    /// Переносит состояние из прежней цепочки: фильтр на той же позиции и того же типа
    /// продолжает с того места, где был (плавный переход к новым параметрам).
    pub fn adopt(&mut self, old: &mut FilterChain) {
        for (new, old) in self.stages.iter_mut().zip(old.stages.iter_mut()) {
            match (&mut new.kind, &mut old.kind) {
                (StageKind::Gain(n), StageKind::Gain(o)) => n.current = o.current,
                // Сеть RNNoise продолжает со своим состоянием: без повторного «разгона».
                (StageKind::Denoise(n), StageKind::Denoise(o)) => std::mem::swap(n, o),
                _ => {}
            }
        }
    }

    /// Задержка включённых фильтров, кадров: микшер берёт звук канала на столько же раньше,
    /// чтобы синхронизация не уходила (§8.3).
    pub fn latency_frames(&self) -> usize {
        self.stages
            .iter()
            .filter(|s| s.enabled)
            .map(|s| match s.kind {
                StageKind::Gain(_) => 0,
                StageKind::Denoise(_) => DenoiseState::FRAME_SIZE,
            })
            .sum()
    }

    /// Обрабатывает блок на месте (`buf` — interleaved стерео, блок микшера 480 кадров).
    pub fn process(&mut self, buf: &mut [f32]) {
        for s in self.stages.iter_mut().filter(|s| s.enabled) {
            match &mut s.kind {
                StageKind::Gain(g) => g.process(buf),
                StageKind::Denoise(d) => d.process(buf),
            }
        }
    }
}

/// Усиление с линейным переходом за блок: изменение без щелчка.
struct Gain {
    target: f32,
    current: f32,
}

impl Gain {
    fn new(db: f32) -> Self {
        let g = db_to_gain(db.clamp(GAIN_RANGE_DB.0, GAIN_RANGE_DB.1));
        Self { target: g, current: g }
    }

    fn process(&mut self, buf: &mut [f32]) {
        let frames = buf.len() / 2;
        if self.current == self.target {
            for v in buf.iter_mut() {
                *v *= self.target;
            }
            return;
        }
        let step = (self.target - self.current) / frames.max(1) as f32;
        for frame in buf.as_chunks_mut::<2>().0 {
            self.current += step;
            frame[0] *= self.current;
            frame[1] *= self.current;
        }
        self.current = self.target;
    }
}

/// Шумоподавление RNNoise: кадр 480 сэмплов моно в шкале 16 бит, задержка — один кадр.
struct Denoise {
    states: [Box<DenoiseState<'static>>; 2],
    input: [f32; DenoiseState::FRAME_SIZE],
    output: [f32; DenoiseState::FRAME_SIZE],
}

impl Denoise {
    fn new() -> Self {
        Self {
            states: [DenoiseState::new(), DenoiseState::new()],
            input: [0.0; DenoiseState::FRAME_SIZE],
            output: [0.0; DenoiseState::FRAME_SIZE],
        }
    }

    fn process(&mut self, buf: &mut [f32]) {
        const SCALE: f32 = 32768.0;
        for block in buf.as_chunks_mut::<{ DenoiseState::FRAME_SIZE * 2 }>().0 {
            // Моно-микрофон приходит одинаковым в обоих каналах: считаем сеть один раз.
            let mono = block.as_chunks::<2>().0.iter().all(|f| f[0] == f[1]);
            for c in 0..if mono { 1 } else { 2 } {
                for (i, v) in self.input.iter_mut().enumerate() {
                    *v = block[i * 2 + c] * SCALE;
                }
                self.states[c].process_frame(&mut self.output, &self.input);
                for (i, v) in self.output.iter().enumerate() {
                    block[i * 2 + c] = v / SCALE;
                }
            }
            if mono {
                for f in block.as_chunks_mut::<2>().0 {
                    f[1] = f[0];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gain(db: f32) -> AudioFilter {
        AudioFilter { enabled: true, kind: AudioFilterKind::Gain { db } }
    }

    #[test]
    fn gain_scales_and_clamps() {
        let mut c = FilterChain::new(&[gain(6.0)]);
        let mut buf = [0.25f32; 8];
        c.process(&mut buf);
        assert!((buf[0] - 0.25 * db_to_gain(6.0)).abs() < 1e-6);

        let mut c = FilterChain::new(&[gain(60.0)]); // выше предела → +30 дБ
        let mut buf = [0.001f32; 2];
        c.process(&mut buf);
        assert!((buf[0] - 0.001 * db_to_gain(30.0)).abs() < 1e-6);
    }

    #[test]
    fn disabled_filter_is_bypassed() {
        let mut c = FilterChain::new(&[AudioFilter { enabled: false, kind: AudioFilterKind::Gain { db: -20.0 } }]);
        let mut buf = [0.5f32; 4];
        c.process(&mut buf);
        assert_eq!(buf, [0.5; 4]);
    }

    /// Речеподобный сигнал (гармоники с вибрато, пачками) плюс слабый белый шум.
    fn voice_with_noise(frames: usize, voice: f32, noise: f32) -> Vec<f32> {
        let mut seed = 1u32;
        (0..frames)
            .flat_map(|i| {
                let t = i as f32 / 48_000.0;
                let on = if (t * 3.0) % 1.0 < 0.6 { voice } else { 0.0 };
                let f0 = 150.0 + 30.0 * (t * 5.0).sin();
                let v: f32 = (1..12).map(|h| (std::f32::consts::TAU * f0 * h as f32 * t).sin() / h as f32).sum();
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let n = ((seed >> 9) as f32 / (1u32 << 23) as f32 - 0.5) * noise;
                [v * on + n, v * on + n]
            })
            .collect()
    }

    fn run(chain: &mut FilterChain, signal: &mut [f32]) {
        for block in signal.chunks_mut(960) {
            chain.process(block);
        }
    }

    /// Задержка RNNoise — ровно кадр (её вычитает микшер): проверка по взаимной корреляции.
    #[test]
    fn denoise_latency_is_one_frame() {
        let mut c = FilterChain::new(&[AudioFilter { enabled: true, kind: AudioFilterKind::NoiseSuppress }]);
        assert_eq!(c.latency_frames(), 480);
        let x = voice_with_noise(480 * 150, 0.2, 0.02);
        let mut y = x.clone();
        run(&mut c, &mut y);
        let (s0, len) = (480 * 30 * 2, 480 * 100 * 2);
        let lag = (0..1500)
            .max_by(|a, b| {
                let corr = |l: usize| -> f64 { (s0..s0 + len).step_by(2).map(|i| (x[i] * y[i + l * 2]) as f64).sum() };
                corr(*a).total_cmp(&corr(*b))
            })
            .unwrap_or(0);
        assert!((478..=482).contains(&lag), "задержка {lag} кадров");
        let mut off = c;
        off.stages[0].enabled = false;
        assert_eq!(off.latency_frames(), 0);
    }

    /// Тихий стационарный шум без голоса гасится, а голос остаётся. На синтетическом белом
    /// шуме среднего уровня RNNoise ведёт себя неровно (модель обучена на реальных шумах),
    /// поэтому в тесте — шум фона около −60 dBFS и 5 с на привыкание сети.
    #[test]
    fn denoise_suppresses_noise_keeps_voice() {
        let rms = |s: &[f32]| (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt();
        let filter = [AudioFilter { enabled: true, kind: AudioFilterKind::NoiseSuppress }];
        let mut noise = voice_with_noise(48_000 * 5, 0.0, 0.002);
        let tail = noise.len() - 96_000;
        let before = rms(&noise[tail..]);
        run(&mut FilterChain::new(&filter), &mut noise);
        let after = rms(&noise[tail..]);
        assert!(after < before * 0.1, "шум {before} → {after}");

        let mut voice = voice_with_noise(480 * 100, 0.2, 0.0);
        let before = rms(&voice[480 * 40..]);
        run(&mut FilterChain::new(&filter), &mut voice);
        let after = rms(&voice[480 * 40..]);
        assert!(after > before * 0.5, "голос {before} → {after}");
    }

    /// Смена параметра переносит состояние: переход идёт плавно от прежнего значения.
    #[test]
    fn adopt_ramps_from_previous_gain() {
        let mut old = FilterChain::new(&[gain(0.0)]);
        let mut new = FilterChain::new(&[gain(-6.0)]);
        new.adopt(&mut old);
        let mut buf = [1.0f32; 960];
        new.process(&mut buf);
        assert!(buf[0] > 0.99, "начало блока — прежнее усиление");
        assert!((buf[958] - db_to_gain(-6.0)).abs() < 1e-3, "конец блока — новое");
        let mut buf = [1.0f32; 4];
        new.process(&mut buf);
        assert!((buf[0] - db_to_gain(-6.0)).abs() < 1e-6);
    }
}
