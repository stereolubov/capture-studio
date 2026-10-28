//! Окно без системной рамки (§11.2) с рендером egui через D3D11 + DirectComposition.
//!
//! * `WM_NCCALCSIZE` → клиентская область на всё окно (собственный заголовок).
//! * `WM_NCHITTEST` → перетаскивание за заголовок, изменение размера за края,
//!   `HTMAXBUTTON` над кнопкой «развернуть» (Snap Layouts Windows 11).
//! * Per-Monitor V2 DPI: масштаб egui = DPI / 96.
//! * Мини-режим: то же окно, поверх всех, исключено из захвата экрана.
//!
//! Состояние окна живёт в [`Host`]; оконная процедура получает `&Host` и использует
//! внутреннюю изменяемость (`Cell`/`RefCell`), поэтому повторный вход (например,
//! `SetWindowPos` → `WM_SIZE` во время кадра) безопасен: кадр в этом случае пропускается.

use super::{clipboard, input, system};
use crate::ui::{App, Chrome, Mode, WindowCommand, WindowInfo};
use cs_gpu::{EguiRenderer, GpuDevice, WindowSurface};
use egui::{CursorIcon, Pos2, Rect, ViewportId, pos2, vec2};
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, ScreenToClient, ValidateRect,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{MARGINS, WM_MOUSELEAVE};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

/// Имя класса окна (используется и для активации существующего экземпляра).
pub const CLASS_NAME: PCWSTR = w!("CaptureStudioMainWindow");
/// Минимальный размер главного окна, логические пиксели.
const MIN_SIZE: (f32, f32) = (960.0, 600.0);
const DEFAULT_SIZE: (f32, f32) = (1280.0, 760.0);
/// Ширина зоны изменения размера, логические пиксели.
const RESIZE_BORDER: f32 = 6.0;

/// Состояние, разделяемое оконной процедурой и циклом сообщений.
struct Shared {
    events: RefCell<Vec<egui::Event>>,
    ppp: Cell<f32>,
    size_px: Cell<[u32; 2]>,
    chrome: RefCell<Chrome>,
    mode: Cell<Mode>,
    max_hover: Cell<bool>,
    focused: Cell<bool>,
    maximized: Cell<bool>,
    minimized: Cell<bool>,
    cursor: Cell<CursorIcon>,
    repaint: Cell<bool>,
    system_dark: Cell<bool>,
    tracking: Cell<bool>,
    buttons_down: Cell<u8>,
}

/// Всё, что нужно для рендера кадра.
struct Runtime {
    gpu: GpuDevice,
    renderer: EguiRenderer,
    surface: WindowSurface,
    ctx: egui::Context,
    app: App,
    start: Instant,
    next_repaint: Option<Instant>,
    main_placement: Option<WINDOWPLACEMENT>,
    first_frame_logged: bool,
}

pub struct Host {
    hwnd: Cell<HWND>,
    shared: Shared,
    runtime: RefCell<Option<Runtime>>,
}

fn loword(v: usize) -> i32 {
    i32::from((v & 0xFFFF) as u16 as i16)
}
fn hiword(v: usize) -> i32 {
    i32::from(((v >> 16) & 0xFFFF) as u16 as i16)
}

impl Host {
    fn info(&self) -> WindowInfo {
        WindowInfo {
            maximized: self.shared.maximized.get(),
            max_hover: self.shared.max_hover.get(),
            system_dark: self.shared.system_dark.get(),
        }
    }

    fn push(&self, e: egui::Event) {
        self.shared.events.borrow_mut().push(e);
        self.shared.repaint.set(true);
    }

    fn to_points(&self, x: i32, y: i32) -> Pos2 {
        let ppp = self.shared.ppp.get();
        pos2(x as f32 / ppp, y as f32 / ppp)
    }

