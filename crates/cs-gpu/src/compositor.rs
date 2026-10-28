//! Компоновщик сцены (§7.3): рисует элементы сцены в текстуру холста на едином устройстве.
//!
//! * Холст `R8G8B8A8_UNORM`, фон — чёрный непрозрачный.
//! * Элемент — прямоугольник с текстурой источника (обрезка через UV, поворот на 90° —
//!   перестановкой UV углов) или сплошной цвет.
//! * Переход «Затухание»: старая и новая сцены рисуются в промежуточные текстуры и
//!   смешиваются на холсте.
//! * Время компоновки на GPU — timestamp-запросы с задержкой в несколько кадров (без ожидания).
//!
//! Все методы, принимающие `&GpuDevice`, вызываются под [`GpuDevice::lock`].

use crate::{GpuDevice, GpuError, GpuTexture, Result};
use windows::Win32::Graphics::Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R32G32_FLOAT};
use windows::core::s;

const VS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/compose_vs.cso"));
const PS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/compose_ps.cso"));

/// Чем заполнять элемент.
#[derive(Clone)]
pub enum DrawSource {
    Texture(ID3D11ShaderResourceView),
    /// Premultiplied RGBA 0..1.
    Color([f32; 4]),
}

/// Элемент к отрисовке (координаты холста в пикселях).
#[derive(Clone)]
pub struct DrawItem {
    pub source: DrawSource,
    /// x, y, w, h на холсте.
    pub rect: [f32; 4],
    /// u0, v0, u1, v1 (обрезка).
    pub uv: [f32; 4],
    /// Поворот по часовой стрелке: 0..=3 (×90°).
    pub rotation: u8,
    pub opacity: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ItemConstants {
    color: [f32; 4],
    textured: f32,
    _pad: [f32; 3],
}

/// Замер времени на GPU без блокировки: кольцо запросов, результат читается через несколько кадров.
struct GpuTimer {
    sets: Vec<(ID3D11Query, ID3D11Query, ID3D11Query)>,
    next: usize,
    pending: Vec<bool>,
}

impl GpuTimer {
    const RING: usize = 4;

    fn new(device: &ID3D11Device) -> Result<Self> {
        let mk = |q| -> Result<ID3D11Query> {
            let mut out = None;
            // SAFETY: корректное описание запроса.
            unsafe { device.CreateQuery(&D3D11_QUERY_DESC { Query: q, MiscFlags: 0 }, Some(&mut out))? };
            out.ok_or_else(|| GpuError::NoDevice("query".into()))
        };
        let mut sets = Vec::with_capacity(Self::RING);
        for _ in 0..Self::RING {
            sets.push((mk(D3D11_QUERY_TIMESTAMP_DISJOINT)?, mk(D3D11_QUERY_TIMESTAMP)?, mk(D3D11_QUERY_TIMESTAMP)?));
        }
        Ok(Self { sets, next: 0, pending: vec![false; Self::RING] })
    }

    fn begin(&self, ctx: &ID3D11DeviceContext) {
        let (d, a, _) = &self.sets[self.next];
        // SAFETY: запросы созданы на этом устройстве.
        unsafe {
            ctx.Begin(d);
            ctx.End(a);
        }
    }

    fn end(&mut self, ctx: &ID3D11DeviceContext) {
        let (d, _, b) = &self.sets[self.next];
        // SAFETY: см. begin.
        unsafe {
            ctx.End(b);
            ctx.End(d);
        }
        self.pending[self.next] = true;
        self.next = (self.next + 1) % Self::RING;
    }

