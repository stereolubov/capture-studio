//! Windows.Graphics.Capture: захват монитора и окна (§6.4, методы B и C).
//!
//! Кадры приходят в пул `Direct3D11CaptureFramePool` (free-threaded) на нашем устройстве;
//! рендер-поток забирает последний кадр и копирует его GPU→GPU в текстуру источника.
//! Размер окна меняется → пул пересоздаётся; окно закрыто → ожидание и автоприкрепление
//! по правилу «exe + класс» раз в секунду; свёрнуто → последний кадр.

use crate::enumerate;
use crate::{CaptureMethod, Content, SourceState, VideoSource};
use cs_gpu::{GpuDevice, GpuTexture};
use std::time::{Duration, Instant};
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::{D3D11_BOX, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindow};
use windows::core::Interface;

const RETRY: Duration = Duration::from_secs(1);

/// Поддерживается ли WGC (Windows 10 1903+).
pub fn supported() -> bool {
    GraphicsCaptureSession::IsSupported().unwrap_or(false)
}

enum Target {
    Monitor(u32),
    Window { exe: String, class: String },
}

struct Session {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    size: SizeInt32,
    hwnd: Option<HWND>,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

pub struct WgcSource {
    target: Target,
    cursor: bool,
    device: Option<IDirect3DDevice>,
    session: Option<Session>,
    texture: Option<GpuTexture>,
    state: SourceState,
    last_attempt: Option<Instant>,
    frame_time: Option<cs_core::clock::Ticks>,
}

// SAFETY: объекты WGC созданы free-threaded (агильные), D3D-ресурсы — на свободно-поточном
// устройстве; источник используется только рендер-потоком под блокировкой устройства.
unsafe impl Send for WgcSource {}

fn d3d_device(gpu: &GpuDevice) -> windows::core::Result<IDirect3DDevice> {
    let dxgi: IDXGIDevice = gpu.device.cast()?;
    // SAFETY: обёртка WinRT над живым DXGI-устройством.
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi)? };
    inspectable.cast()
}

impl WgcSource {
    fn new(gpu: &GpuDevice, target: Target, cursor: bool) -> Self {
        let (device, state) = match d3d_device(gpu) {
            Ok(d) => (Some(d), SourceState::Waiting),
            Err(e) => (None, SourceState::Error(format!("WGC недоступен: {e}"))),
        };
        Self { target, cursor, device, session: None, texture: None, state, last_attempt: None, frame_time: None }
    }

    pub fn monitor(gpu: &GpuDevice, index: u32, cursor: bool) -> Self {
        Self::new(gpu, Target::Monitor(index), cursor)
    }

    pub fn window(gpu: &GpuDevice, exe: String, class: String, cursor: bool) -> Self {
        Self::new(gpu, Target::Window { exe, class }, cursor)
    }

