//! Линейный ресемплер для устройств с частотой ≠ 48 кГц (ADR-0010: качественный ресемплер
//! `rubato` — при подключении фильтров в M2d; большинство устройств уже работают на 48 кГц).

/// Потоковый линейный ресемплер стерео-сигнала.
pub struct Resampler {
    step: f64,
    pos: f64,
    prev: [f32; 2],
}

impl Resampler {
    pub fn new(from: u32, to: u32) -> Self {
        Self { step: f64::from(from) / f64::from(to.max(1)), pos: 0.0, prev: [0.0; 2] }
    }

    /// Нужен ли ресемплинг вообще.
    pub fn is_identity(&self) -> bool {
        (self.step - 1.0).abs() < 1e-9
    }

    /// Преобразует блок interleaved стерео и дописывает результат в `out`.
    pub fn process(&mut self, input: &[f32], out: &mut impl Extend<f32>) {
        let frames = input.len() / 2;
        if self.is_identity() {
            out.extend(input.iter().copied());
            return;
        }
        let sample =
            |i: isize, c: usize, prev: &[f32; 2]| -> f32 { if i < 0 { prev[c] } else { input[i as usize * 2 + c] } };
        let mut res = Vec::with_capacity((frames as f64 / self.step) as usize * 2 + 4);
        // pos — позиция относительно начала текущего блока; -1 соответствует последнему кадру прошлого.
        // Строго меньше: точка ровно на последнем кадре берётся в следующем блоке через `prev`.
        while self.pos < frames as f64 - 1.0 {
            let i = self.pos.floor() as isize;
            let t = (self.pos - i as f64) as f32;
            for c in 0..2 {
                let a = sample(i, c, &self.prev);
                let b = sample(i + 1, c, &self.prev);
                res.push(a + (b - a) * t);
            }
            self.pos += self.step;
        }
        self.pos -= frames as f64;
        if frames > 0 {
            self.prev = [input[(frames - 1) * 2], input[(frames - 1) * 2 + 1]];
        }
        out.extend(res);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_ratio_preserved_across_blocks() {
        let mut r = Resampler::new(44_100, 48_000);
        let mut out = Vec::new();
        for _ in 0..100 {
            let block: Vec<f32> = (0..441 * 2).map(|i| (i / 2) as f32 * 0.0).collect();
            r.process(&block, &mut out);
        }
        // 1 секунда 44.1 кГц → ~48000 кадров.
        let frames = out.len() / 2;
        assert!((47_990..=48_010).contains(&frames), "{frames}");
    }

    #[test]
    fn identity_passthrough() {
        let mut r = Resampler::new(48_000, 48_000);
        let mut out = Vec::new();
        r.process(&[0.1, 0.2, 0.3, 0.4], &mut out);
        assert_eq!(out, vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn ramp_is_linear() {
        let mut r = Resampler::new(24_000, 48_000);
        let mut out = Vec::new();
        let input: Vec<f32> = (0..8).flat_map(|i| [i as f32, i as f32]).collect();
        r.process(&input, &mut out);
        // Удвоение частоты: значения через 0.5.
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        assert_eq!(&left[..5], &[0.0, 0.5, 1.0, 1.5, 2.0]);
    }
}
