//! Конвертер цвета (§7.3): холст RGBA → NV12 для аппаратного энкодера, без копий в RAM.
//!
//! Две отрисовки в плоскости одной NV12-текстуры (RTV `R8` для Y и `R8G8` для UV);
//! выходное разрешение может отличаться от холста — масштабирование билинейной выборкой
//! (bicubic/Lanczos — отдельные проходы, M2d). Пул текстур кольцевой: энкодер читает
//! кадр асинхронно, текстура переиспользуется через `POOL` кадров.

use crate::{GpuDevice, GpuError, Result};
use windows::Win32::Graphics::Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

const VS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/convert_vs.cso"));
const PS_Y: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/convert_y.cso"));
const PS_UV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/convert_uv.cso"));

/// Глубина пула: больше задержки конвейера аппаратного энкодера (без B-кадров и lookahead).
pub const POOL: usize = 8;

/// Цветовое пространство и диапазон (§7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorParams {
    pub bt601: bool,
    pub full_range: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct Coefs {
    y: [f32; 4],
    u: [f32; 4],
    v: [f32; 4],
    texel: [f32; 2],
    _pad: [f32; 2],
}

/// Коэффициенты RGB → YCbCr (гамма-кодированные значения 0..1).
fn coefs(p: ColorParams) -> ([f32; 4], [f32; 4], [f32; 4]) {
    let (kr, kb) = if p.bt601 { (0.299, 0.114) } else { (0.2126, 0.0722) };
    let kg = 1.0 - kr - kb;
    let (ys, yo, cs, co) =
        if p.full_range { (1.0, 0.0, 1.0, 0.5) } else { (219.0 / 255.0, 16.0 / 255.0, 224.0 / 255.0, 128.0 / 255.0) };
    let cb = [-kr / (2.0 * (1.0 - kb)), -kg / (2.0 * (1.0 - kb)), 0.5];
    let cr = [0.5, -kg / (2.0 * (1.0 - kr)), -kb / (2.0 * (1.0 - kr))];
    (
        [kr * ys, kg * ys, kb * ys, yo],
        [cb[0] * cs, cb[1] * cs, cb[2] * cs, co],
        [cr[0] * cs, cr[1] * cs, cr[2] * cs, co],
    )
}

struct Target {
    texture: ID3D11Texture2D,
    rtv_y: ID3D11RenderTargetView,
    rtv_uv: ID3D11RenderTargetView,
}

pub struct ColorConverter {
    vs: ID3D11VertexShader,
    ps_y: ID3D11PixelShader,
    ps_uv: ID3D11PixelShader,
    constants: ID3D11Buffer,
    sampler: ID3D11SamplerState,
    raster: ID3D11RasterizerState,
    pool: Vec<Target>,
    next: usize,
    width: u32,
    height: u32,
    params: ColorParams,
}