    fn start(&mut self) -> Result<(), String> {
        let device = self.device.clone().ok_or("нет устройства")?;
        let interop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(|e| e.to_string())?;
        let (item, hwnd): (GraphicsCaptureItem, Option<HWND>) = match &self.target {
            Target::Monitor(index) => {
                let m =
                    enumerate::monitor_by_index(*index).ok_or_else(|| format!("монитор {} не найден", index + 1))?;
                // SAFETY: дескриптор монитора получен только что из перечисления.
                let item =
                    unsafe { interop.CreateForMonitor(HMONITOR(m.handle as *mut _)) }.map_err(|e| e.to_string())?;
                (item, None)
            }
            Target::Window { exe, class } => {
                let w = enumerate::find_window(exe, class).ok_or("окно не найдено")?;
                let hwnd = HWND(w.hwnd as *mut _);
                // SAFETY: hwnd получен из перечисления; если окно уже закрыто, вызов вернёт ошибку.
                let item = unsafe { interop.CreateForWindow(hwnd) }.map_err(|e| e.to_string())?;
                (item, Some(hwnd))
            }
        };
        let size = item.Size().map_err(|e| e.to_string())?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )
        .map_err(|e| e.to_string())?;
        let session = pool.CreateCaptureSession(&item).map_err(|e| e.to_string())?;
        // Отключение курсора — Windows 10 2004+, жёлтой рамки — Windows 11; на старых — игнорируем.
        let _ = session.SetIsCursorCaptureEnabled(self.cursor);
        let _ = session.SetIsBorderRequired(false);
        session.StartCapture().map_err(|e| e.to_string())?;
        self.session = Some(Session { pool, session, size, hwnd });
        Ok(())
    }

    fn pull(&mut self, gpu: &GpuDevice) -> windows::core::Result<()> {
        let Some(s) = self.session.as_mut() else { return Ok(()) };
        let mut latest = None;
        while let Ok(f) = s.pool.TryGetNextFrame() {
            // Промежуточные кадры закрываем сразу, чтобы пул не остался без буферов.
            if let Some(old) = latest.replace(f) {
                let _ = old.Close();
            }
        }
        let Some(frame) = latest else { return Ok(()) };
        let content = frame.ContentSize()?;
        // SystemRelativeTime — время композиции кадра в шкале QPC (100 нс).
        if let Ok(t) = frame.SystemRelativeTime() {
            let freq = i128::from(cs_core::clock::Ticks::frequency());
            self.frame_time = Some(cs_core::clock::Ticks((i128::from(t.Duration) * freq / 10_000_000) as i64));
        }
        let surface = frame.Surface()?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
        // SAFETY: поверхность кадра — текстура нашего устройства.
        let src: ID3D11Texture2D = unsafe { access.GetInterface()? };
        let (w, h) = (content.Width.max(1) as u32, content.Height.max(1) as u32);
        let w = w.min(s.size.Width.max(1) as u32);
        let h = h.min(s.size.Height.max(1) as u32);
        if self.texture.as_ref().is_none_or(|t| (t.width, t.height) != (w, h)) {
            self.texture = gpu.create_texture(w, h, DXGI_FORMAT_B8G8R8A8_UNORM, None, false).ok();
        }
        if let Some(t) = &self.texture {
            let b = D3D11_BOX { left: 0, top: 0, front: 0, right: w, bottom: h, back: 1 };
            // SAFETY: обе текстуры BGRA8 на одном устройстве; бокс в пределах источника.
            unsafe { gpu.context.CopySubresourceRegion(&t.texture, 0, 0, 0, 0, &src, 0, Some(&b)) };
        }
        let _ = frame.Close();
        // Окно изменило размер — пересоздаём пул под новый размер.
        if content.Width != s.size.Width || content.Height != s.size.Height {
            if let Some(device) = &self.device {
                s.pool.Recreate(device, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, content)?;
            }
            s.size = content;
        }
        Ok(())
    }
}

impl VideoSource for WgcSource {
    fn update(&mut self, gpu: &GpuDevice) {
        if self.device.is_none() {
            return;
        }
        // Окно закрыто → ожидание; свёрнуто → последний кадр.
        if let Some(hwnd) = self.session.as_ref().and_then(|s| s.hwnd) {
            // SAFETY: проверка дескриптора без доступа к окну.
            if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
                self.session = None;
                self.state = SourceState::Waiting;
            } else if unsafe { IsIconic(hwnd) }.as_bool() {
                self.state = SourceState::Minimized;
            } else if self.state == SourceState::Minimized {
                self.state = SourceState::Active;
            }
        }
        if self.session.is_none() {
            let now = Instant::now();
            if self.last_attempt.is_some_and(|t| now - t < RETRY) {
                return;
            }
            self.last_attempt = Some(now);
            match self.start() {
                Ok(()) => self.state = SourceState::Active,
                Err(e) => {
                    self.state = match self.target {
                        Target::Window { .. } => SourceState::Waiting,
                        Target::Monitor(_) => SourceState::Error(e),
                    }
                }
            }
        }
        if let Err(e) = self.pull(gpu) {
            tracing::warn!("WGC: {e}");
            self.session = None;
        }
    }

    fn content(&self) -> Option<Content> {
        self.texture.as_ref().map(|t| Content::Texture {
            srv: t.srv.clone(),
            width: t.width,
            height: t.height,
            rotation: 0,
        })
    }

    fn state(&self) -> SourceState {
        self.state.clone()
    }

    fn method(&self) -> CaptureMethod {
        CaptureMethod::Wgc
    }

    fn frame_time(&self) -> Option<cs_core::clock::Ticks> {
        self.frame_time
    }
}
