//! Конвертер цвета (§7.3): холст RGBA → NV12 (8 бит) или P010 (10 бит) для аппаратного
//! энкодера, без копий в RAM.
//!
//! Две отрисовки в плоскости одной текстуры (RTV `R8`/`R16` для Y и `R8G8`/`R16G16` для UV).
//! Выходное разрешение может отличаться от холста: билинейная выборка прямо при отрисовке либо
//! заранее разделимым ядром (bicubic — Catmull-Rom, Lanczos-3) в два прохода через текстуры
//! с плавающей точкой; при уменьшении ядро растягивается на коэффициент (без муара). Пул
//! текстур кольцевой: энкодер читает кадр асинхронно, текстура переиспользуется через `POOL`.

use crate::{GpuDevice, GpuError, Result};
use windows::Win32::Graphics::Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

const VS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/convert_vs.cso"));
const PS_Y: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/convert_y.cso"));
const PS_UV: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/convert_uv.cso"));
const PS_SCALE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/convert_scale.cso"));

/// Глубина пула: больше задержки конвейера аппаратного энкодера (без B-кадров и lookahead).
pub const POOL: usize = 8;

/// Цветовое пространство и диапазон (§7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorParams {
    pub bt601: bool,
    pub full_range: bool,
}

/// Фильтр масштабирования до выходного разрешения (§7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleKernel {
    /// Билинейная выборка при отрисовке (без лишних проходов).
    #[default]
    Bilinear,
    /// Catmull-Rom 4×4: резкий, слабый «звон».
    Bicubic,
    /// Lanczos-3 6×6: самый резкий, чуть больше «звона» на контрастных границах.
    Lanczos,
}

