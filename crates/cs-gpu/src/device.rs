//! Единое D3D11-устройство.

use crate::{GpuError, Result};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    ID3D11DeviceContext,
};
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
/// Immediate-контекст D3D11 не потокобезопасен: в M0 используется только из UI-потока.
/// В M1 рендер-поток получит собственный доступ через синхронизацию (ADR-0003).
pub struct GpuDevice {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub kind: DriverKind,
    pub adapter: AdapterInfo,
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
                    return Ok(Self { device, context, kind, adapter });
                }
                Err(e) => {
                    tracing::warn!("D3D11CreateDevice({kind:?}) не удалось: {e}");
                    last_err = e.to_string();
                }
            }
        }
        Err(GpuError::NoDevice(last_err))
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
