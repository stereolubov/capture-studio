use cs_core::clock::Ticks;
use cs_gpu::{DriverKind, GpuDevice};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D11::{ID3D11RenderTargetView, ID3D11Texture2D};
use windows::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmGetCompositionTimingInfo};
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{Interface, w};

const CLICK_SECS: f64 = 0.05;

/// Событие генератора (секунды от старта).
#[derive(Default, Clone, Copy)]
struct Event {
    audio: f64,
    video_target: f64,
    video_actual: Option<f64>,
}

struct Shared {
    /// Моменты щелчков (тики QPC), добавляет видеопоток.
    clicks: Mutex<Vec<i64>>,
    stop: AtomicBool,
}

fn secs(t: i64, start: i64) -> f64 {
    (t - start) as f64 / Ticks::frequency() as f64
}

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: стандартная обработка сообщений окна.
    unsafe { DefWindowProcW(h, m, w, l) }
}

/// Аудиопоток: щелчок 1 кГц в моменты `clicks`, время сэмплов — по `IAudioClock`.
fn audio_thread(shared: Arc<Shared>) -> windows::core::Result<()> {
    // SAFETY: последовательность WASAPI shared mode; буфер заполняется в пределах GetBuffer.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let dev = en.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let client: IAudioClient = dev.Activate(CLSCTX_ALL, None)?;
        let fmt = client.GetMixFormat()?;
        let rate = (*fmt).nSamplesPerSec;
        let channels = usize::from((*fmt).nChannels);
        client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, 200_000, 0, fmt, None)?;
        CoTaskMemFree(Some(fmt as *const _));
        let event = CreateEventW(None, false, false, None)?;
        client.SetEventHandle(event)?;
        let render: IAudioRenderClient = client.GetService()?;
        let clock: IAudioClock = client.GetService()?;
        let clock_freq = clock.GetFrequency()?;
        let buffer = client.GetBufferSize()?;
        client.Start()?;
        let freq = Ticks::frequency();
        let mut written: u64 = 0;
        while !shared.stop.load(Ordering::Acquire) {
            WaitForSingleObject(event, 200);
            let padding = client.GetCurrentPadding()?;
            let n = buffer - padding;
            if n == 0 {
                continue;
            }
            // Сопоставление: позиция `pos` воспроизводилась в момент `qpc` (100 нс).
            let (mut pos, mut qpc) = (0u64, 0u64);
            clock.GetPosition(&mut pos, Some(&mut qpc))?;
            let played_frames = pos as f64 * f64::from(rate) / clock_freq as f64;
            let qpc_ticks = qpc as f64 * freq as f64 / 10_000_000.0;
            let first = qpc_ticks + (written as f64 - played_frames) / f64::from(rate) * freq as f64;
            let clicks = shared.clicks.lock().map(|c| c.clone()).unwrap_or_default();
            let data = render.GetBuffer(n)?.cast::<f32>();
            for i in 0..n as usize {
                let t = first + i as f64 / f64::from(rate) * freq as f64;
                let v = clicks
                    .iter()
                    .find(|&&c| t >= c as f64 && t < c as f64 + CLICK_SECS * freq as f64)
                    .map_or(0.0, |&c| {
                        (2.0 * std::f64::consts::PI * 1000.0 * (t - c as f64) / freq as f64).sin() as f32 * 0.8
                    });
                for ch in 0..channels {
                    *data.add(i * channels + ch) = v;
                }
            }
            render.ReleaseBuffer(n, 0)?;
            written += u64::from(n);
        }
        client.Stop()?;
    }
    Ok(())
}