    /// Рисует кадр, если не находимся уже внутри кадра.
    fn render(&self) {
        let Ok(mut guard) = self.runtime.try_borrow_mut() else {
            return;
        };
        let Some(rt) = guard.as_mut() else {
            return;
        };
        if self.shared.minimized.get() {
            rt.next_repaint = None;
            return;
        }
        self.shared.repaint.set(false);
        let hwnd = self.hwnd.get();
        let ppp = self.shared.ppp.get();
        let size = self.shared.size_px.get();
        if let Err(e) = rt.surface.resize(&rt.gpu, size) {
            tracing::error!("resize swapchain: {e}");
        }

        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(size[0] as f32, size[1] as f32) / ppp)),
            time: Some(rt.start.elapsed().as_secs_f64()),
            events: std::mem::take(&mut *self.shared.events.borrow_mut()),
            focused: self.shared.focused.get(),
            max_texture_side: Some(16384),
            system_theme: Some(if self.shared.system_dark.get() { egui::Theme::Dark } else { egui::Theme::Light }),
            ..Default::default()
        };
        if let Some(vp) = raw.viewports.get_mut(&ViewportId::ROOT) {
            vp.native_pixels_per_point = Some(ppp);
            vp.focused = Some(self.shared.focused.get());
            vp.maximized = Some(self.shared.maximized.get());
        } else {
            raw.viewports.insert(
                ViewportId::ROOT,
                egui::ViewportInfo {
                    native_pixels_per_point: Some(ppp),
                    focused: Some(self.shared.focused.get()),
                    maximized: Some(self.shared.maximized.get()),
                    ..Default::default()
                },
            );
        }

        let info = self.info();
        let app = &mut rt.app;
        let mut out = rt.ctx.run_pass(raw, |ctx| app.frame(ctx, info));
        *self.shared.chrome.borrow_mut() = rt.app.chrome().clone();
        self.shared.mode.set(rt.app.mode);
        self.shared.cursor.set(out.platform_output.cursor_icon);

        for cmd in &out.platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = cmd {
                clipboard::set_text(hwnd, text);
            }
        }
        let delay = out.viewport_output.get(&ViewportId::ROOT).map_or(Duration::from_secs(1), |v| v.repaint_delay);
        rt.next_repaint = Instant::now().checked_add(delay);

        let primitives = rt.ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
        let clear = if rt.app.mode == Mode::Mini {
            [0.0; 4]
        } else {
            let bg = rt.app.palette().bg;
            [f32::from(bg.r()) / 255.0, f32::from(bg.g()) / 255.0, f32::from(bg.b()) / 255.0, 1.0]
        };
        let delta = &mut out.textures_delta;
        let result = (|| -> cs_gpu::Result<()> {
            rt.renderer.update_textures(delta)?;
            let rtv = rt.surface.begin(&rt.gpu, clear)?;
            rt.renderer.render(&rtv, rt.surface.size(), out.pixels_per_point, &primitives)?;
            rt.surface.present(true)?;
            rt.renderer.free_textures(delta);
            Ok(())
        })();
        out.textures_delta.clear();
        if let Err(e) = result {
            // Потеря устройства обрабатывается полноценно в M5 (§15); пока — лог.
            tracing::error!("ошибка рендера UI: {e}");
        }
        if !rt.first_frame_logged {
            rt.first_frame_logged = true;
            tracing::info!("первый кадр показан через {:.0} мс от старта процесса", system::ms_since_process_start());
        }

        let commands = rt.app.take_commands();
        drop(guard);
        for c in commands {
            self.execute(c);
        }
    }

    fn execute(&self, cmd: WindowCommand) {
        let hwnd = self.hwnd.get();
        // SAFETY: hwnd — наше живое окно; вызовы из потока окна.
        unsafe {
            match cmd {
                WindowCommand::Minimize => {
                    let _ = ShowWindow(hwnd, SW_MINIMIZE);
                }
                WindowCommand::ToggleMaximize => {
                    let cmd = if IsZoomed(hwnd).as_bool() { SW_RESTORE } else { SW_MAXIMIZE };
                    let _ = ShowWindow(hwnd, cmd);
                }
                WindowCommand::Close => {
                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                WindowCommand::EnterMini => self.enter_mini(),
                WindowCommand::ExitMini => self.exit_mini(),
                WindowCommand::OpenFolder(path) => {
                    let _ = std::fs::create_dir_all(&path);
                    let wide: Vec<u16> = path.as_os_str().to_string_lossy().encode_utf16().chain(Some(0)).collect();
                    ShellExecuteW(
                        Some(hwnd),
                        w!("open"),
                        PCWSTR(wide.as_ptr()),
                        PCWSTR::null(),
                        PCWSTR::null(),
                        SW_SHOWNORMAL,
                    );
                }
                WindowCommand::CopyText(text) => clipboard::set_text(hwnd, &text),
            }
        }
        self.shared.repaint.set(true);
    }

    fn enter_mini(&self) {
        let hwnd = self.hwnd.get();
        let ppp = self.shared.ppp.get();
        let mut guard = self.runtime.borrow_mut();
        let Some(rt) = guard.as_mut() else { return };
        // SAFETY: операции над собственным окном из его потока.
        unsafe {
            let mut wp =
                WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
            if GetWindowPlacement(hwnd, &mut wp).is_ok() {
                rt.main_placement = Some(wp);
            }
            if IsZoomed(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            self.shared.mode.set(Mode::Mini);
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            let style = style & !(WS_THICKFRAME.0 | WS_MAXIMIZEBOX.0);
            SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize);
            set_corner(hwnd, false);
            let size = crate::ui::mini_size();
            let (w, h) = ((size.x * ppp).round() as i32, (size.y * ppp).round() as i32);
            let (x, y) = match rt.app.settings.mini_position {
                Some((x, y)) => (x, y),
                None => {
                    let work = work_area(hwnd);
                    (work.right - w - (16.0 * ppp) as i32, work.top + (16.0 * ppp) as i32)
                }
            };
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, w, h, SWP_FRAMECHANGED | SWP_SHOWWINDOW);
            // Мини-окно не должно попадать в эфир при захвате экрана (Win10 2004+).
            if system::os_build() >= 19041 {
                let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
            }
        }
        rt.app.mode = Mode::Mini;
    }

    fn exit_mini(&self) {
        let hwnd = self.hwnd.get();
        let mut guard = self.runtime.borrow_mut();
        let Some(rt) = guard.as_mut() else { return };
        // SAFETY: операции над собственным окном из его потока.
        unsafe {
            let mut r = RECT::default();
            if GetWindowRect(hwnd, &mut r).is_ok() {
                rt.app.settings.mini_position = Some((r.left, r.top));
                if let Err(e) = rt.app.save_config() {
                    tracing::warn!("не удалось сохранить позицию мини-режима: {e}");
                }
            }
            let _ = SetWindowDisplayAffinity(hwnd, WDA_NONE);
            self.shared.mode.set(Mode::Main);
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            SetWindowLongPtrW(hwnd, GWL_STYLE, (style | WS_THICKFRAME.0 | WS_MAXIMIZEBOX.0) as isize);
            set_corner(hwnd, true);
            let _ = SetWindowPos(hwnd, Some(HWND_NOTOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED);
            if let Some(wp) = rt.main_placement.take() {
                let _ = SetWindowPlacement(hwnd, &wp);
            }
        }
        rt.app.mode = Mode::Main;
    }

    /// Сохраняет положение окна и настройки при выходе.
    fn save_on_exit(&self) {
        let hwnd = self.hwnd.get();
        let ppp = self.shared.ppp.get();
        let mut guard = self.runtime.borrow_mut();
        let Some(rt) = guard.as_mut() else { return };
        // SAFETY: чтение положения собственного окна.
        unsafe {
            if rt.app.mode == Mode::Mini {
                let mut r = RECT::default();
                if GetWindowRect(hwnd, &mut r).is_ok() {
                    rt.app.settings.mini_position = Some((r.left, r.top));
                }
            }
            let mut wp = rt.main_placement.unwrap_or(WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            });
            if rt.app.mode == Mode::Main {
                let _ = GetWindowPlacement(hwnd, &mut wp);
            }
            let r = wp.rcNormalPosition;
            rt.app.settings.window = Some(cs_core::config::settings::WindowPlacement {
                x: r.left,
                y: r.top,
                width: ((r.right - r.left) as f32 / ppp).round() as u32,
                height: ((r.bottom - r.top) as f32 / ppp).round() as u32,
                maximized: wp.showCmd == SW_SHOWMAXIMIZED.0 as u32,
            });
        }
        if let Err(e) = rt.app.save_config() {
            tracing::warn!("не удалось сохранить настройки при выходе: {e}");
        }
    }

    fn hit_test(&self, screen_x: i32, screen_y: i32) -> u32 {
        let hwnd = self.hwnd.get();
        let mut pt = POINT { x: screen_x, y: screen_y };
        // SAFETY: преобразование координат для собственного окна.
        unsafe {
            let _ = ScreenToClient(hwnd, &mut pt);
        }
        let p = self.to_points(pt.x, pt.y);
        let chrome = self.shared.chrome.borrow();
        let ppp = self.shared.ppp.get();
        let [w, h] = self.shared.size_px.get();
        let (wl, hl) = (w as f32 / ppp, h as f32 / ppp);

        if chrome.drag_everywhere {
            return if chrome.interactive.iter().any(|r| r.contains(p)) { HTCLIENT } else { HTCAPTION };
        }
        if chrome.resizable && !self.shared.maximized.get() {
            let b = RESIZE_BORDER;
            let (l, r, t, bt) = (p.x < b, p.x >= wl - b, p.y < b, p.y >= hl - b);
            match (l, r, t, bt) {
                (true, _, true, _) => return HTTOPLEFT,
                (_, true, true, _) => return HTTOPRIGHT,
                (true, _, _, true) => return HTBOTTOMLEFT,
                (_, true, _, true) => return HTBOTTOMRIGHT,
                (true, ..) => return HTLEFT,
                (_, true, ..) => return HTRIGHT,
                (_, _, true, _) => return HTTOP,
                (_, _, _, true) => return HTBOTTOM,
                _ => {}
            }
        }
        if chrome.max_button.is_some_and(|r| r.contains(p)) {
            return HTMAXBUTTON;
        }
        if chrome.caption.contains(p) && !chrome.interactive.iter().any(|r| r.contains(p)) {
            return HTCAPTION;
        }
        HTCLIENT
    }
}

