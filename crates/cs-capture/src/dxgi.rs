//! DXGI Desktop Duplication: захват монитора (§6.4, метод D).
//!
//! Учитывает поворот монитора и `DXGI_ERROR_ACCESS_LOST` (смена режима, UAC, полноэкранное
//! приложение) — дубликатор пересоздаётся. Монитор на другом адаптере (гибридная графика)
//! этим методом недоступен — источник сообщает об ошибке и предлагает WGC.
//! Курсор в этом методе пока не рисуется (M1).

use crate::enumerate;
use crate::{CaptureMethod, Content, SourceState, VideoSource};
use cs_gpu::{GpuDevice, GpuTexture};
use std::time::{Duration, Instant};
use windows::Win32::Graphics::Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::Interface;

const RETRY: Duration = Duration::from_secs(1);

pub struct DuplicationSource {
    index: u32,
    dup: Option<IDXGIOutputDuplication>,
    rotation: u8,
    texture: Option<GpuTexture>,
    state: SourceState,
    last_attempt: Option<Instant>,
}

impl DuplicationSource {
    pub fn new(_gpu: &GpuDevice, index: u32) -> Self {
        Self { index, dup: None, rotation: 0, texture: None, state: SourceState::Waiting, last_attempt: None }
    }

    fn start(&mut self, gpu: &GpuDevice) -> Result<(), String> {
        let m =
            enumerate::monitor_by_index(self.index).ok_or_else(|| format!("монитор {} не найден", self.index + 1))?;
        // SAFETY: COM-вызовы на живых интерфейсах устройства.
        unsafe {
            let adapter =
                gpu.device.cast::<IDXGIDevice>().map_err(|e| e.to_string())?.GetAdapter().map_err(|e| e.to_string())?;
            let mut i = 0;
            while let Ok(output) = adapter.EnumOutputs(i) {
                i += 1;
                let desc = output.GetDesc().map_err(|e| e.to_string())?;
                if desc.Monitor.0 as isize != m.handle {
                    continue;
                }
                self.rotation = match desc.Rotation {
                    DXGI_MODE_ROTATION_ROTATE90 => 1,
                    DXGI_MODE_ROTATION_ROTATE180 => 2,
                    DXGI_MODE_ROTATION_ROTATE270 => 3,
                    _ => 0,
                };
                let o1: IDXGIOutput1 = output.cast().map_err(|e| e.to_string())?;
                let dup = o1.DuplicateOutput(&gpu.device).map_err(|e| e.to_string())?;
                self.dup = Some(dup);
                return Ok(());
            }
        }
        Err("монитор подключён к другому GPU — выберите метод WGC".into())
    }

    fn pull(&mut self, gpu: &GpuDevice) -> windows::core::Result<()> {
        let Some(dup) = &self.dup else { return Ok(()) };
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;
        // SAFETY: таймаут 0 — без ожидания; кадр освобождается ReleaseFrame в любом случае.
        unsafe {
            match dup.AcquireNextFrame(0, &mut info, &mut resource) {
                Ok(()) => {}
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(()),
                Err(e) => return Err(e),
            }
            let result = (|| -> windows::core::Result<()> {
                let Some(res) = resource else { return Ok(()) };
                if info.LastPresentTime == 0 && self.texture.is_some() {
                    return Ok(()); // изменился только курсор
                }
                let src: ID3D11Texture2D = res.cast()?;
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                src.GetDesc(&mut desc);
                if self.texture.as_ref().is_none_or(|t| (t.width, t.height) != (desc.Width, desc.Height)) {
                    self.texture = gpu.create_texture(desc.Width, desc.Height, desc.Format, None, false).ok();
                }
                if let Some(t) = &self.texture {
                    gpu.context.CopyResource(&t.texture, &src);
                }
                Ok(())
            })();
            let _ = dup.ReleaseFrame();
            result
        }
    }
}

impl VideoSource for DuplicationSource {
    fn update(&mut self, gpu: &GpuDevice) {
        if self.dup.is_none() {
            let now = Instant::now();
            if self.last_attempt.is_some_and(|t| now - t < RETRY) {
                return;
            }
            self.last_attempt = Some(now);
            match self.start(gpu) {
                Ok(()) => self.state = SourceState::Active,
                Err(e) => {
                    self.state = SourceState::Error(e);
                    return;
                }
            }
        }
        if let Err(e) = self.pull(gpu) {
            // ACCESS_LOST и пр. — пересоздание при следующей попытке.
            tracing::info!("DXGI Duplication: {e}; переинициализация");
            self.dup = None;
            self.state = SourceState::Waiting;
        }
    }

    fn content(&self) -> Option<Content> {
        self.texture.as_ref().map(|t| Content::Texture {
            srv: t.srv.clone(),
            width: t.width,
            height: t.height,
            rotation: self.rotation,
        })
    }

    fn state(&self) -> SourceState {
        self.state.clone()
    }

    fn method(&self) -> CaptureMethod {
        CaptureMethod::DxgiDuplication
    }
}
