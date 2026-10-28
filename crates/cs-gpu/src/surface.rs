//! Поверхности для рендера UI: окно (swapchain через DirectComposition) и
//! внеэкранная цель (скриншот-тесты).
//!
//! DirectComposition выбран, чтобы окна поддерживали попиксельную прозрачность
//! (мини-режим с фоном `#17181bee`, §11.6) без legacy layered windows (ADR-0002).

use crate::{GpuDevice, GpuError, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::Interface;

/// Swapchain окна с premultiplied alpha, показываемый через DirectComposition.
pub struct WindowSurface {
    swapchain: IDXGISwapChain1,
    rtv: Option<ID3D11RenderTargetView>,
    size: [u32; 2],
    _dcomp: IDCompositionDevice,
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
}

impl WindowSurface {
    pub fn new(gpu: &GpuDevice, hwnd: HWND, size: [u32; 2]) -> Result<Self> {
        let size = [size[0].max(1), size[1].max(1)];
        // SAFETY: COM-вызовы на валидных интерфейсах; hwnd принадлежит вызывающему потоку
        // и живёт дольше поверхности (поверхность уничтожается до окна).
        unsafe {
            let dxgi_device: IDXGIDevice = gpu.device.cast()?;
            let adapter = dxgi_device.GetAdapter()?;
            let factory: IDXGIFactory2 = adapter.GetParent()?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: size[0],
                Height: size[1],
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: false.into(),
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                Flags: 0,
            };
            let swapchain = factory.CreateSwapChainForComposition(&gpu.device, &desc, None)?;
            let dcomp: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;
            let target = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            visual.SetContent(&swapchain)?;
            target.SetRoot(&visual)?;
            dcomp.Commit()?;
            let mut s = Self { swapchain, rtv: None, size, _dcomp: dcomp, _target: target, _visual: visual };
            s.create_rtv(gpu)?;
            Ok(s)
        }
    }

    fn create_rtv(&mut self, gpu: &GpuDevice) -> Result<()> {
        // SAFETY: буфер 0 swapchain'а существует после создания/ResizeBuffers.
        unsafe {
            let back: ID3D11Texture2D = self.swapchain.GetBuffer(0)?;
            let mut rtv = None;
            gpu.device.CreateRenderTargetView(&back, None, Some(&mut rtv))?;
            self.rtv = rtv;
        }
        Ok(())
    }

    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    /// Изменяет размер буферов (вызывать по `WM_SIZE`).
    pub fn resize(&mut self, gpu: &GpuDevice, size: [u32; 2]) -> Result<()> {
        let size = [size[0].max(1), size[1].max(1)];
        if size == self.size {
            return Ok(());
        }
        self.rtv = None;
        // SAFETY: все ссылки на буферы (RTV) освобождены выше; контекст сброшен.
        unsafe {
            gpu.context.OMSetRenderTargets(None, None);
            gpu.context.Flush();
            self.swapchain.ResizeBuffers(0, size[0], size[1], DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0))?;
        }
        self.size = size;
        self.create_rtv(gpu)
    }

    /// Очищает буфер цветом (premultiplied RGBA 0..1) и возвращает RTV для рисования.
    pub fn begin(&self, gpu: &GpuDevice, clear: [f32; 4]) -> Result<ID3D11RenderTargetView> {
        let rtv = self.rtv.clone().ok_or(GpuError::DeviceLost)?;
        // SAFETY: RTV валиден до следующего resize.
        unsafe { gpu.context.ClearRenderTargetView(&rtv, &clear) };
        Ok(rtv)
    }

    /// Показывает кадр. `vsync = true` — синхронизация с обновлением монитора.
    pub fn present(&self, vsync: bool) -> Result<()> {
        // SAFETY: обычный Present на валидном swapchain.
        let hr = unsafe { self.swapchain.Present(u32::from(vsync), DXGI_PRESENT(0)) };
        if hr == DXGI_ERROR_DEVICE_REMOVED || hr == DXGI_ERROR_DEVICE_RESET {
            return Err(GpuError::DeviceLost);
        }
        hr.ok()?;
        Ok(())
    }
}

/// Внеэкранная цель рендера с чтением пикселей (только для тестов и `--screenshot`;
/// копирование GPU→CPU в основном пути запрещено, §4).
pub struct OffscreenTarget {
    texture: ID3D11Texture2D,
    staging: ID3D11Texture2D,
    rtv: ID3D11RenderTargetView,
    size: [u32; 2],
}

impl OffscreenTarget {
    pub fn new(gpu: &GpuDevice, size: [u32; 2]) -> Result<Self> {
        let mut desc = D3D11_TEXTURE2D_DESC {
            Width: size[0],
            Height: size[1],
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            ..Default::default()
        };
        let missing = || GpuError::NoDevice("offscreen".into());
        // SAFETY: корректные описания ресурсов.
        unsafe {
            let mut texture = None;
            gpu.device.CreateTexture2D(&desc, None, Some(&mut texture))?;
            let texture = texture.ok_or_else(missing)?;
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            let mut staging = None;
            gpu.device.CreateTexture2D(&desc, None, Some(&mut staging))?;
            let mut rtv = None;
            gpu.device.CreateRenderTargetView(&texture, None, Some(&mut rtv))?;
            Ok(Self { texture, staging: staging.ok_or_else(missing)?, rtv: rtv.ok_or_else(missing)?, size })
        }
    }

    pub fn rtv(&self) -> &ID3D11RenderTargetView {
        &self.rtv
    }

    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    pub fn clear(&self, gpu: &GpuDevice, color: [f32; 4]) {
        // SAFETY: RTV валиден всё время жизни цели.
        unsafe { gpu.context.ClearRenderTargetView(&self.rtv, &color) };
    }

    /// Читает пиксели как RGBA8 (без premultiply-коррекции: фон непрозрачный).
    pub fn read_rgba(&self, gpu: &GpuDevice) -> Result<Vec<u8>> {
        let [w, h] = self.size;
        let mut out = vec![0u8; (w * h * 4) as usize];
        // SAFETY: staging-текстура того же размера и формата; Map/Unmap парные;
        // чтение строк в пределах RowPitch.
        unsafe {
            gpu.context.CopyResource(&self.staging, &self.texture);
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            gpu.context.Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
            for y in 0..h as usize {
                let src = std::slice::from_raw_parts(m.pData.cast::<u8>().add(y * m.RowPitch as usize), w as usize * 4);
                let dst = &mut out[y * w as usize * 4..(y + 1) * w as usize * 4];
                for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0) {
                    *d = [s[2], s[1], s[0], s[3]];
                }
            }
            gpu.context.Unmap(&self.staging, 0);
        }
        Ok(out)
    }
}