fn set_corner(hwnd: HWND, round: bool) {
    let pref = if round { DWMWCP_ROUND } else { DWMWCP_DONOTROUND };
    // SAFETY: атрибут DWM с корректным размером значения; ошибка (Win10) игнорируется.
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, (&raw const pref).cast(), 4);
    }
}

fn work_area(hwnd: HWND) -> RECT {
    // SAFETY: MONITORINFO с корректным cbSize.
    unsafe {
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(mon, &mut mi);
        mi.rcWork
    }
}

fn cursor_for(icon: CursorIcon) -> PCWSTR {
    match icon {
        CursorIcon::PointingHand => IDC_HAND,
        CursorIcon::Text => IDC_IBEAM,
        CursorIcon::ResizeVertical | CursorIcon::ResizeRow => IDC_SIZENS,
        CursorIcon::ResizeHorizontal | CursorIcon::ResizeColumn => IDC_SIZEWE,
        CursorIcon::Move | CursorIcon::Grab | CursorIcon::Grabbing => IDC_SIZEALL,
        CursorIcon::NotAllowed | CursorIcon::NoDrop => IDC_NO,
        CursorIcon::Wait => IDC_WAIT,
        CursorIcon::Progress => IDC_APPSTARTING,
        CursorIcon::Help => IDC_HELP,
        CursorIcon::Crosshair => IDC_CROSS,
        _ => IDC_ARROW,
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: GWLP_USERDATA содержит указатель на Host, установленный в WM_NCCREATE;
    // Host живёт дольше окна (уничтожается после выхода из цикла сообщений).
    unsafe {
        if msg == WM_NCCREATE {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Host;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let host = &*ptr;
        handle(host, hwnd, msg, wparam, lparam).unwrap_or_else(|| DefWindowProcW(hwnd, msg, wparam, lparam))
    }
}

/// Обработка сообщения; `None` — передать в `DefWindowProcW`.
///
/// # Safety
/// Вызывается только из `wndproc` для собственного окна.
unsafe fn handle(host: &Host, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    let sh = &host.shared;
    let mouse_pos = || host.to_points(loword(lparam.0 as usize), hiword(lparam.0 as usize));
    let button = |b: egui::PointerButton, pressed: bool| {
        let n = sh.buttons_down.get();
        // SAFETY: захват мыши собственным окном.
        unsafe {
            if pressed {
                if n == 0 {
                    SetCapture(hwnd);
                }
                sh.buttons_down.set(n + 1);
            } else {
                sh.buttons_down.set(n.saturating_sub(1));
                if n <= 1 {
                    let _ = ReleaseCapture();
                }
            }
        }
        host.push(egui::Event::PointerButton { pos: mouse_pos(), button: b, pressed, modifiers: input::modifiers() });
        Some(LRESULT(0))
    };
    // SAFETY: все вызовы Win32 ниже относятся к собственному окну и выполняются в его потоке.
    unsafe {
        match msg {
            WM_NCCALCSIZE if wparam.0 != 0 => {
                // Клиентская область на всё окно; при развороте — компенсация выхода за экран.
                if IsZoomed(hwnd).as_bool() {
                    let params = &mut *(lparam.0 as *mut NCCALCSIZE_PARAMS);
                    let dpi = GetDpiForWindow(hwnd);
                    let pad = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
                    let fx = GetSystemMetricsForDpi(SM_CXFRAME, dpi) + pad;
                    let fy = GetSystemMetricsForDpi(SM_CYFRAME, dpi) + pad;
                    let r = &mut params.rgrc[0];
                    r.left += fx;
                    r.right -= fx;
                    r.top += fy;
                    r.bottom -= fy;
                }
                Some(LRESULT(0))
            }
            WM_NCHITTEST => Some(LRESULT(host.hit_test(loword(lparam.0 as usize), hiword(lparam.0 as usize)) as isize)),
            WM_NCMOUSEMOVE => {
                let over_max = wparam.0 as u32 == HTMAXBUTTON;
                if over_max != sh.max_hover.get() {
                    sh.max_hover.set(over_max);
                    sh.repaint.set(true);
                }
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
                let mut pt = POINT { x: loword(lparam.0 as usize), y: hiword(lparam.0 as usize) };
                let _ = ScreenToClient(hwnd, &mut pt);
                host.push(egui::Event::PointerMoved(host.to_points(pt.x, pt.y)));
                None
            }
            WM_NCMOUSELEAVE => {
                if sh.max_hover.replace(false) {
                    sh.repaint.set(true);
                }
                None
            }
            WM_NCLBUTTONDOWN if wparam.0 as u32 == HTMAXBUTTON => Some(LRESULT(0)),
            WM_NCLBUTTONUP if wparam.0 as u32 == HTMAXBUTTON => {
                host.execute(WindowCommand::ToggleMaximize);
                Some(LRESULT(0))
            }
            WM_NCLBUTTONDBLCLK if sh.mode.get() == Mode::Mini => Some(LRESULT(0)),
            WM_MOUSEMOVE => {
                if sh.max_hover.replace(false) {
                    sh.repaint.set(true);
                }
                if !sh.tracking.get() {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                    sh.tracking.set(true);
                }
                host.push(egui::Event::PointerMoved(mouse_pos()));
                Some(LRESULT(0))
            }
            WM_MOUSELEAVE => {
                sh.tracking.set(false);
                host.push(egui::Event::PointerGone);
                Some(LRESULT(0))
            }
            WM_LBUTTONDOWN => button(egui::PointerButton::Primary, true),
            WM_LBUTTONUP => button(egui::PointerButton::Primary, false),
            WM_RBUTTONDOWN => button(egui::PointerButton::Secondary, true),
            WM_RBUTTONUP => button(egui::PointerButton::Secondary, false),
            WM_MBUTTONDOWN => button(egui::PointerButton::Middle, true),
            WM_MBUTTONUP => button(egui::PointerButton::Middle, false),
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                let delta = hiword(wparam.0) as f32 / WHEEL_DELTA as f32;
                let delta = if msg == WM_MOUSEWHEEL { vec2(0.0, delta) } else { vec2(-delta, 0.0) };
                host.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta,
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Wheel,
                    modifiers: input::modifiers(),
                });
                Some(LRESULT(0))
            }
            WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
                let pressed = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                let repeat = pressed && (lparam.0 >> 30) & 1 == 1;
                let modifiers = input::modifiers();
                sh.repaint.set(true);
                host.push(egui::Event::ModifiersChanged(modifiers));
                if let Some(key) = input::vk_to_key(wparam.0 as u16) {
                    if pressed && modifiers.ctrl && !modifiers.alt {
                        match key {
                            egui::Key::C => host.push(egui::Event::Copy),
                            egui::Key::X => host.push(egui::Event::Cut),
                            egui::Key::V => {
                                if let Some(text) = clipboard::get_text(hwnd) {
                                    host.push(egui::Event::Paste(text));
                                }
                            }
                            _ => {}
                        }
                    }
                    host.push(egui::Event::Key { key, physical_key: None, pressed, repeat, modifiers });
                }
                // Alt+F4 и системные сочетания обрабатывает система.
                if msg == WM_SYSKEYDOWN || msg == WM_SYSKEYUP { None } else { Some(LRESULT(0)) }
            }
            WM_CHAR => {
                let c = wparam.0 as u32;
                if let Some(ch) = char::from_u32(c)
                    && !ch.is_control()
                {
                    host.push(egui::Event::Text(ch.to_string()));
                }
                Some(LRESULT(0))
            }
            WM_SETFOCUS | WM_KILLFOCUS => {
                let f = msg == WM_SETFOCUS;
                sh.focused.set(f);
                host.push(egui::Event::WindowFocused(f));
                Some(LRESULT(0))
            }
            WM_SIZE => {
                let (w, h) = (loword(lparam.0 as usize) as u32, hiword(lparam.0 as usize) as u32);
                sh.minimized.set(wparam.0 as u32 == SIZE_MINIMIZED);
                sh.maximized.set(wparam.0 as u32 == SIZE_MAXIMIZED);
                if w > 0 && h > 0 {
                    sh.size_px.set([w, h]);
                    // Перерисовка во время изменения размера (модальный цикл системы).
                    host.render();
                }
                Some(LRESULT(0))
            }
            WM_DPICHANGED => {
                sh.ppp.set(f32::from(hiword(wparam.0) as u16) / 96.0);
                let r = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                sh.repaint.set(true);
                Some(LRESULT(0))
            }
            WM_GETMINMAXINFO => {
                let mmi = &mut *(lparam.0 as *mut MINMAXINFO);
                let ppp = sh.ppp.get();
                if sh.mode.get() == Mode::Main {
                    mmi.ptMinTrackSize.x = (MIN_SIZE.0 * ppp) as i32;
                    mmi.ptMinTrackSize.y = (MIN_SIZE.1 * ppp) as i32;
                }
                Some(LRESULT(0))
            }
            WM_SETCURSOR if loword(lparam.0 as usize) as u32 == HTCLIENT => {
                if let Ok(c) = LoadCursorW(None, cursor_for(sh.cursor.get())) {
                    SetCursor(Some(c));
                }
                Some(LRESULT(1))
            }
            WM_SETTINGCHANGE => {
                if lparam.0 != 0 {
                    let s = PCWSTR(lparam.0 as *const u16);
                    if s.to_string().is_ok_and(|s| s == "ImmersiveColorSet") {
                        sh.system_dark.set(system::system_dark_theme());
                        sh.repaint.set(true);
                    }
                }
                None
            }
            WM_PAINT => {
                let _ = ValidateRect(Some(hwnd), None);
                host.render();
                Some(LRESULT(0))
            }
            WM_ERASEBKGND => Some(LRESULT(1)),
            WM_CLOSE | WM_ENDSESSION => {
                host.save_on_exit();
                let _ = DestroyWindow(hwnd);
                Some(LRESULT(0))
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}

/// Иконка окна: красная точка (как логотип) — генерируется в рантайме, без ресурсов.
fn create_icon(size: i32) -> Option<HICON> {
    let n = (size * size) as usize;
    let mut bgra = vec![0u8; n * 4];
    let c = size as f32 / 2.0;
    let r = size as f32 * 0.38;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2)).sqrt();
            let a = (r + 0.5 - d).clamp(0.0, 1.0);
            let i = ((y * size + x) * 4) as usize;
            // Premultiplied BGRA (#ed4a49).
            bgra[i] = (0x49 as f32 * a) as u8;
            bgra[i + 1] = (0x4a as f32 * a) as u8;
            bgra[i + 2] = (0xed as f32 * a) as u8;
            bgra[i + 3] = (255.0 * a) as u8;
        }
    }
    let mask = vec![0u8; ((size + 15) / 16 * 2 * size) as usize];
    // SAFETY: буферы соответствуют размерам и формату (1 плоскость, 32 бита).
    unsafe { CreateIcon(None, size, size, 1, 32, mask.as_ptr(), bgra.as_ptr()).ok() }
}

