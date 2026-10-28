//! Общие помощники Media Foundation.

use std::sync::Once;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::System::Variant::{VARENUM, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_UI4};

/// Инициализация COM (MTA) и Media Foundation для текущего потока / процесса.
pub fn startup() {
    static MF: Once = Once::new();
    // SAFETY: COM для текущего потока; MFStartup — один раз на процесс (MFShutdown не вызывается).
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MF.call_once(|| {
            if let Err(e) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
                tracing::error!("MFStartup: {e}");
            }
        });
    }
}

/// Упаковка пары u32 в атрибут UINT64 (размер кадра, частота кадров).
pub fn pack(hi: u32, lo: u32) -> u64 {
    (u64::from(hi) << 32) | u64::from(lo)
}

fn variant(vt: VARENUM, value: VARIANT_0_0_0) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: value,
            }),
        },
    }
}

pub fn variant_u32(v: u32) -> VARIANT {
    variant(VT_UI4, VARIANT_0_0_0 { ulVal: v })
}

pub fn variant_bool(v: bool) -> VARIANT {
    variant(VT_BOOL, VARIANT_0_0_0 { boolVal: windows::Win32::Foundation::VARIANT_BOOL(if v { -1 } else { 0 }) })
}

/// Байты буфера сэмпла.
///
/// # Safety
/// `sample` — живой сэмпл MF.
pub unsafe fn sample_bytes(sample: &IMFSample) -> windows::core::Result<Vec<u8>> {
    // SAFETY: буфер заблокирован между Lock/Unlock, длина — из Lock.
    unsafe {
        let buf = sample.ConvertToContiguousBuffer()?;
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        buf.Lock(&mut ptr, None, Some(&mut len))?;
        let out = std::slice::from_raw_parts(ptr, len as usize).to_vec();
        buf.Unlock()?;
        Ok(out)
    }
}
