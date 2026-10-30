//! Единое D3D11-устройство.

use crate::{GpuError, Result};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_P010, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{DXGI_ADAPTER_DESC1, IDXGIAdapter1, IDXGIDevice};
use windows::core::Interface;

/// Тип драйвера.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverKind {
    /// Аппаратный GPU.
    Hardware,
    /// Программный растеризатор WARP (CI, скриншот-тесты, машины без GPU).
    Warp,
}

/// Описание адаптера для лога и диагностики (§14.2).
#[derive(Debug, Clone)]
pub struct AdapterInfo {
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_video_mb: u64,
    pub feature_level: u32,
}

impl AdapterInfo {
    /// Человекочитаемое имя производителя по PCI vendor id.
    pub fn vendor(&self) -> &'static str {
        match self.vendor_id {
            0x10DE => "NVIDIA",
            0x1002 | 0x1022 => "AMD",
            0x8086 => "Intel",
            0x1414 => "Microsoft",
            _ => "unknown",
        }
    }
}

/// D3D11-устройство и его immediate-контекст.
///
/// Устройство общее для UI-потока, рендер-потока и захвата (§5.2 п.1). Immediate-контекст
/// не потокобезопасен, поэтому любая последовательность команд выполняется под [`GpuDevice::lock`];
/// дополнительно включена встроенная защита D3D11 (`SetMultithreadProtected`) — её требует
/// Windows.Graphics.Capture, которая обращается к устройству из своих потоков (ADR-0003).
pub struct GpuDevice {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub kind: DriverKind,
    pub adapter: AdapterInfo,
    lock: parking_lot::Mutex<()>,
}

// SAFETY: ID3D11Device свободно-поточный; immediate-контекст используется только под `lock`
// (и под встроенной защитой D3D11), поэтому разделение между потоками безопасно.
unsafe impl Sync for GpuDevice {}

/// Текстура на GPU с представлением для шейдеров.
#[derive(Clone)]
pub struct GpuTexture {
    pub texture: ID3D11Texture2D,
    pub srv: ID3D11ShaderResourceView,
    pub rtv: Option<ID3D11RenderTargetView>,
    pub width: u32,
    pub height: u32,
}

impl GpuDevice {
    /// Создаёт устройство: аппаратное (FL 11_1/11_0), при неудаче — WARP.
    pub fn new(prefer: DriverKind) -> Result<Self> {
        let order: &[DriverKind] = match prefer {
            DriverKind::Hardware => &[DriverKind::Hardware, DriverKind::Warp],
            DriverKind::Warp => &[DriverKind::Warp],
        };
        let mut last_err = String::new();
        for &kind in order {
            let driver: D3D_DRIVER_TYPE = match kind {
                DriverKind::Hardware => D3D_DRIVER_TYPE_HARDWARE,
                DriverKind::Warp => D3D_DRIVER_TYPE_WARP,
            };
            match create(driver) {
                Ok((device, context, fl)) => {
                    let adapter = adapter_info(&device, fl).unwrap_or(AdapterInfo {
                        name: "unknown".into(),
                        vendor_id: 0,
                        device_id: 0,
                        dedicated_video_mb: 0,
                        feature_level: fl,
                    });
                    tracing::info!(
                        "D3D11: {} ({}), VRAM {} МБ, FL {:x}, драйвер {:?}",
                        adapter.name,
                        adapter.vendor(),
                        adapter.dedicated_video_mb,
                        adapter.feature_level,
                        kind
                    );
                    // SAFETY: запрос интерфейса у живого контекста; ошибка — не критична (старые драйверы).
                    if let Ok(mt) = context.cast::<ID3D11Multithread>() {
                        unsafe {
                            let _ = mt.SetMultithreadProtected(true);
                        }
                    }
                    return Ok(Self { device, context, kind, adapter, lock: parking_lot::Mutex::new(()) });
                }
                Err(e) => {
                    tracing::warn!("D3D11CreateDevice({kind:?}) не удалось: {e}");
                    last_err = e.to_string();
                }
            }
        }
        Err(GpuError::NoDevice(last_err))
    }

