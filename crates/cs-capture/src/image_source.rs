//! Источник «Изображение»: PNG, JPEG, WebP (§6.5). Декодирование — в отдельном потоке,
//! загрузка на GPU — в рендер-потоке; анимированные GIF/WebP — v1.x.

use crate::{CaptureMethod, Content, SourceState, VideoSource};
use crossbeam_channel::{Receiver, bounded};
use cs_gpu::{GpuDevice, GpuTexture};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;

type Decoded = Result<(u32, u32, Vec<u8>), String>;

pub struct ImageSource {
    rx: Option<Receiver<Decoded>>,
    texture: Option<GpuTexture>,
    error: Option<String>,
}

/// Декодирует файл в RGBA8 с premultiplied alpha.
pub fn decode(path: &str) -> Decoded {
    let img = image::open(path).map_err(|e| format!("не удалось открыть изображение: {e}"))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut px = rgba.into_raw();
    for p in px.as_chunks_mut::<4>().0 {
        let a = u16::from(p[3]);
        for c in &mut p[..3] {
            *c = ((u16::from(*c) * a + 127) / 255) as u8;
        }
    }
    Ok((w, h, px))
}

impl ImageSource {
    pub fn new(path: &str) -> Self {
        let (tx, rx) = bounded(1);
        let path = path.to_owned();
        let spawned = std::thread::Builder::new().name("image-decode".into()).spawn(move || {
            let _ = tx.send(decode(&path));
        });
        match spawned {
            Ok(_) => Self { rx: Some(rx), texture: None, error: None },
            Err(e) => Self { rx: None, texture: None, error: Some(e.to_string()) },
        }
    }
}

impl VideoSource for ImageSource {
    fn update(&mut self, gpu: &GpuDevice) {
        let Some(rx) = &self.rx else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.rx = None;
        match result.and_then(|(w, h, px)| {
            gpu.create_texture(w, h, DXGI_FORMAT_R8G8B8A8_UNORM, Some(&px), false).map_err(|e| e.to_string())
        }) {
            Ok(t) => self.texture = Some(t),
            Err(e) => {
                tracing::warn!("изображение: {e}");
                self.error = Some(e);
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
        match (&self.error, &self.texture) {
            (Some(e), _) => SourceState::Error(e.clone()),
            (None, Some(_)) => SourceState::Active,
            (None, None) => SourceState::Waiting,
        }
    }

    fn method(&self) -> CaptureMethod {
        CaptureMethod::Static
    }
}