pub fn run() -> anyhow_lite::Result<()> {
    let seconds: f64 = arg("--seconds").and_then(|s| s.parse().ok()).unwrap_or(30.0);
    let x: i32 = arg("--x").and_then(|s| s.parse().ok()).unwrap_or(200);
    let y: i32 = arg("--y").and_then(|s| s.parse().ok()).unwrap_or(200);
    let out = arg("--out").unwrap_or_else(|| "events.json".into());

    let gpu = GpuDevice::new(DriverKind::Hardware)?;
    // SAFETY: создание окна и swapchain для собственного окна; цикл — в этом потоке.
    let (hwnd, swapchain) = unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None)?.into();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            lpszClassName: w!("SyncGen"),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            w!("SyncGen"),
            w!("syncgen"),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            480,
            270,
            None,
            None,
            Some(hinst),
            None,
        )?;
        let factory: IDXGIFactory2 = gpu.device.cast::<IDXGIDevice>()?.GetAdapter()?.GetParent()?;
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: 480,
            Height: 270,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            ..Default::default()
        };
        (hwnd, factory.CreateSwapChainForHwnd(&gpu.device, hwnd, &desc, None, None)?)
    };

    let shared = Arc::new(Shared { clicks: Mutex::new(Vec::new()), stop: AtomicBool::new(false) });
    let sh = shared.clone();
    let audio = std::thread::spawn(move || audio_thread(sh));
    std::thread::sleep(Duration::from_millis(500));

    let freq = Ticks::frequency();
    let start = Ticks::now().0;
    let mut events: Vec<Event> = Vec::new();
    let mut pending: Vec<(u32, usize)> = Vec::new(); // (номер Present, событие)
    let mut next_target = start + freq; // первое событие через 1 с
    let mut k = 0usize;
    let mut scheduled: Option<(i64, usize)> = None; // (vblank показа, событие)
    let mut lead: i64 = 1;
    let mut flash_until: Option<i64> = None;
    let mut msg = MSG::default();
    while secs(Ticks::now().0, start) < seconds {
        // SAFETY: обработка сообщений окна, запросы таймингов DWM и статистики swapchain.
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                DispatchMessageW(&msg);
            }
            let mut ti = DWM_TIMING_INFO { cbSize: size_of::<DWM_TIMING_INFO>() as u32, ..Default::default() };
            DwmGetCompositionTimingInfo(HWND::default(), &mut ti)?;
            let period = ti.qpcRefreshPeriod as i64;
            let vblank = ti.qpcVBlank as i64;
            let now = Ticks::now().0;
            // Выбор развёртки для следующего события (~3 кадра вперёд) и планирование щелчка.
            if scheduled.is_none() && now >= next_target - 4 * period {
                let mut v = vblank;
                while v < now + 3 * period {
                    v += period;
                }
                shared.clicks.lock().map_err(|_| "mutex")?.push(v);
                events.push(Event { audio: secs(v, start), video_target: secs(v, start), video_actual: None });
                scheduled = Some((v, events.len() - 1));
                k += 1;
                next_target += freq + (k % 4) as i64 * freq / 20;
            }
            // Кадр, поданный сейчас, показывается через `lead` развёрток (окно, flip model);
            // `lead` подстраивается по фактическому времени показа из статистики.
            let display = vblank + period * lead;
            // Первый белый кадр — ровно в выбранную развёртку; дальше вспышка держится 50 мс,
            // чтобы её гарантированно поймала запись на любой частоте кадров.
            let onset = scheduled.is_some_and(|(v, _)| (display - v).abs() < period / 2);
            let hold = flash_until.is_some_and(|u| display < u);
            let white = onset || hold;
            let back: ID3D11Texture2D = swapchain.GetBuffer(0)?;
            let mut rtv: Option<ID3D11RenderTargetView> = None;
            gpu.device.CreateRenderTargetView(&back, None, Some(&mut rtv))?;
            if let Some(rtv) = &rtv {
                let c = if white { [1.0, 1.0, 1.0, 1.0] } else { [0.0, 0.0, 0.0, 1.0] };
                gpu.context.ClearRenderTargetView(rtv, &c);
            }
            swapchain.Present(1, DXGI_PRESENT(0)).ok()?;
            if onset && let Some((v, ev)) = scheduled.take() {
                pending.push((swapchain.GetLastPresentCount()?, ev));
                flash_until = Some(v + freq / 20);
            } else if !hold {
                flash_until = None;
            }
            // Фактическое время показа белого кадра — из статистики (когда она доступна).
            let mut st = DXGI_FRAME_STATISTICS::default();
            if swapchain.GetFrameStatistics(&mut st).is_ok() {
                pending.retain(|&(id, ev)| {
                    if st.PresentCount >= id {
                        let shown = st.SyncQPCTime - (i64::from(st.PresentCount - id)) * period;
                        events[ev].video_actual = Some(secs(shown, start));
                        // Показ позже цели на целое число развёрток — подаём белый кадр раньше.
                        let target = (events[ev].video_target * freq as f64) as i64 + start;
                        let late = ((shown - target) as f64 / period as f64).round() as i64;
                        lead = (lead + late).clamp(1, 6);
                        false
                    } else {
                        true
                    }
                });
            }
        }
    }
    shared.stop.store(true, Ordering::Release);
    let _ = audio.join();
    // SAFETY: закрытие собственного окна.
    unsafe {
        let _ = DestroyWindow(hwnd);
    }

    let mut json = String::from("[\n");
    for (i, e) in events.iter().enumerate() {
        let actual = e.video_actual.map_or("null".to_string(), |v| format!("{v:.6}"));
        json.push_str(&format!(
            "  {{\"k\": {i}, \"audio\": {:.6}, \"video_target\": {:.6}, \"video_actual\": {actual}}}{}\n",
            e.audio,
            e.video_target,
            if i + 1 < events.len() { "," } else { "" }
        ));
    }
    json.push(']');
    std::fs::write(&out, json)?;
    let measured: Vec<f64> = events.iter().filter_map(|e| e.video_actual.map(|v| (e.audio - v) * 1000.0)).collect();
    println!("событий: {}, со статистикой показа: {}", events.len(), measured.len());
    if !measured.is_empty() {
        let mean = measured.iter().sum::<f64>() / measured.len() as f64;
        println!("эталонный сдвиг звук−показ: среднее {mean:+.2} мс");
    }
    Ok(())
}

/// Минимальный тип ошибки без внешних зависимостей.
mod anyhow_lite {
    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
}