impl ColorConverter {
    /// `width`/`height` — выходное разрешение (чётные значения).
    pub fn new(gpu: &GpuDevice, width: u32, height: u32, params: ColorParams) -> Result<Self> {
        let (width, height) = (width.max(2) & !1, height.max(2) & !1);
        let d = &gpu.device;
        let missing = || GpuError::NoDevice("converter".into());
        // SAFETY: создание объектов конвейера и NV12-текстур с корректными описаниями.
        unsafe {
            let mut vs = None;
            d.CreateVertexShader(VS, None, Some(&mut vs))?;
            let mut ps_y = None;
            d.CreatePixelShader(PS_Y, None, Some(&mut ps_y))?;
            let mut ps_uv = None;
            d.CreatePixelShader(PS_UV, None, Some(&mut ps_uv))?;
            let cbd = D3D11_BUFFER_DESC {
                ByteWidth: size_of::<Coefs>() as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            };
            let mut constants = None;
            d.CreateBuffer(&cbd, None, Some(&mut constants))?;
            let sd = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MaxLOD: f32::MAX,
                ComparisonFunc: D3D11_COMPARISON_NEVER,
                ..Default::default()
            };
            let mut sampler = None;
            d.CreateSamplerState(&sd, Some(&mut sampler))?;
            let rd =
                D3D11_RASTERIZER_DESC { FillMode: D3D11_FILL_SOLID, CullMode: D3D11_CULL_NONE, ..Default::default() };
            let mut raster = None;
            d.CreateRasterizerState(&rd, Some(&mut raster))?;

            let mut pool = Vec::with_capacity(POOL);
            for _ in 0..POOL {
                let desc = D3D11_TEXTURE2D_DESC {
                    Width: width,
                    Height: height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_NV12,
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                    ..Default::default()
                };
                let mut tex = None;
                d.CreateTexture2D(&desc, None, Some(&mut tex))?;
                let texture = tex.ok_or_else(missing)?;
                let rtv = |format| -> Result<ID3D11RenderTargetView> {
                    let vd = D3D11_RENDER_TARGET_VIEW_DESC {
                        Format: format,
                        ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
                        Anonymous: D3D11_RENDER_TARGET_VIEW_DESC_0 { Texture2D: D3D11_TEX2D_RTV { MipSlice: 0 } },
                    };
                    let mut v = None;
                    d.CreateRenderTargetView(&texture, Some(&vd), Some(&mut v))?;
                    v.ok_or_else(missing)
                };
                let rtv_y = rtv(DXGI_FORMAT_R8_UNORM)?;
                let rtv_uv = rtv(DXGI_FORMAT_R8G8_UNORM)?;
                pool.push(Target { texture, rtv_y, rtv_uv });
            }
            Ok(Self {
                vs: vs.ok_or_else(missing)?,
                ps_y: ps_y.ok_or_else(missing)?,
                ps_uv: ps_uv.ok_or_else(missing)?,
                constants: constants.ok_or_else(missing)?,
                sampler: sampler.ok_or_else(missing)?,
                raster: raster.ok_or_else(missing)?,
                pool,
                next: 0,
                width,
                height,
                params,
            })
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Конвертирует холст в следующую NV12-текстуру пула (под блокировкой устройства).
    pub fn convert(
        &mut self,
        gpu: &GpuDevice,
        canvas: &ID3D11ShaderResourceView,
        canvas_size: (u32, u32),
    ) -> ID3D11Texture2D {
        let target = &self.pool[self.next];
        self.next = (self.next + 1) % POOL;
        let (y, u, v) = coefs(self.params);
        let c = Coefs {
            y,
            u,
            v,
            texel: [1.0 / canvas_size.0.max(1) as f32, 1.0 / canvas_size.1.max(1) as f32],
            _pad: [0.0; 2],
        };
        let ctx = &gpu.context;
        let vp = |w: u32, h: u32| D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: w as f32,
            Height: h as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        // SAFETY: объекты принадлежат этому устройству; вызывается под GpuDevice::lock.
        unsafe {
            ctx.UpdateSubresource(&self.constants, 0, None, (&raw const c).cast(), 0, 0);
            ctx.OMSetBlendState(None, None, u32::MAX);
            ctx.RSSetState(&self.raster);
            ctx.IASetInputLayout(None);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            ctx.VSSetShader(&self.vs, None);
            ctx.PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            ctx.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            ctx.PSSetShaderResources(0, Some(&[Some(canvas.clone())]));

            ctx.OMSetRenderTargets(Some(&[Some(target.rtv_y.clone())]), None);
            ctx.RSSetViewports(Some(&[vp(self.width, self.height)]));
            ctx.PSSetShader(&self.ps_y, None);
            ctx.Draw(3, 0);

            ctx.OMSetRenderTargets(Some(&[Some(target.rtv_uv.clone())]), None);
            ctx.RSSetViewports(Some(&[vp(self.width / 2, self.height / 2)]));
            ctx.PSSetShader(&self.ps_uv, None);
            ctx.Draw(3, 0);

            ctx.OMSetRenderTargets(None, None);
            ctx.PSSetShaderResources(0, Some(&[None]));
        }
        target.texture.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(c: [f32; 4], rgb: [f32; 3]) -> f32 {
        c[0] * rgb[0] + c[1] * rgb[1] + c[2] * rgb[2] + c[3]
    }

    #[test]
    fn bt709_limited_reference_values() {
        let (y, u, v) = coefs(ColorParams { bt601: false, full_range: false });
        // Белый → Y=235, U=V=128; чёрный → Y=16.
        assert!((apply(y, [1.0, 1.0, 1.0]) * 255.0 - 235.0).abs() < 0.01);
        assert!((apply(y, [0.0, 0.0, 0.0]) * 255.0 - 16.0).abs() < 0.01);
        assert!((apply(u, [1.0, 1.0, 1.0]) * 255.0 - 128.0).abs() < 0.01);
        assert!((apply(v, [1.0, 1.0, 1.0]) * 255.0 - 128.0).abs() < 0.01);
        // Красный (BT.709): Y≈63, Cb≈102, Cr=240.
        assert!((apply(y, [1.0, 0.0, 0.0]) * 255.0 - 62.6).abs() < 0.5);
        assert!((apply(u, [1.0, 0.0, 0.0]) * 255.0 - 102.0).abs() < 0.5);
        assert!((apply(v, [1.0, 0.0, 0.0]) * 255.0 - 240.0).abs() < 0.5);
    }

    #[test]
    fn full_range_and_bt601() {
        let (y, _, v) = coefs(ColorParams { bt601: true, full_range: true });
        assert!((apply(y, [1.0, 1.0, 1.0]) - 1.0).abs() < 1e-5);
        assert!((apply(y, [1.0, 0.0, 0.0]) - 0.299).abs() < 1e-5);
        assert!((apply(v, [1.0, 0.0, 0.0]) - 1.0).abs() < 1e-5);
    }

    /// NV12 на WARP: белый холст → Y=235, UV=128.
    #[test]
    fn converts_on_warp() {
        let gpu = GpuDevice::new(crate::DriverKind::Warp).expect("WARP");
        let _g = gpu.lock();
        let white = vec![255u8; 64 * 64 * 4];
        let canvas = gpu.create_texture(64, 64, DXGI_FORMAT_R8G8B8A8_UNORM, Some(&white), false).expect("canvas");
        let mut conv =
            ColorConverter::new(&gpu, 32, 32, ColorParams { bt601: false, full_range: false }).expect("conv");
        let nv12 = conv.convert(&gpu, &canvas.srv, (64, 64));
        let px = gpu.read_nv12(&nv12).expect("read");
        assert_eq!(px[0], 235);
        // Плоскость UV начинается после 32×32 байт яркости.
        assert_eq!(px[32 * 32], 128);
        assert_eq!(px[32 * 32 + 1], 128);
    }
}
