//! Источник «Цвет»: прямоугольник заданного цвета и размера (§6.5).

use crate::{Content, SourceState, VideoSource};
use cs_gpu::GpuDevice;

pub struct ColorSource {
    rgba: [f32; 4],
    width: u32,
    height: u32,
}

impl ColorSource {
    pub fn new(rgba: [u8; 4], width: u32, height: u32) -> Self {
        let a = f32::from(rgba[3]) / 255.0;
        // Premultiplied alpha.
        let c = |v: u8| f32::from(v) / 255.0 * a;
        Self { rgba: [c(rgba[0]), c(rgba[1]), c(rgba[2]), a], width: width.max(1), height: height.max(1) }
    }
}

impl VideoSource for ColorSource {
    fn update(&mut self, _gpu: &GpuDevice) {}

    fn content(&self) -> Option<Content> {
        Some(Content::Color { rgba: self.rgba, width: self.width, height: self.height })
    }

    fn state(&self) -> SourceState {
        SourceState::Active
    }
}
