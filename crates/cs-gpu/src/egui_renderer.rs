//! Рендерер egui поверх D3D11 (ADR-0002).
//!
//! ~300 строк вместо внешнего крейта: полный контроль, нет лишних зависимостей,
//! и возможность показывать GPU-текстуры (превью сцены) напрямую через
//! [`EguiRenderer::register_native_texture`].

use crate::Result;
use egui::epaint::{ClippedPrimitive, ImageDelta, Primitive, Vertex};
use egui::{TextureFilter, TextureId, TexturesDelta};
use std::collections::HashMap;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R32_UINT, DXGI_FORMAT_R32G32_FLOAT, DXGI_SAMPLE_DESC,
};
use windows::core::s;

const VS_BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/egui_vs.cso"));
const PS_BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/egui_ps.cso"));

struct Texture {
    texture: Option<ID3D11Texture2D>,
    srv: ID3D11ShaderResourceView,
    linear: bool,
    size: [usize; 2],
}

/// Рендерер egui-примитивов в произвольный render target view.
pub struct EguiRenderer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    layout: ID3D11InputLayout,
    blend: ID3D11BlendState,
    raster: ID3D11RasterizerState,
    sampler_linear: ID3D11SamplerState,
    sampler_nearest: ID3D11SamplerState,
    constants: ID3D11Buffer,
    vb: Option<(ID3D11Buffer, usize)>,
    ib: Option<(ID3D11Buffer, usize)>,
    textures: HashMap<TextureId, Texture>,
    next_user_id: u64,
}