/// Создаёт окно и запускает цикл сообщений до выхода.
pub fn run(app: App, gpu: GpuDevice) -> anyhow::Result<()> {
    let host = Box::new(Host {
        hwnd: Cell::new(HWND::default()),
        shared: Shared {
            events: RefCell::new(Vec::new()),
            ppp: Cell::new(1.0),
            size_px: Cell::new([1280, 760]),
            chrome: RefCell::new(Chrome::default()),
            mode: Cell::new(Mode::Main),
            max_hover: Cell::new(false),
            focused: Cell::new(true),
            maximized: Cell::new(false),
            minimized: Cell::new(false),
            cursor: Cell::new(CursorIcon::Default),
            repaint: Cell::new(true),
            system_dark: Cell::new(system::system_dark_theme()),
            tracking: Cell::new(false),
            buttons_down: Cell::new(0),
        },
        runtime: RefCell::new(None),
    });
    let host_ptr: *const Host = &*host;
    let placement = app.settings.window;
    let start_minimized = app.settings.start_minimized;

    // SAFETY: регистрация класса и создание окна с указателем на Host, который
    // живёт до конца функции (дольше окна).
    let hwnd = unsafe {
        let hinstance: HINSTANCE = GetModuleHandleW(None)?.into();
        let icon = create_icon(32);
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            hIcon: icon.unwrap_or_default(),
            hIconSm: create_icon(16).unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        if RegisterClassExW(&wc) == 0 {
            anyhow::bail!("RegisterClassExW: {}", windows::core::Error::from_thread());
        }
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_APPWINDOW,
            CLASS_NAME,
            w!("Capture Studio"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            DEFAULT_SIZE.0 as i32,
            DEFAULT_SIZE.1 as i32,
            None,
            None,
            Some(hinstance),
            Some(host_ptr.cast()),
        )?
    };
    host.hwnd.set(hwnd);

    // SAFETY: настройка собственного окна до показа.
    let (w, h) = unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        let ppp = dpi as f32 / 96.0;
        host.shared.ppp.set(ppp);
        let dark = windows::core::BOOL::from(true);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, (&raw const dark).cast(), 4);
        set_corner(hwnd, true);
        // Свою рамку 1 px рисуем сами (как в дизайне) — системную отключаем (Win11).
        let none: u32 = 0xFFFF_FFFE;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, (&raw const none).cast(), 4);
        let margins = MARGINS { cxLeftWidth: 0, cxRightWidth: 0, cyTopHeight: 1, cyBottomHeight: 0 };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

        let work = work_area(hwnd);
        let (w, h, x, y) = match placement {
            Some(p) => {
                let w = (p.width.max(MIN_SIZE.0 as u32) as f32 * ppp) as i32;
                let h = (p.height.max(MIN_SIZE.1 as u32) as f32 * ppp) as i32;
                (w, h, p.x, p.y)
            }
            None => {
                let w = ((DEFAULT_SIZE.0 * ppp) as i32).min(work.right - work.left);
                let h = ((DEFAULT_SIZE.1 * ppp) as i32).min(work.bottom - work.top);
                (w, h, work.left + (work.right - work.left - w) / 2, work.top + (work.bottom - work.top - h) / 2)
            }
        };
        let _ = SetWindowPos(hwnd, None, x, y, w, h, SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE);
        let mut actual = RECT::default();
        let _ = GetWindowRect(hwnd, &mut actual);
        tracing::debug!(
            "окно: dpi {dpi}, рабочая область {:?}, запрошено ({x},{y},{w}×{h}), получено {:?}",
            (work.left, work.top, work.right, work.bottom),
            (actual.left, actual.top, actual.right, actual.bottom)
        );
        (w, h)
    };
    host.shared.size_px.set([w.max(1) as u32, h.max(1) as u32]);

    let surface = WindowSurface::new(&gpu, hwnd, host.shared.size_px.get())?;
    let renderer = EguiRenderer::new(&gpu.device, &gpu.context)?;
    let ctx = egui::Context::default();
    ctx.set_fonts(crate::theme::font_definitions());
    *host.runtime.borrow_mut() = Some(Runtime {
        gpu,
        renderer,
        surface,
        ctx,
        app,
        start: Instant::now(),
        next_repaint: Some(Instant::now()),
        main_placement: None,
        first_frame_logged: false,
    });

    // Первый кадр до показа окна, чтобы не мигал пустой фон.
    host.render();
    // SAFETY: показ собственного окна.
    unsafe {
        let cmd = if start_minimized {
            SW_SHOWMINIMIZED
        } else if placement.is_some_and(|p| p.maximized) {
            SW_SHOWMAXIMIZED
        } else {
            SW_SHOW
        };
        let _ = ShowWindow(hwnd, cmd);
    }

    message_loop(&host);
    // Освобождаем GPU-ресурсы до уничтожения Host.
    host.runtime.borrow_mut().take();
    Ok(())
}

fn message_loop(host: &Host) {
    let mut msg = MSG::default();
    loop {
        // SAFETY: стандартный цикл сообщений потока окна.
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    return;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let next = host.runtime.borrow().as_ref().and_then(|rt| rt.next_repaint);
        let now = Instant::now();
        if host.shared.repaint.get() || next.is_some_and(|t| t <= now) {
            host.render();
            continue;
        }
        let timeout = next.map_or(windows::Win32::System::Threading::INFINITE, |t| {
            t.saturating_duration_since(now).as_millis().clamp(1, 1000) as u32
        });
        // SAFETY: ожидание сообщений без объектов синхронизации.
        unsafe {
            MsgWaitForMultipleObjectsEx(None, timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        }
    }
}

/// Активирует уже запущенный экземпляр (§15: единственный экземпляр).
pub fn activate_existing() -> bool {
    // SAFETY: поиск окна по имени класса и активация.
    unsafe {
        match FindWindowW(CLASS_NAME, PCWSTR::null()) {
            Ok(h) if !h.is_invalid() => {
                let _ = ShowWindow(h, SW_RESTORE);
                let _ = SetForegroundWindow(h);
                true
            }
            _ => false,
        }
    }
}
