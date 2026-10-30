//! Ресемплер канала (ADR-0010): частота устройства → 48 кГц и подстройка под дрейф его часов.
//!
//! Асинхронный sinc-ресемплер `rubato` с фиксированным выходом (блок микшера). Коэффициент
//! = 48 кГц / частота устройства × поправка; поправку задаёт P-регулятор по сглаженной ошибке
//! выравнивания канала (метка захвата против шкалы микшера), не больше ±0.5 %. Так канал не
//! уходит от шкалы без выбрасывания звука и щелчков. Память выделяется при создании.

use crate::mixer::{AudioQueue, BLOCK_FRAMES, SAMPLE_RATE};
use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{
    Adjustable, Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

/// Наибольшая поправка скорости (±0.5 %: дрейф устройств — сотни ppm, подтяжка после рывка —
/// секунды; высота звука меняется не больше чем на 9 центов).
const MAX_ADJUST: f64 = 0.005;
/// Коэффициент регулятора: ошибка уходит с постоянной времени ≈ 2 с.
const KP: f64 = 1.0 / (SAMPLE_RATE as f64 * 2.0);
/// Сглаживание ошибки за блок (постоянная времени ≈ 0.5 с): дрожание меток пакетов не
/// превращается в дрожание высоты звука.
const SMOOTH: f64 = 0.02;
/// Частоты устройств от 12 до 192 кГц.
const MAX_RATIO_RELATIVE: f64 = 4.0;

pub struct Drift {
    rs: Async<f32>,
    input: Vec<f32>,
    /// Частота устройства, для которой выставлен коэффициент.
    rate: u32,
    error: f64,
    adjust: f64,
}

impl Default for Drift {
    fn default() -> Self {
        Self::new()
    }
}

impl Drift {
    pub fn new() -> Self {
        let params = SincInterpolationParameters {
            sinc_len: 64,
            f_cutoff: None,
            oversampling_factor: 64,
            interpolation: SincInterpolationType::Linear,
            window: WindowFunction::BlackmanHarris2,
        };
        let rs = Async::<f32>::new_sinc(1.0, MAX_RATIO_RELATIVE, &params, BLOCK_FRAMES, 2, FixedAsync::Output)
            .expect("параметры ресемплера заданы константами");
        let input = vec![0.0; rs.input_frames_max() * 2];
        Self { rs, input, rate: SAMPLE_RATE, error: 0.0, adjust: 0.0 }
    }

    /// Задержка ресемплера, кадров на 48 кГц: микшер берёт звук канала на столько же раньше.
    pub fn delay_frames(&self) -> usize {
        self.rs.output_delay()
    }

    /// Текущая поправка скорости, ppm (для лога).
    pub fn ppm(&self) -> f64 {
        self.adjust * 1e6
    }

    /// Обновляет поправку по ошибке выравнивания (кадры на 48 кГц; «+» — звук канала новее
    /// нужного, то есть его надо брать медленнее).
    pub fn steer(&mut self, error_frames: f64) {
        self.error += SMOOTH * (error_frames - self.error);
        self.adjust = (KP * self.error).clamp(-MAX_ADJUST, MAX_ADJUST);
    }

    /// Сброс регулятора после грубого выравнивания (старт, разрыв в данных).
    pub fn reset(&mut self) {
        self.error = 0.0;
        self.adjust = 0.0;
    }

    /// Кадров очереди, которые нужно отбросить, чтобы сдвинуть выход на `frames` кадров 48 кГц.
    pub fn input_frames_for(&self, frames: usize, rate: u32) -> usize {
        (frames as u64 * u64::from(rate) / u64::from(SAMPLE_RATE)) as usize
    }

    /// Блок 480 кадров (interleaved стерео) из очереди канала. Если данных не хватает,
    /// недостающее добивается тишиной; возвращает `true`, если данные были, но не все.
    pub fn process(&mut self, q: &mut AudioQueue, out: &mut [f32]) -> bool {
        let rate = q.rate();
        let ratio = f64::from(SAMPLE_RATE) / f64::from(rate) * (1.0 + self.adjust);
        // Коэффициент меняется плавно внутри блока; частоту устройства — сразу.
        let _ = self.rs.set_resample_ratio(ratio, rate == self.rate);
        self.rate = rate;
        let need = self.rs.input_frames_next();
        let have = q.frames().min(need);
        let (a, b) = q.samples.as_slices();
        let take = have * 2;
        let from_a = take.min(a.len());
        self.input[..from_a].copy_from_slice(&a[..from_a]);
        self.input[from_a..take].copy_from_slice(&b[..take - from_a]);
        self.input[take..need * 2].fill(0.0);
        q.discard_front(have);
        let done = match (
            InterleavedSlice::new(&self.input[..need * 2], 2, need),
            InterleavedSlice::new_mut(&mut out[..BLOCK_FRAMES * 2], 2, BLOCK_FRAMES),
        ) {
            (Ok(input), Ok(mut output)) => self.rs.process_into_buffer(&input, &mut output, None).is_ok(),
            _ => false,
        };
        if !done {
            out.fill(0.0);
        }
        have > 0 && have < need
    }
}