    /// Чтение текстуры RGBA8/BGRA8 в память — только для тестов и диагностики:
    /// в основном пути копирование GPU→CPU запрещено (§4).
    pub fn read_pixels(&self, tex: &GpuTexture) -> Result<Vec<u8>> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: staging-копия той же текстуры; Map/Unmap парные; строки читаются в пределах RowPitch.
        unsafe {
            tex.texture.GetDesc(&mut desc);
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            let mut staging = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut staging))?;
            let staging = staging.ok_or_else(|| GpuError::NoDevice("staging".into()))?;
            self.context.CopyResource(&staging, &tex.texture);
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
            let row = tex.width as usize * 4;
            let mut out = vec![0u8; row * tex.height as usize];
            for y in 0..tex.height as usize {
                let src = std::slice::from_raw_parts(m.pData.cast::<u8>().add(y * m.RowPitch as usize), row);
                out[y * row..(y + 1) * row].copy_from_slice(src);
            }
            self.context.Unmap(&staging, 0);
            Ok(out)
        }
    }

    /// Чтение NV12-текстуры (Y, затем UV) — только для тестов (§4).
    pub fn read_nv12(&self, tex: &ID3D11Texture2D) -> Result<Vec<u8>> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: staging-копия; плоскость UV следует за Y через RowPitch × Height.
        unsafe {
            tex.GetDesc(&mut desc);
            let (w, h) = (desc.Width as usize, desc.Height as usize);
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            let mut staging = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut staging))?;
            let staging = staging.ok_or_else(|| GpuError::NoDevice("staging".into()))?;
            self.context.CopyResource(&staging, tex);
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
            let pitch = m.RowPitch as usize;
            let base = m.pData.cast::<u8>();
            // P010 — 2 байта на отсчёт (little-endian), NV12 — 1.
            let bytes = if desc.Format == DXGI_FORMAT_P010 { 2 } else { 1 };
            let mut out = Vec::with_capacity(w * h * 3 / 2 * bytes);
            for y in 0..h + h / 2 {
                out.extend_from_slice(std::slice::from_raw_parts(base.add(y * pitch), w * bytes));
            }
            self.context.Unmap(&staging, 0);
            Ok(out)
        }
    }

    /// Захват immediate-контекста на время последовательности команд.
    pub fn lock(&self) -> parking_lot::MutexGuard<'_, ()> {
        self.lock.lock()
    }

    /// Создаёт 2D-текстуру. `data` — начальные пиксели (строки по `width * 4` байт).
    pub fn create_texture(
        &self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        data: Option<&[u8]>,
        render_target: bool,
    ) -> Result<GpuTexture> {
        let mut bind = D3D11_BIND_SHADER_RESOURCE.0 as u32;
        if render_target {
            bind |= D3D11_BIND_RENDER_TARGET.0 as u32;
        }
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width.max(1),
            Height: height.max(1),
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: bind,
            ..Default::default()
        };
        let init = data.map(|d| D3D11_SUBRESOURCE_DATA {
            pSysMem: d.as_ptr().cast(),
            SysMemPitch: width * 4,
            SysMemSlicePitch: 0,
        });
        let missing = || GpuError::NoDevice("texture".into());
        // SAFETY: описание корректно; если переданы данные, их размер — width*height*4 (проверено ниже).
        unsafe {
            if let Some(d) = data
                && d.len() < (width * height * 4) as usize
            {
                return Err(GpuError::NoDevice("недостаточно данных для текстуры".into()));
            }
            let mut tex = None;
            self.device.CreateTexture2D(&desc, init.as_ref().map(|i| i as *const _), Some(&mut tex))?;
            let texture = tex.ok_or_else(missing)?;
            let mut srv = None;
            self.device.CreateShaderResourceView(&texture, None, Some(&mut srv))?;
            let rtv = if render_target {
                let mut r = None;
                self.device.CreateRenderTargetView(&texture, None, Some(&mut r))?;
                r
            } else {
                None
            };
            Ok(GpuTexture { texture, srv: srv.ok_or_else(missing)?, rtv, width: width.max(1), height: height.max(1) })
        }
    }
}

fn create(driver: D3D_DRIVER_TYPE) -> windows::core::Result<(ID3D11Device, ID3D11DeviceContext, u32)> {
    let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
    let flags: D3D11_CREATE_DEVICE_FLAG = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
    let mut device = None;
    let mut context = None;
    let mut fl = D3D_FEATURE_LEVEL::default();
    // SAFETY: все выходные указатели — на локальные Option/значения, живущие
    // во время вызова; массив уровней валиден.
    unsafe {
        D3D11CreateDevice(
            None,
            driver,
            HMODULE::default(),
            flags,
            Some(&levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut fl),
            Some(&mut context),
        )?;
    }
    match (device, context) {
        (Some(d), Some(c)) => Ok((d, c, fl.0 as u32)),
        _ => Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL)),
    }
}

fn adapter_info(device: &ID3D11Device, fl: u32) -> windows::core::Result<AdapterInfo> {
    let dxgi: IDXGIDevice = device.cast()?;
    // SAFETY: COM-вызовы на валидных интерфейсах; DXGI_ADAPTER_DESC1 заполняется целиком.
    unsafe {
        let adapter: IDXGIAdapter1 = dxgi.GetAdapter()?.cast()?;
        let desc: DXGI_ADAPTER_DESC1 = adapter.GetDesc1()?;
        let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
        Ok(AdapterInfo {
            name: String::from_utf16_lossy(&desc.Description[..len]),
            vendor_id: desc.VendorId,
            device_id: desc.DeviceId,
            dedicated_video_mb: desc.DedicatedVideoMemory as u64 / 1_048_576,
            feature_level: fl,
        })
    }
}
