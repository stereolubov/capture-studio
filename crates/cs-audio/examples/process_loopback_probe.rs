//! Пробник захвата звука процесса (process loopback, Windows 10 2004+).
//!
//! `cargo run -p cs-audio --example process_loopback_probe -- <pid> [секунд]`
//! Печатает пакеты, их метки QPC относительно «сейчас» и пиковый уровень.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Media::Audio::*;
    use windows::Win32::Media::Multimedia::WAVE_FORMAT_IEEE_FLOAT;
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

    let mut args = std::env::args().skip(1);
    let pid: u32 = args.next().and_then(|a| a.parse().ok()).expect("pid");
    let secs: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(5);

    // SAFETY: COM-вызовы WASAPI.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let client = cs_audio::wasapi::process_loopback_client(pid)?;

        let fmt = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_IEEE_FLOAT as u16,
            nChannels: 2,
            nSamplesPerSec: 48_000,
            nAvgBytesPerSec: 48_000 * 8,
            nBlockAlign: 8,
            wBitsPerSample: 32,
            cbSize: 0,
        };
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
            1_000_000,
            0,
            &fmt,
            None,
        )?;
        let capture: IAudioCaptureClient = client.GetService()?;
        client.Start()?;
        let freq = {
            let mut f = 0i64;
            let _ = windows::Win32::System::Performance::QueryPerformanceFrequency(&mut f);
            f
        };
        let now_100ns = || {
            let mut c = 0i64;
            let _ = windows::Win32::System::Performance::QueryPerformanceCounter(&mut c);
            (i128::from(c) * 10_000_000 / i128::from(freq)) as i64
        };
        let end = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        let (mut packets, mut total, mut peak, mut zero_qpc) = (0u32, 0u64, 0f32, 0u32);
        while std::time::Instant::now() < end {
            while capture.GetNextPacketSize()? > 0 {
                let (mut data, mut frames, mut flags, mut qpc) = (std::ptr::null_mut(), 0u32, 0u32, 0u64);
                capture.GetBuffer(&mut data, &mut frames, &mut flags, None, Some(&mut qpc))?;
                if !data.is_null() && flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 == 0 {
                    let s = std::slice::from_raw_parts(data as *const f32, frames as usize * 2);
                    peak = s.iter().fold(peak, |m, v| m.max(v.abs()));
                }
                if qpc == 0 {
                    zero_qpc += 1;
                }
                if packets < 5 || packets % 200 == 0 {
                    println!(
                        "пакет {packets}: {frames} кадров, флаги {flags:#x}, метка {:.1} мс назад",
                        (now_100ns() - qpc as i64) as f64 / 10_000.0
                    );
                }
                capture.ReleaseBuffer(frames)?;
                packets += 1;
                total += u64::from(frames);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        println!(
            "итого: пакетов {packets}, кадров {total} ({:.2} с), без метки {zero_qpc}, пик {:.1} dBFS",
            total as f64 / 48_000.0,
            20.0 * peak.max(1e-9).log10()
        );
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