    /// Самый старый готовый результат, мс.
    fn poll(&mut self, ctx: &ID3D11DeviceContext) -> Option<f32> {
        let i = self.next; // самый старый в кольце
        if !self.pending[i] {
            return None;
        }
        let (d, a, b) = &self.sets[i];
        let mut dj = D3D11_QUERY_DATA_TIMESTAMP_DISJOINT::default();
        let (mut t0, mut t1) = (0u64, 0u64);
        // SAFETY: буферы соответствуют типам данных запросов; DONOTFLUSH — без ожидания.
        unsafe {
            let flags = D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32;
            let ok = ctx.GetData(d, Some((&raw mut dj).cast()), size_of_val(&dj) as u32, flags).is_ok()
                && ctx.GetData(a, Some((&raw mut t0).cast()), 8, flags).is_ok()
                && ctx.GetData(b, Some((&raw mut t1).cast()), 8, flags).is_ok();
            // S_FALSE («не готово») — тоже успех для windows-rs: признак готовности — ненулевые данные.
            if !ok || dj.Frequency == 0 || t0 == 0 || t1 == 0 {
                return None;
            }
        }
        self.pending[i] = false;
        if dj.Disjoint.as_bool() || t1 < t0 {
            return None;
        }
        Some(((t1 - t0) as f64 * 1000.0 / dj.Frequency as f64) as f32)
    }
}

/// Компоновщик.
pub struct Compositor {
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    layout: ID3D11InputLayout,
    blend: ID3D11BlendState,
    raster: ID3D11RasterizerState,
    sampler: ID3D11SamplerState,
    constants: ID3D11Buffer,
    vb: Option<(ID3D11Buffer, usize)>,
    canvas: GpuTexture,
    scene_a: Option<GpuTexture>,
    scene_b: Option<GpuTexture>,
    timer: GpuTimer,
    generation: u64,
}

impl Compositor {
    pub fn new(gpu: &GpuDevice, width: u32, height: u32) -> Result<Self> {
        let d = &gpu.device;
        let missing = || GpuError::NoDevice("compositor".into());
        // SAFETY: создание объектов конвейера с корректными описаниями и встроенным байт-кодом.
        unsafe {
            let mut vs = None;
            d.CreateVertexShader(VS, None, Some(&mut vs))?;
            let mut ps = None;
            d.CreatePixelShader(PS, None, Some(&mut ps))?;
            let el = |name, offset| D3D11_INPUT_ELEMENT_DESC {
                SemanticName: name,
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: offset,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            };
            let mut layout = None;
            d.CreateInputLayout(&[el(s!("POSITION"), 0), el(s!("TEXCOORD"), 8)], VS, Some(&mut layout))?;

            let mut bd = D3D11_BLEND_DESC::default();
            bd.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            };
            let mut blend = None;
            d.CreateBlendState(&bd, Some(&mut blend))?;
            let rd = D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_NONE,
                DepthClipEnable: false.into(),
                ..Default::default()
            };
            let mut raster = None;
            d.CreateRasterizerState(&rd, Some(&mut raster))?;
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
            let cbd = D3D11_BUFFER_DESC {
                ByteWidth: size_of::<ItemConstants>() as u32,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                ..Default::default()
            };
            let mut constants = None;
            d.CreateBuffer(&cbd, None, Some(&mut constants))?;

