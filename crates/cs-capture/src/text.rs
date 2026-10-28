//! Источник «Текст»: DirectWrite → текстура, перерисовка только при изменении (§6.5).

use crate::{Content, SourceState, VideoSource};
use cs_gpu::{GpuDevice, GpuTexture, TextRenderer, TextStyle};
use std::sync::Arc;

pub struct TextSource {
    renderer: Arc<TextRenderer>,
    style: TextStyle,
    texture: Option<GpuTexture>,
    dirty: bool,
    error: Option<String>,
}

impl TextSource {
    pub fn new(renderer: Arc<TextRenderer>, style: TextStyle) -> Self {
        Self { renderer, style, texture: None, dirty: true, error: None }
    }
}

impl VideoSource for TextSource {
    fn update(&mut self, gpu: &GpuDevice) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        match self.renderer.render(gpu, &self.style) {
            Ok(t) => {
                self.texture = Some(t);
                self.error = None;
            }
            Err(e) => {
                tracing::warn!("текст: {e}");
                self.error = Some(e.to_string());
            }
        }
    }

    fn content(&self) -> Option<Content> {
        self.texture.as_ref().map(|t| Content::Texture {
            srv: t.srv.clone(),
            width: t.width,
            height: t.height,
            rotation: 0,
        })
    }

    fn state(&self) -> SourceState {
        match &self.error {
            Some(e) => SourceState::Error(e.clone()),
            None => SourceState::Active,
        }
    }
}
