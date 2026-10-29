//! Фильтры канала (§8.3): цепочка до фейдера, блоки interleaved стерео 48 кГц.
//!
//! Цепочку собирает поток движка (там же выделяется память), поток микшера только подменяет
//! её на лету и переносит состояние прежней цепочки ([`FilterChain::adopt`]): правка параметра
//! не сбрасывает огибающие и не даёт щелчка. В самой обработке выделений памяти нет.

use crate::fader::db_to_gain;
use cs_core::config::profile::{AudioFilter, AudioFilterKind};

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
            }
        }
    }

    /// Обрабатывает блок на месте (`buf` — interleaved стерео).
    pub fn process(&mut self, buf: &mut [f32]) {
        for s in self.stages.iter_mut().filter(|s| s.enabled) {
            match &mut s.kind {
                StageKind::Gain(g) => g.process(buf),
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
