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
/// Пределы порогов гейта, дБ.
pub const GATE_THRESHOLD_DB: (f32, f32) = (-96.0, 0.0);
/// Пределы атаки, удержания и спада гейта, мс.
pub const GATE_ATTACK_MS: (f32, f32) = (1.0, 500.0);
pub const GATE_HOLD_MS: (f32, f32) = (0.0, 2000.0);
pub const GATE_RELEASE_MS: (f32, f32) = (1.0, 2000.0);

/// Пределы параметров компрессора.
pub const COMP_THRESHOLD_DB: (f32, f32) = (-60.0, 0.0);
pub const COMP_RATIO: (f32, f32) = (1.0, 32.0);
pub const COMP_ATTACK_MS: (f32, f32) = (1.0, 500.0);
pub const COMP_RELEASE_MS: (f32, f32) = (1.0, 1000.0);
pub const COMP_MAKEUP_DB: (f32, f32) = (0.0, 30.0);

/// Сэмплов в миллисекунде на частоте микшера.
const PER_MS: f32 = crate::mixer::SAMPLE_RATE as f32 / 1000.0;

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
    Gate(Gate),
    Compressor(Compressor),
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
                    AudioFilterKind::Gate { open_db, close_db, attack_ms, hold_ms, release_ms } => {
                        StageKind::Gate(Gate::new(open_db, close_db, attack_ms, hold_ms, release_ms))
                    }
                    AudioFilterKind::Compressor { threshold_db, ratio, attack_ms, release_ms, makeup_db } => {
                        StageKind::Compressor(Compressor::new(threshold_db, ratio, attack_ms, release_ms, makeup_db))
                    }
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
                (StageKind::Gate(n), StageKind::Gate(o)) => n.adopt(o),
                (StageKind::Compressor(n), StageKind::Compressor(o)) => n.env = o.env,
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
                StageKind::Gain(_) | StageKind::Gate(_) | StageKind::Compressor(_) => 0,
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
                StageKind::Gate(g) => g.process(buf),
                StageKind::Compressor(c) => c.process(buf),
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

/// Шумовой гейт. Огибающая — пик по обоим каналам (мгновенный подхват, спад 10 мс), чтобы
/// гейт не дёргался на переходах через ноль. Выше порога открытия гейт открывается за время
/// атаки; ниже порога закрытия — после удержания закрывается за время спада. Между порогами
/// состояние сохраняется (гистерезис): затухающее слово не «дребезжит».
struct Gate {
    open: f32,
    close: f32,
    attack_step: f32,
    release_step: f32,
    hold: u32,
    decay: f32,
    env: f32,
    is_open: bool,
    held: u32,
    gain: f32,
}

impl Gate {
    fn new(open_db: f32, close_db: f32, attack_ms: f32, hold_ms: f32, release_ms: f32) -> Self {
        let open_db = open_db.clamp(GATE_THRESHOLD_DB.0, GATE_THRESHOLD_DB.1);
        let close_db = close_db.clamp(GATE_THRESHOLD_DB.0, open_db);
        Self {
            open: db_to_gain(open_db),
            close: db_to_gain(close_db),
            attack_step: 1.0 / (attack_ms.clamp(GATE_ATTACK_MS.0, GATE_ATTACK_MS.1) * PER_MS),
            release_step: 1.0 / (release_ms.clamp(GATE_RELEASE_MS.0, GATE_RELEASE_MS.1) * PER_MS),
            hold: (hold_ms.clamp(GATE_HOLD_MS.0, GATE_HOLD_MS.1) * PER_MS) as u32,
            decay: (-1.0 / (10.0 * PER_MS)).exp(),
            env: 0.0,
            is_open: false,
            held: 0,
            gain: 0.0,
        }
    }

    fn adopt(&mut self, old: &Gate) {
        self.env = old.env;
        self.is_open = old.is_open;
        self.held = old.held;
        self.gain = old.gain;
    }

    fn process(&mut self, buf: &mut [f32]) {
        for f in buf.as_chunks_mut::<2>().0 {
            self.env = f[0].abs().max(f[1].abs()).max(self.env * self.decay);
            if !self.is_open && self.env > self.open {
                self.is_open = true;
            } else if self.is_open && self.env < self.close {
                self.is_open = false;
                self.held = 0;
            }
            if self.is_open {
                self.gain = (self.gain + self.attack_step).min(1.0);
            } else if self.held < self.hold {
                self.held += 1;
            } else {
                self.gain = (self.gain - self.release_step).max(0.0);
            }
            f[0] *= self.gain;
            f[1] *= self.gain;
        }
    }
}

/// Компрессор. Детектор — пик по обоим каналам, сглаженный атакой (рост) и спадом (убывание);
/// превышение над порогом сжимается в `ratio` раз, затем компенсирующее усиление.
struct Compressor {
    threshold_db: f32,
    slope: f32,
    attack: f32,
    release: f32,
    makeup_db: f32,
    env: f32,
}

impl Compressor {
    fn new(threshold_db: f32, ratio: f32, attack_ms: f32, release_ms: f32, makeup_db: f32) -> Self {
        let coef = |ms: f32| (-1.0 / (ms * PER_MS)).exp();
        Self {
            threshold_db: threshold_db.clamp(COMP_THRESHOLD_DB.0, COMP_THRESHOLD_DB.1),
            slope: 1.0 - 1.0 / ratio.clamp(COMP_RATIO.0, COMP_RATIO.1),
            attack: coef(attack_ms.clamp(COMP_ATTACK_MS.0, COMP_ATTACK_MS.1)),
            release: coef(release_ms.clamp(COMP_RELEASE_MS.0, COMP_RELEASE_MS.1)),
            makeup_db: makeup_db.clamp(COMP_MAKEUP_DB.0, COMP_MAKEUP_DB.1),
            env: 0.0,
        }
    }