/// Параметры выхода конвертера.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConvertOptions {
    pub scale: ScaleKernel,
    /// P010 (10 бит) вместо NV12 — для HEVC/AV1 10 бит.
    pub ten_bit: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct Coefs {
    y: [f32; 4],
    u: [f32; 4],
    v: [f32; 4],
    texel: [f32; 2],
    quant: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct ScaleCb {
    axis: [f32; 2],
    src_size: [f32; 2],
    scale: f32,
    taps: f32,
    kind: f32,
    _pad: f32,
}

/// Наибольшее число отсчётов ядра по каждую сторону (уменьшение до ~10× для Lanczos-3).
const MAX_TAPS: f32 = 32.0;

/// Промежуточные текстуры масштабирования: по горизонтали (выход × высота холста), затем по
/// вертикали (выход). Пересоздаются при смене размера холста.
struct ScaleTargets {
    canvas: (u32, u32),
    horizontal: crate::GpuTexture,
    scaled: crate::GpuTexture,
}

/// Коэффициенты RGB → YCbCr (гамма-кодированные значения 0..1).
/// Ограниченный диапазон зависит от разрядности: 16…235 из 255 (8 бит), 64…940 из 1023 (10 бит).
fn coefs(p: ColorParams, ten_bit: bool) -> ([f32; 4], [f32; 4], [f32; 4]) {
    let (kr, kb) = if p.bt601 { (0.299, 0.114) } else { (0.2126, 0.0722) };
    let kg = 1.0 - kr - kb;
    let (m, max) = if ten_bit { (4.0, 1023.0) } else { (1.0, 255.0) };
    let (ys, yo, cs, co) = if p.full_range {
        (1.0, 0.0, 1.0, 0.5)
    } else {
        (219.0 * m / max, 16.0 * m / max, 224.0 * m / max, 128.0 * m / max)
    };
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
    ps_scale: ID3D11PixelShader,
    constants: ID3D11Buffer,
    scale_constants: ID3D11Buffer,
    scale_targets: Option<ScaleTargets>,
    options: ConvertOptions,
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
    pub fn new(gpu: &GpuDevice, width: u32, height: u32, params: ColorParams, options: ConvertOptions) -> Result<Self> {
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
            let mut ps_scale = None;
            d.CreatePixelShader(PS_SCALE, None, Some(&mut ps_scale))?;
            let cbd = |size: usize| D3D11_BUFFER_DESC {
                ByteWidth: size as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            };
            let mut constants = None;
            d.CreateBuffer(&cbd(size_of::<Coefs>()), None, Some(&mut constants))?;
            let mut scale_constants = None;
            d.CreateBuffer(&cbd(size_of::<ScaleCb>()), None, Some(&mut scale_constants))?;
            let (format, y_view, uv_view) = if options.ten_bit {
                (DXGI_FORMAT_P010, DXGI_FORMAT_R16_UNORM, DXGI_FORMAT_R16G16_UNORM)
            } else {
                (DXGI_FORMAT_NV12, DXGI_FORMAT_R8_UNORM, DXGI_FORMAT_R8G8_UNORM)
            };
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
                    Format: format,
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
                let rtv_y = rtv(y_view)?;
                let rtv_uv = rtv(uv_view)?;
                pool.push(Target { texture, rtv_y, rtv_uv });
            }
            Ok(Self {
                vs: vs.ok_or_else(missing)?,
                ps_y: ps_y.ok_or_else(missing)?,
                ps_uv: ps_uv.ok_or_else(missing)?,
                ps_scale: ps_scale.ok_or_else(missing)?,
                constants: constants.ok_or_else(missing)?,
                scale_constants: scale_constants.ok_or_else(missing)?,
                scale_targets: None,
                options,
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

    /// Масштабирует холст до выходного разрешения выбранным ядром (два прохода). `None` —
    /// масштабировать не нужно или фильтр билинейный (выборка прямо при конвертации).
    fn scale(
        &mut self,
        gpu: &GpuDevice,
        canvas: &ID3D11ShaderResourceView,
        canvas_size: (u32, u32),
    ) -> Option<ID3D11ShaderResourceView> {
        let out = (self.width, self.height);
        if self.options.scale == ScaleKernel::Bilinear || canvas_size == out {
            return None;
        }
        if self.scale_targets.as_ref().is_none_or(|t| t.canvas != canvas_size) {
            let f = DXGI_FORMAT_R16G16B16A16_FLOAT;
            let made = gpu.create_texture(out.0, canvas_size.1, f, None, true).and_then(|horizontal| {
                Ok(ScaleTargets {
                    canvas: canvas_size,
                    horizontal,
                    scaled: gpu.create_texture(out.0, out.1, f, None, true)?,
                })
            });
            match made {
                Ok(t) => self.scale_targets = Some(t),
                Err(e) => {
                    tracing::warn!("масштабирование {:?}: {e}; билинейное", self.options.scale);
                    self.options.scale = ScaleKernel::Bilinear;
                    return None;
                }
            }
        }
        let t = self.scale_targets.as_ref()?;
        let (support, kind) = match self.options.scale {
            ScaleKernel::Bicubic => (2.0, 0.0),
            _ => (3.0, 1.0),
        };
        let ctx = &gpu.context;
        let pass = |axis: [f32; 2], src: &ID3D11ShaderResourceView, src_size: (u32, u32), dst: &crate::GpuTexture| {
            let ratio = if axis[0] > 0.0 {
                src_size.0 as f32 / dst.width as f32
            } else {
                src_size.1 as f32 / dst.height as f32
            };
            let scale = ratio.max(1.0);
            let cb = ScaleCb {
                axis,
                src_size: [src_size.0 as f32, src_size.1 as f32],
                scale,
                taps: (support * scale).ceil().clamp(1.0, MAX_TAPS),
                kind,
                _pad: 0.0,
            };
            let vp = D3D11_VIEWPORT {
                Width: dst.width as f32,
                Height: dst.height as f32,
                MaxDepth: 1.0,
                ..Default::default()
            };
            // SAFETY: объекты этого устройства; источник отвязан от выхода до привязки.
            unsafe {
                ctx.UpdateSubresource(&self.scale_constants, 0, None, (&raw const cb).cast(), 0, 0);
                ctx.OMSetRenderTargets(Some(std::slice::from_ref(&dst.rtv)), None);
                ctx.RSSetViewports(Some(&[vp]));
                ctx.PSSetShaderResources(0, Some(&[Some(src.clone())]));
                ctx.Draw(3, 0);
                ctx.PSSetShaderResources(0, Some(&[None]));
            }
        };
        // SAFETY: общее состояние конвейера для обоих проходов.
        unsafe {
            ctx.OMSetBlendState(None, None, u32::MAX);
            ctx.RSSetState(&self.raster);
            ctx.IASetInputLayout(None);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            ctx.VSSetShader(&self.vs, None);
            ctx.PSSetShader(&self.ps_scale, None);
            ctx.PSSetConstantBuffers(1, Some(&[Some(self.scale_constants.clone())]));
        }
        pass([1.0, 0.0], canvas, canvas_size, &t.horizontal);
        pass([0.0, 1.0], &t.horizontal.srv, (out.0, canvas_size.1), &t.scaled);
        // Отвязать выход: иначе D3D11 молча обнулит эту же текстуру на входе конвертации.
        // SAFETY: сброс привязки целей отрисовки.
        unsafe { ctx.OMSetRenderTargets(None, None) };
        Some(t.scaled.srv.clone())
    }

    /// Конвертирует холст в следующую текстуру пула (под блокировкой устройства).
    pub fn convert(
        &mut self,
        gpu: &GpuDevice,
        canvas: &ID3D11ShaderResourceView,
        canvas_size: (u32, u32),
    ) -> ID3D11Texture2D {
        let scaled = self.scale(gpu, canvas, canvas_size);
        let (canvas, canvas_size) = match &scaled {
            Some(srv) => (srv, (self.width, self.height)),
            None => (canvas, canvas_size),
        };
        let target = &self.pool[self.next];
        self.next = (self.next + 1) % POOL;
        let (y, u, v) = coefs(self.params, self.options.ten_bit);
        let c = Coefs {
            y,
            u,
            v,
            texel: [1.0 / canvas_size.0.max(1) as f32, 1.0 / canvas_size.1.max(1) as f32],
            quant: if self.options.ten_bit { 1023.0 } else { 0.0 },
            _pad: 0.0,
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
        let (y, u, v) = coefs(ColorParams { bt601: false, full_range: false }, false);
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
        let (y, _, v) = coefs(ColorParams { bt601: true, full_range: true }, false);
        assert!((apply(y, [1.0, 1.0, 1.0]) - 1.0).abs() < 1e-5);
        assert!((apply(y, [1.0, 0.0, 0.0]) - 0.299).abs() < 1e-5);
        assert!((apply(v, [1.0, 0.0, 0.0]) - 1.0).abs() < 1e-5);
    }

    /// Холст из строк серого `row(x)` (0..255) → плоскость Y после конвертации.
    fn luma(cw: u32, ch: u32, ow: u32, oh: u32, scale: ScaleKernel, row: impl Fn(u32) -> u8) -> Vec<u8> {
        let gpu = GpuDevice::new(crate::DriverKind::Warp).expect("WARP");
        let _g = gpu.lock();
        let px: Vec<u8> = (0..ch)
            .flat_map(|_| {
                (0..cw).flat_map(|x| {
                    let v = row(x);
                    [v, v, v, 255]
                })
            })
            .collect();
        let canvas = gpu.create_texture(cw, ch, DXGI_FORMAT_R8G8B8A8_UNORM, Some(&px), false).expect("canvas");
        let params = ColorParams { bt601: false, full_range: true };
        let mut conv =
            ColorConverter::new(&gpu, ow, oh, params, ConvertOptions { scale, ten_bit: false }).expect("conv");
        let tex = conv.convert(&gpu, &canvas.srv, (cw, ch));
        let mut y = gpu.read_nv12(&tex).expect("read");
        y.truncate((ow * oh) as usize);
        y
    }

    const KERNELS: [ScaleKernel; 3] = [ScaleKernel::Bilinear, ScaleKernel::Bicubic, ScaleKernel::Lanczos];

    /// Ядра нормированы: однотонный цвет не меняется; линейный градиент остаётся линейным
    /// (bicubic и Lanczos воспроизводят линейные функции) — при уменьшении и увеличении.
    #[test]
    fn scaling_keeps_flat_color_and_ramp() {
        for k in KERNELS {
            let flat = luma(64, 16, 32, 8, k, |_| 200);
            assert!(flat.iter().all(|v| v.abs_diff(200) <= 1), "{k:?}: {flat:?}");
            for (cw, ow) in [(128u32, 64u32), (32, 64)] {
                let y = luma(cw, 8, ow, 8, k, |x| (x * 255 / (cw - 1)) as u8);
                // Центр выходного пикселя x — в точке источника (x + 0.5)·cw/ow − 0.5.
                for x in 4..ow - 4 {
                    let src = (x as f32 + 0.5) * cw as f32 / ow as f32 - 0.5;
                    let expected = src * 255.0 / (cw - 1) as f32;
                    let got = f32::from(y[x as usize]);
                    assert!((got - expected).abs() <= 2.0, "{k:?} {cw}→{ow}, x={x}: {got} вместо {expected:.1}");
                }
            }
        }
    }

    /// Полоски в 1 пиксель при уменьшении в 3 раза: растянутое ядро даёт ровный серый, а
    /// билинейная выборка (2 соседних текселя из 3) — ложные полосы (муар).
    #[test]
    fn downscale_kernels_avoid_moire() {
        let stripes = |x: u32| if x.is_multiple_of(2) { 255 } else { 0 };
        // Размах по строке; средний уровень должен остаться серым (~127), иначе «ровный» — это пусто.
        let spread = |y: &[u8]| {
            let row = &y[4..28];
            let mean = row.iter().map(|&v| f32::from(v)).sum::<f32>() / row.len() as f32;
            assert!((mean - 127.5).abs() < 12.0, "средний уровень {mean}");
            row.iter().max().copied().unwrap_or(0) - row.iter().min().copied().unwrap_or(0)
        };
        let bilinear = spread(&luma(96, 8, 32, 8, ScaleKernel::Bilinear, stripes));
        let lanczos = spread(&luma(96, 8, 32, 8, ScaleKernel::Lanczos, stripes));
        let bicubic = spread(&luma(96, 8, 32, 8, ScaleKernel::Bicubic, stripes));
        assert!(bilinear > 60, "билинейная даёт муар: размах {bilinear}");
        assert!(lanczos < 20, "Lanczos без муара: размах {lanczos}");
        assert!(bicubic < 20, "bicubic без муара: размах {bicubic}");
    }

    /// P010: белый (limited) → Y = 940, UV = 512 в старших 10 битах.
    #[test]
    fn p010_on_warp() {
        let gpu = GpuDevice::new(crate::DriverKind::Warp).expect("WARP");
        let _g = gpu.lock();
        let white = vec![255u8; 64 * 64 * 4];
        let canvas = gpu.create_texture(64, 64, DXGI_FORMAT_R8G8B8A8_UNORM, Some(&white), false).expect("canvas");
        let opts = ConvertOptions { scale: ScaleKernel::Lanczos, ten_bit: true };
        let mut conv =
            ColorConverter::new(&gpu, 32, 32, ColorParams { bt601: false, full_range: false }, opts).expect("conv");
        let tex = conv.convert(&gpu, &canvas.srv, (64, 64));
        let px = gpu.read_nv12(&tex).expect("read");
        let word = |i: usize| u16::from_le_bytes([px[i * 2], px[i * 2 + 1]]);
        assert_eq!(word(0), 940 << 6);
        assert_eq!(word(32 * 32), 512 << 6);
        assert_eq!(word(32 * 32 + 1), 512 << 6);
    }

    /// NV12 на WARP: белый холст → Y=235, UV=128.
    #[test]
    fn converts_on_warp() {
        let gpu = GpuDevice::new(crate::DriverKind::Warp).expect("WARP");
        let _g = gpu.lock();
        let white = vec![255u8; 64 * 64 * 4];
        let canvas = gpu.create_texture(64, 64, DXGI_FORMAT_R8G8B8A8_UNORM, Some(&white), false).expect("canvas");
        let mut conv =
            ColorConverter::new(&gpu, 32, 32, ColorParams { bt601: false, full_range: false }, Default::default())
                .expect("conv");
        let nv12 = conv.convert(&gpu, &canvas.srv, (64, 64));
        let px = gpu.read_nv12(&nv12).expect("read");
        assert_eq!(px[0], 235);
        // Плоскость UV начинается после 32×32 байт яркости.
        assert_eq!(px[32 * 32], 128);
        assert_eq!(px[32 * 32 + 1], 128);
    }
}