            Ok(Self {
                vs: vs.ok_or_else(missing)?,
                ps: ps.ok_or_else(missing)?,
                layout: layout.ok_or_else(missing)?,
                blend: blend.ok_or_else(missing)?,
                raster: raster.ok_or_else(missing)?,
                sampler: sampler.ok_or_else(missing)?,
                constants: constants.ok_or_else(missing)?,
                vb: None,
                canvas: gpu.create_texture(width, height, DXGI_FORMAT_R8G8B8A8_UNORM, None, true)?,
                // Текстуры перехода создаются заранее: выделение 2×4K в момент перехода давало рывок.
                scene_a: Some(gpu.create_texture(width, height, DXGI_FORMAT_R8G8B8A8_UNORM, None, true)?),
                scene_b: Some(gpu.create_texture(width, height, DXGI_FORMAT_R8G8B8A8_UNORM, None, true)?),
                timer: GpuTimer::new(d)?,
                generation: 1,
            })
        }
    }

    /// Текстура холста (превью и вход конвертера цвета в M2).
    pub fn canvas(&self) -> &GpuTexture {
        &self.canvas
    }

    /// Номер поколения холста: меняется при пересоздании (смена разрешения).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn size(&self) -> (u32, u32) {
        (self.canvas.width, self.canvas.height)
    }

    /// Меняет разрешение холста.
    pub fn resize(&mut self, gpu: &GpuDevice, width: u32, height: u32) -> Result<()> {
        if (width, height) == self.size() {
            return Ok(());
        }
        self.canvas = gpu.create_texture(width, height, DXGI_FORMAT_R8G8B8A8_UNORM, None, true)?;
        self.scene_a = Some(gpu.create_texture(width, height, DXGI_FORMAT_R8G8B8A8_UNORM, None, true)?);
        self.scene_b = Some(gpu.create_texture(width, height, DXGI_FORMAT_R8G8B8A8_UNORM, None, true)?);
        self.generation += 1;
        Ok(())
    }

    fn scratch(gpu: &GpuDevice, slot: &mut Option<GpuTexture>, w: u32, h: u32) -> Result<GpuTexture> {
        if let Some(t) = slot
            && (t.width, t.height) == (w, h)
        {
            return Ok(t.clone());
        }
        let t = gpu.create_texture(w, h, DXGI_FORMAT_R8G8B8A8_UNORM, None, true)?;
        *slot = Some(t.clone());
        Ok(t)
    }

    /// Компонует кадр. `previous` — старая сцена и прогресс перехода `t` (0 → 1).
    pub fn compose(
        &mut self,
        gpu: &GpuDevice,
        current: &[DrawItem],
        previous: Option<(&[DrawItem], f32)>,
    ) -> Result<()> {
        let ctx = &gpu.context;
        self.timer.begin(ctx);
        let (w, h) = self.size();
        let canvas_rtv = self.canvas.rtv.clone().ok_or(GpuError::DeviceLost)?;
        match previous {
            None => self.draw(gpu, &canvas_rtv, current)?,
            Some((old, t)) => {
                let a = Self::scratch(gpu, &mut self.scene_a, w, h)?;
                let b = Self::scratch(gpu, &mut self.scene_b, w, h)?;
                let (art, brt) =
                    (a.rtv.clone().ok_or(GpuError::DeviceLost)?, b.rtv.clone().ok_or(GpuError::DeviceLost)?);
                self.draw(gpu, &art, old)?;
                self.draw(gpu, &brt, current)?;
                let full = |tex: &GpuTexture, opacity: f32| DrawItem {
                    source: DrawSource::Texture(tex.srv.clone()),
                    rect: [0.0, 0.0, w as f32, h as f32],
                    uv: [0.0, 0.0, 1.0, 1.0],
                    rotation: 0,
                    opacity,
                };
                self.draw(gpu, &canvas_rtv, &[full(&a, 1.0), full(&b, t.clamp(0.0, 1.0))])?;
            }
        }
        self.timer.end(ctx);
        // Отвязываем холст от выхода, чтобы UI мог читать его как текстуру.
        // SAFETY: сброс привязок контекста.
        unsafe { ctx.OMSetRenderTargets(None, None) };
        Ok(())
    }

    /// Последнее измеренное время компоновки на GPU, мс (с задержкой в несколько кадров).
    pub fn poll_gpu_time(&mut self, gpu: &GpuDevice) -> Option<f32> {
        self.timer.poll(&gpu.context)
    }

    fn draw(&mut self, gpu: &GpuDevice, rtv: &ID3D11RenderTargetView, items: &[DrawItem]) -> Result<()> {
        let ctx = &gpu.context;
        let (cw, ch) = (self.canvas.width as f32, self.canvas.height as f32);
        // SAFETY: RTV принадлежит этому устройству.
        unsafe { ctx.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0]) };
        if items.is_empty() {
            return Ok(());
        }
        let verts: Vec<Vertex> = items.iter().flat_map(|it| quad(it, cw, ch)).collect();
        let bytes = verts.len() * size_of::<Vertex>();
        let vb = match &self.vb {
            Some((b, cap)) if *cap >= bytes => b.clone(),
            _ => {
                let cap = bytes.next_power_of_two().max(4096);
                let desc = D3D11_BUFFER_DESC {
                    ByteWidth: cap as u32,
                    Usage: D3D11_USAGE_DYNAMIC,
                    BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
                    CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                    ..Default::default()
                };
                let mut b = None;
                // SAFETY: корректное описание буфера.
                unsafe { gpu.device.CreateBuffer(&desc, None, Some(&mut b))? };
                let b = b.ok_or_else(|| GpuError::NoDevice("vb".into()))?;
                self.vb = Some((b.clone(), cap));
                b
            }
        };
        // SAFETY: буфер вмещает все вершины; Map/Unmap парные; привязки — объекты этого устройства.
        unsafe {
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            ctx.Map(&vb, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut m))?;
            std::ptr::copy_nonoverlapping(verts.as_ptr(), m.pData.cast::<Vertex>(), verts.len());
            ctx.Unmap(&vb, 0);

            ctx.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            ctx.OMSetBlendState(&self.blend, Some(&[0.0; 4]), u32::MAX);
            ctx.RSSetState(&self.raster);
            ctx.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: cw,
                Height: ch,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            ctx.IASetInputLayout(&self.layout);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            let stride = size_of::<Vertex>() as u32;
            let offset = 0u32;
            ctx.IASetVertexBuffers(0, 1, Some(&Some(vb.clone())), Some(&stride), Some(&offset));
            ctx.VSSetShader(&self.vs, None);
            ctx.PSSetShader(&self.ps, None);
            ctx.PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            ctx.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));

            for (i, it) in items.iter().enumerate() {
                let op = it.opacity.clamp(0.0, 1.0);
                let c = match &it.source {
                    DrawSource::Texture(srv) => {
                        ctx.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
                        ItemConstants { color: [op; 4], textured: 1.0, _pad: [0.0; 3] }
                    }
                    DrawSource::Color(rgba) => ItemConstants {
                        color: [rgba[0] * op, rgba[1] * op, rgba[2] * op, rgba[3] * op],
                        textured: 0.0,
                        _pad: [0.0; 3],
                    },
                };
                ctx.Map(&self.constants, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut m))?;
                std::ptr::copy_nonoverlapping(&raw const c, m.pData.cast::<ItemConstants>(), 1);
                ctx.Unmap(&self.constants, 0);
                ctx.Draw(4, (i * 4) as u32);
            }
            ctx.PSSetShaderResources(0, Some(&[None]));
        }
        Ok(())
    }
}