    fn process(&mut self, buf: &mut [f32]) {
        for f in buf.as_chunks_mut::<2>().0 {
            let level = f[0].abs().max(f[1].abs());
            let k = if level > self.env { self.attack } else { self.release };
            self.env = k * self.env + (1.0 - k) * level;
            let over = 20.0 * self.env.max(1e-9).log10() - self.threshold_db;
            let reduction = if over > 0.0 { over * self.slope } else { 0.0 };
            let g = db_to_gain(self.makeup_db - reduction);
            f[0] *= g;
            f[1] *= g;
        }
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

    fn gate() -> FilterChain {
        FilterChain::new(&[AudioFilter { enabled: true, kind: AudioFilterKind::ALL[2] }])
    }

    /// Синус 1 кГц с пиком `db` dBFS, `ms` миллисекунд.
    fn tone(db: f32, ms: usize) -> Vec<f32> {
        let a = db_to_gain(db);
        (0..ms * 48)
            .flat_map(|i| {
                let v = a * (std::f32::consts::TAU * 1000.0 * i as f32 / 48_000.0).sin();
                [v, v]
            })
            .collect()
    }

    fn peak(s: &[f32]) -> f32 {
        s.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    /// Пробы по 10 мс: громкий сигнал открывает гейт за атаку (25 мс), тихий закрывает его
    /// после удержания (200 мс) за спад (150 мс).
    #[test]
    fn gate_opens_on_attack_and_closes_after_hold_and_release() {
        let mut g = gate();
        let mut loud = tone(-10.0, 100);
        run(&mut g, &mut loud);
        let ms = |s: &[f32], from: usize, to: usize| peak(&s[from * 96..to * 96]);
        assert!(ms(&loud, 0, 5) < 0.6 * db_to_gain(-10.0), "атака ещё идёт");
        assert!((ms(&loud, 40, 100) - db_to_gain(-10.0)).abs() < 1e-3, "открыт полностью");

        let mut quiet = tone(-50.0, 600);
        run(&mut g, &mut quiet);
        let q = db_to_gain(-50.0);
        assert!(ms(&quiet, 20, 180) > 0.95 * q, "удержание: гейт ещё открыт");
        assert!(ms(&quiet, 260, 300) < q && ms(&quiet, 260, 300) > 0.0, "спад");
        assert!(ms(&quiet, 400, 600) == 0.0, "закрыт");
    }

    /// Уровень между порогами (−29 дБ при −32/−26) не меняет состояние гейта.
    #[test]
    fn gate_hysteresis_keeps_state_between_thresholds() {
        let mut closed = gate();
        let mut mid = tone(-29.0, 300);
        run(&mut closed, &mut mid);
        assert_eq!(peak(&mid), 0.0, "закрытый гейт не открывается");

        let mut open = gate();
        run(&mut open, &mut tone(-10.0, 100));
        let mut mid = tone(-29.0, 600);
        run(&mut open, &mut mid);
        assert!(peak(&mid[mid.len() - 9600..]) > 0.9 * db_to_gain(-29.0), "открытый гейт не закрывается");
    }

    fn compressor(threshold_db: f32, ratio: f32, makeup_db: f32) -> FilterChain {
        let kind = AudioFilterKind::Compressor { threshold_db, ratio, attack_ms: 6.0, release_ms: 60.0, makeup_db };
        FilterChain::new(&[AudioFilter { enabled: true, kind }])
    }

    fn peak_db(s: &[f32]) -> f32 {
        20.0 * peak(s).log10()
    }

    /// Синус −6 dBFS при пороге −18 дБ и 4:1: превышение 12 дБ сжимается до 3 дБ → −15 dBFS.
    /// Ниже порога сигнал не меняется; компенсация добавляется целиком.
    #[test]
    fn compressor_static_curve() {
        let mut loud = tone(-6.0, 500);
        run(&mut compressor(-18.0, 4.0, 0.0), &mut loud);
        let settled = peak_db(&loud[loud.len() - 9600..]);
        // Детектор сглаживает пики синуса, поэтому допуск ±1 дБ.
        assert!((settled + 15.0).abs() < 1.0, "{settled} dBFS");

        let mut quiet = tone(-30.0, 300);
        run(&mut compressor(-18.0, 4.0, 0.0), &mut quiet);
        assert!((peak_db(&quiet[quiet.len() - 9600..]) + 30.0).abs() < 0.05);

        let mut quiet = tone(-30.0, 300);
        run(&mut compressor(-18.0, 4.0, 6.0), &mut quiet);
        assert!((peak_db(&quiet[quiet.len() - 9600..]) + 24.0).abs() < 0.05, "компенсация +6 дБ");
    }

    /// Атака 6 мс: скачок громкости сжимается не мгновенно, но за десятки миллисекунд.
    #[test]
    fn compressor_attack_is_gradual() {
        let mut c = compressor(-18.0, 4.0, 0.0);
        run(&mut c, &mut tone(-30.0, 200));
        let mut step = tone(-6.0, 200);
        run(&mut c, &mut step);
        let first = peak_db(&step[..96]); // первая 1 мс
        let later = peak_db(&step[96 * 50..96 * 60]); // 50–60 мс
        assert!(first > -9.0, "в начале сжатие ещё слабое: {first}");
        assert!(later < -13.0, "к 50 мс сжатие почти полное: {later}");
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