impl EguiRenderer {
    pub fn new(device: &ID3D11Device, context: &ID3D11DeviceContext) -> Result<Self> {
        // SAFETY: создание D3D-объектов с корректными описаниями; байт-код шейдеров
        // встроен при сборке и соответствует раскладке входа ниже.
        unsafe {
            let mut vs = None;
            device.CreateVertexShader(VS_BYTECODE, None, Some(&mut vs))?;
            let mut ps = None;
            device.CreatePixelShader(PS_BYTECODE, None, Some(&mut ps))?;

            let elements = [
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: s!("POSITION"),
                    SemanticIndex: 0,
                    Format: DXGI_FORMAT_R32G32_FLOAT,
                    InputSlot: 0,
                    AlignedByteOffset: 0,
                    InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                    InstanceDataStepRate: 0,
                },
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: s!("TEXCOORD"),
                    SemanticIndex: 0,
                    Format: DXGI_FORMAT_R32G32_FLOAT,
                    InputSlot: 0,
                    AlignedByteOffset: 8,
                    InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                    InstanceDataStepRate: 0,
                },
                D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: s!("COLOR"),
                    SemanticIndex: 0,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    InputSlot: 0,
                    AlignedByteOffset: 16,
                    InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                    InstanceDataStepRate: 0,
                },
            ];
            let mut layout = None;
            device.CreateInputLayout(&elements, VS_BYTECODE, Some(&mut layout))?;

            // Premultiplied alpha: src*1 + dst*(1-srcA).
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
            device.CreateBlendState(&bd, Some(&mut blend))?;

            let rd = D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_NONE,
                ScissorEnable: true.into(),
                DepthClipEnable: false.into(),
                ..Default::default()
            };
            let mut raster = None;
            device.CreateRasterizerState(&rd, Some(&mut raster))?;

            let sampler = |filter| -> Result<ID3D11SamplerState> {
                let sd = D3D11_SAMPLER_DESC {
                    Filter: filter,
                    AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                    MaxLOD: f32::MAX,
                    ComparisonFunc: D3D11_COMPARISON_NEVER,
                    ..Default::default()
                };
                let mut s = None;
                device.CreateSamplerState(&sd, Some(&mut s))?;
                s.ok_or_else(|| crate::GpuError::NoDevice("sampler".into()))
            };
            let sampler_linear = sampler(D3D11_FILTER_MIN_MAG_MIP_LINEAR)?;
            let sampler_nearest = sampler(D3D11_FILTER_MIN_MAG_MIP_POINT)?;

            let cbd = D3D11_BUFFER_DESC {
                ByteWidth: 16,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                ..Default::default()
            };
            let mut constants = None;
            device.CreateBuffer(&cbd, None, Some(&mut constants))?;

            let missing = || crate::GpuError::NoDevice("egui pipeline".into());
            Ok(Self {
                device: device.clone(),
                context: context.clone(),
                vs: vs.ok_or_else(missing)?,
                ps: ps.ok_or_else(missing)?,
                layout: layout.ok_or_else(missing)?,
                blend: blend.ok_or_else(missing)?,
                raster: raster.ok_or_else(missing)?,
                sampler_linear,
                sampler_nearest,
                constants: constants.ok_or_else(missing)?,
                vb: None,
                ib: None,
                textures: HashMap::new(),
                next_user_id: 0,
            })
        }
    }

    /// Регистрирует внешнюю GPU-текстуру (например, превью сцены) для `egui::Image`.
    pub fn register_native_texture(&mut self, srv: ID3D11ShaderResourceView, size: [usize; 2]) -> TextureId {
        let id = TextureId::User(self.next_user_id);
        self.next_user_id += 1;
        self.textures.insert(id, Texture { texture: None, srv, linear: true, size });
        id
    }

    /// Применяет изменения текстур до отрисовки кадра (забирает `delta.set`).
    pub fn update_textures(&mut self, delta: &mut TexturesDelta) -> Result<()> {
        for (id, deltas) in std::mem::take(&mut delta.set) {
            for d in &deltas {
                self.set_texture(id, d)?;
            }
        }
        Ok(())
    }

    /// Освобождает текстуры после отрисовки кадра (забирает `delta.free`).
    pub fn free_textures(&mut self, delta: &mut TexturesDelta) {
        for id in std::mem::take(&mut delta.free) {
            self.textures.remove(&id);
        }
    }

    fn set_texture(&mut self, id: TextureId, delta: &ImageDelta) -> Result<()> {
        let egui::ImageData::Color(image) = &delta.image;
        let [w, h] = image.size;
        if w == 0 || h == 0 {
            return Ok(());
        }
        let linear = delta.options.magnification == TextureFilter::Linear;
        let pixels: &[egui::Color32] = &image.pixels;
        // SAFETY: Color32 — repr(C) [u8; 4] в порядке RGBA, совпадает с R8G8B8A8_UNORM;
        // размеры буфера соответствуют w*h*4.
        unsafe {
            if let Some([x, y]) = delta.pos {
                let Some(tex) = self.textures.get(&id).and_then(|t| t.texture.clone()) else {
                    tracing::warn!("частичное обновление несуществующей текстуры {id:?}");
                    return Ok(());
                };
                let b = D3D11_BOX {
                    left: x as u32,
                    top: y as u32,
                    front: 0,
                    right: (x + w) as u32,
                    bottom: (y + h) as u32,
                    back: 1,
                };
                self.context.UpdateSubresource(&tex, 0, Some(&b), pixels.as_ptr().cast(), (w * 4) as u32, 0);
            } else {
                let desc = D3D11_TEXTURE2D_DESC {
                    Width: w as u32,
                    Height: h as u32,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                    ..Default::default()
                };
                let init = D3D11_SUBRESOURCE_DATA {
                    pSysMem: pixels.as_ptr().cast(),
                    SysMemPitch: (w * 4) as u32,
                    SysMemSlicePitch: 0,
                };
                let mut tex = None;
                self.device.CreateTexture2D(&desc, Some(&init), Some(&mut tex))?;
                let tex = tex.ok_or_else(|| crate::GpuError::NoDevice("texture".into()))?;
                let mut srv = None;
                self.device.CreateShaderResourceView(&tex, None, Some(&mut srv))?;
                let srv = srv.ok_or_else(|| crate::GpuError::NoDevice("srv".into()))?;
                self.textures.insert(id, Texture { texture: Some(tex), srv, linear, size: [w, h] });
            }
        }
        Ok(())
    }

    /// Размер зарегистрированной текстуры.
    pub fn texture_size(&self, id: TextureId) -> Option<[usize; 2]> {
        self.textures.get(&id).map(|t| t.size)
    }

    fn ensure_buffer(
        device: &ID3D11Device,
        slot: &mut Option<(ID3D11Buffer, usize)>,
        bytes: usize,
        bind: D3D11_BIND_FLAG,
    ) -> Result<ID3D11Buffer> {
        if let Some((b, cap)) = slot
            && *cap >= bytes
        {
            return Ok(b.clone());
        }
        let cap = bytes.next_power_of_two().max(64 * 1024);
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: cap as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: bind.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            ..Default::default()
        };
        let mut b = None;
        // SAFETY: корректное описание буфера.
        unsafe { device.CreateBuffer(&desc, None, Some(&mut b))? };
        let b = b.ok_or_else(|| crate::GpuError::NoDevice("buffer".into()))?;
        *slot = Some((b.clone(), cap));
        Ok(b)
    }

    /// Рисует примитивы в `rtv` размером `size_px` пикселей.
    pub fn render(
        &mut self,
        rtv: &ID3D11RenderTargetView,
        size_px: [u32; 2],
        pixels_per_point: f32,
        primitives: &[ClippedPrimitive],
    ) -> Result<()> {
        let meshes: Vec<(&egui::Rect, &egui::Mesh)> = primitives
            .iter()
            .filter_map(|p| match &p.primitive {
                Primitive::Mesh(m) if !m.indices.is_empty() => Some((&p.clip_rect, m)),
                _ => None,
            })
            .collect();
        let vcount: usize = meshes.iter().map(|(_, m)| m.vertices.len()).sum();
        let icount: usize = meshes.iter().map(|(_, m)| m.indices.len()).sum();
        if vcount == 0 {
            return Ok(());
        }
        let stride = std::mem::size_of::<Vertex>();
        let vb = Self::ensure_buffer(&self.device, &mut self.vb, vcount * stride, D3D11_BIND_VERTEX_BUFFER)?;
        let ib = Self::ensure_buffer(&self.device, &mut self.ib, icount * 4, D3D11_BIND_INDEX_BUFFER)?;
        let ctx = &self.context;

        // SAFETY: буферы созданы с ёмкостью не меньше записываемых данных;
        // Map/Unmap парные; Vertex — repr(C) (pos, uv, color) = 20 байт, совпадает с input layout.
        unsafe {
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            ctx.Map(&vb, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))?;
            let mut dst = mapped.pData.cast::<Vertex>();
            for (_, m) in &meshes {
                std::ptr::copy_nonoverlapping(m.vertices.as_ptr(), dst, m.vertices.len());
                dst = dst.add(m.vertices.len());
            }
            ctx.Unmap(&vb, 0);

            ctx.Map(&ib, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))?;
            let mut dst = mapped.pData.cast::<u32>();
            for (_, m) in &meshes {
                std::ptr::copy_nonoverlapping(m.indices.as_ptr(), dst, m.indices.len());
                dst = dst.add(m.indices.len());
            }
            ctx.Unmap(&ib, 0);

            let screen = [size_px[0] as f32 / pixels_per_point, size_px[1] as f32 / pixels_per_point, 0.0, 0.0];
            ctx.Map(&self.constants, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))?;
            std::ptr::copy_nonoverlapping(screen.as_ptr(), mapped.pData.cast::<f32>(), 4);
            ctx.Unmap(&self.constants, 0);

            ctx.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            ctx.OMSetBlendState(&self.blend, Some(&[0.0; 4]), u32::MAX);
            ctx.RSSetState(&self.raster);
            ctx.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: size_px[0] as f32,
                Height: size_px[1] as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            ctx.IASetInputLayout(&self.layout);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            let stride32 = stride as u32;
            let offset = 0u32;
            ctx.IASetVertexBuffers(0, 1, Some(&Some(vb.clone())), Some(&stride32), Some(&offset));
            ctx.IASetIndexBuffer(&ib, DXGI_FORMAT_R32_UINT, 0);
            ctx.VSSetShader(&self.vs, None);
            ctx.VSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            ctx.PSSetShader(&self.ps, None);

            let (mut vbase, mut ibase) = (0i32, 0u32);
            for (clip, m) in &meshes {
                let n = m.indices.len() as u32;
                let nv = m.vertices.len() as i32;
                let x0 = (clip.min.x * pixels_per_point).round().clamp(0.0, size_px[0] as f32) as i32;
                let y0 = (clip.min.y * pixels_per_point).round().clamp(0.0, size_px[1] as f32) as i32;
                let x1 = (clip.max.x * pixels_per_point).round().clamp(0.0, size_px[0] as f32) as i32;
                let y1 = (clip.max.y * pixels_per_point).round().clamp(0.0, size_px[1] as f32) as i32;
                if x1 > x0
                    && y1 > y0
                    && let Some(t) = self.textures.get(&m.texture_id)
                {
                    ctx.RSSetScissorRects(Some(&[RECT { left: x0, top: y0, right: x1, bottom: y1 }]));
                    ctx.PSSetShaderResources(0, Some(&[Some(t.srv.clone())]));
                    let s = if t.linear { &self.sampler_linear } else { &self.sampler_nearest };
                    ctx.PSSetSamplers(0, Some(&[Some(s.clone())]));
                    ctx.DrawIndexed(n, ibase, vbase);
                }
                ibase += n;
                vbase += nv;
            }
        }
        Ok(())
    }
}