/// Четыре вершины (полоса TL, TR, BL, BR) в NDC с UV, учитывающими обрезку и поворот.
fn quad(it: &DrawItem, cw: f32, ch: f32) -> [Vertex; 4] {
    let [x, y, w, h] = it.rect;
    let ndc = |px: f32, py: f32| [px / cw * 2.0 - 1.0, 1.0 - py / ch * 2.0];
    let [u0, v0, u1, v1] = it.uv;
    // Углы источника: TL, TR, BR, BL.
    let src = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
    // Поворот по часовой: угол назначения k берёт угол источника (k + 4 - r) % 4.
    let r = usize::from(it.rotation % 4);
    let uv = |k: usize| src[(k + 4 - r) % 4];
    [
        Vertex { pos: ndc(x, y), uv: uv(0) },
        Vertex { pos: ndc(x + w, y), uv: uv(1) },
        Vertex { pos: ndc(x, y + h), uv: uv(3) },
        Vertex { pos: ndc(x + w, y + h), uv: uv(2) },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(rotation: u8) -> DrawItem {
        DrawItem {
            source: DrawSource::Color([1.0; 4]),
            rect: [0.0, 0.0, 10.0, 10.0],
            uv: [0.0, 0.0, 1.0, 1.0],
            rotation,
            opacity: 1.0,
        }
    }

    #[test]
    fn rotation_uv_mapping() {
        // Без поворота: левый верх назначения = левый верх источника.
        assert_eq!(quad(&item(0), 10.0, 10.0)[0].uv, [0.0, 0.0]);
        // 90° по часовой: в левом верху оказывается левый низ источника.
        assert_eq!(quad(&item(1), 10.0, 10.0)[0].uv, [0.0, 1.0]);
        // 180°: правый низ.
        assert_eq!(quad(&item(2), 10.0, 10.0)[0].uv, [1.0, 1.0]);
        // 270°: правый верх.
        assert_eq!(quad(&item(3), 10.0, 10.0)[0].uv, [1.0, 0.0]);
    }

    /// Компоновка на WARP: красный прямоугольник поверх чёрного фона и смешивание при переходе.
    #[test]
    fn composes_on_warp() {
        let gpu = GpuDevice::new(crate::DriverKind::Warp).expect("WARP");
        let _g = gpu.lock();
        let mut c = Compositor::new(&gpu, 64, 64).expect("compositor");
        let red = DrawItem {
            source: DrawSource::Color([1.0, 0.0, 0.0, 1.0]),
            rect: [16.0, 16.0, 32.0, 32.0],
            uv: [0.0, 0.0, 1.0, 1.0],
            rotation: 0,
            opacity: 1.0,
        };
        c.compose(&gpu, std::slice::from_ref(&red), None).expect("compose");
        let px = gpu.read_pixels(c.canvas()).expect("read");
        let at = |x: usize, y: usize| &px[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(at(32, 32), &[255, 0, 0, 255]);
        assert_eq!(at(4, 4), &[0, 0, 0, 255]);
        assert_eq!(at(47, 47), &[255, 0, 0, 255]);
        assert_eq!(at(48, 48), &[0, 0, 0, 255]);

        // Переход: старая сцена (красная), новая (пустая) на середине → половина яркости.
        c.compose(&gpu, &[], Some((&[red], 0.5))).expect("fade");
        let px = gpu.read_pixels(c.canvas()).expect("read");
        let r = px[(32 * 64 + 32) * 4];
        assert!((126..=129).contains(&r), "r = {r}");
    }

    #[test]
    fn ndc_corners() {
        let q = quad(&item(0), 10.0, 10.0);
        assert_eq!(q[0].pos, [-1.0, 1.0]);
        assert_eq!(q[3].pos, [1.0, -1.0]);
    }
}
