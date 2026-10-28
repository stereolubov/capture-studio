//! Рендер текста через DirectWrite + Direct2D прямо в D3D11-текстуру (§6.5, источник «Текст»).
//!
//! Текстура перерисовывается только при изменении параметров. Direct2D работает поверх
//! DXGI-поверхности текстуры того же устройства, поэтому вызывать под [`GpuDevice::lock`].

use crate::{GpuDevice, GpuError, GpuTexture, Result};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT};
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::IDXGISurface;
use windows::core::{HSTRING, Interface};

/// Параметры текста.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub text: String,
    /// Имя семейства системного шрифта.
    pub font: String,
    pub size: f32,
    /// Цвет текста, RGBA 0..255 (не premultiplied).
    pub color: [u8; 4],
    /// Обводка: цвет и толщина в пикселях (0 — без обводки).
    pub outline: [u8; 4],
    pub outline_width: f32,
}

/// Фабрики DirectWrite/Direct2D (создаются один раз).
pub struct TextRenderer {
    d2d: ID2D1Factory,
    dwrite: IDWriteFactory,
}

fn color(c: [u8; 4]) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(c[0]) / 255.0,
        g: f32::from(c[1]) / 255.0,
        b: f32::from(c[2]) / 255.0,
        a: f32::from(c[3]) / 255.0,
    }
}

impl TextRenderer {
    pub fn new() -> Result<Self> {
        // SAFETY: создание фабрик без параметров-указателей, кроме опций по умолчанию.
        unsafe {
            let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_MULTI_THREADED, None)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            Ok(Self { d2d, dwrite })
        }
    }

    /// Рисует текст в новую текстуру по размеру текста (с полями под обводку).
    pub fn render(&self, gpu: &GpuDevice, style: &TextStyle) -> Result<GpuTexture> {
        let text = if style.text.is_empty() { " " } else { style.text.as_str() };
        let wide: Vec<u16> = text.encode_utf16().collect();
        let pad = style.outline_width.max(0.0).ceil() + 2.0;
        // SAFETY: COM-вызовы на живых объектах; буфер строки живёт до конца функции;
        // текстура создана с BIND_RENDER_TARGET и форматом BGRA, совместимым с Direct2D.
        unsafe {
            let format = self.dwrite.CreateTextFormat(
                &HSTRING::from(style.font.as_str()),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                style.size.clamp(4.0, 512.0),
                &HSTRING::from("ru-ru"),
            )?;
            let layout = self.dwrite.CreateTextLayout(&wide, &format, 16384.0, 16384.0)?;
            let mut m = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut m)?;
            let w = (m.widthIncludingTrailingWhitespace + pad * 2.0).ceil().clamp(1.0, 8192.0) as u32;
            let h = (m.height + pad * 2.0).ceil().clamp(1.0, 8192.0) as u32;

            let tex = gpu.create_texture(w, h, DXGI_FORMAT_B8G8R8A8_UNORM, None, true)?;
            let surface: IDXGISurface = tex.texture.cast()?;
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let rt = self.d2d.CreateDxgiSurfaceRenderTarget(&surface, &props)?;
            // На прозрачном фоне ClearType невозможен — сглаживание в оттенках серого.
            rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            rt.BeginDraw();
            rt.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
            let origin = windows_numerics::Vector2 { X: pad, Y: pad };
            if style.outline_width > 0.0 && style.outline[3] > 0 {
                // Простая обводка: текст цветом обводки со смещениями по окружности.
                let brush = rt.CreateSolidColorBrush(&color(style.outline), None)?;
                let steps = 16;
                for i in 0..steps {
                    let a = i as f32 / steps as f32 * std::f32::consts::TAU;
                    let o = windows_numerics::Vector2 {
                        X: origin.X + a.cos() * style.outline_width,
                        Y: origin.Y + a.sin() * style.outline_width,
                    };
                    rt.DrawTextLayout(o, &layout, &brush, D2D1_DRAW_TEXT_OPTIONS_NONE);
                }
            }
            let brush = rt.CreateSolidColorBrush(&color(style.color), None)?;
            rt.DrawTextLayout(origin, &layout, &brush, D2D1_DRAW_TEXT_OPTIONS_NONE);
            rt.EndDraw(None, None).map_err(GpuError::from)?;
            Ok(tex)
        }
    }

    /// Имена установленных семейств шрифтов (для выбора в свойствах источника).
    pub fn system_fonts(&self) -> Vec<String> {
        let mut out = Vec::new();
        // SAFETY: перечисление системной коллекции шрифтов; буферы имён по длине из API.
        unsafe {
            let mut coll = None;
            if self.dwrite.GetSystemFontCollection(&mut coll, false).is_err() {
                return out;
            }
            let Some(coll) = coll else { return out };
            for i in 0..coll.GetFontFamilyCount() {
                let Ok(fam) = coll.GetFontFamily(i) else { continue };
                let Ok(names) = fam.GetFamilyNames() else { continue };
                let Ok(len) = names.GetStringLength(0) else { continue };
                let mut buf = vec![0u16; len as usize + 1];
                if names.GetString(0, &mut buf).is_ok() {
                    out.push(String::from_utf16_lossy(&buf[..len as usize]));
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_cyrillic_text() {
        let gpu = GpuDevice::new(crate::DriverKind::Warp).expect("WARP");
        let r = TextRenderer::new().expect("dwrite");
        let _g = gpu.lock();
        let style = TextStyle {
            text: "Скоро вернёмся".into(),
            font: "Segoe UI".into(),
            size: 32.0,
            color: [255, 255, 255, 255],
            outline: [0, 0, 0, 255],
            outline_width: 2.0,
        };
        let tex = r.render(&gpu, &style).expect("render");
        assert!(tex.width > 100 && tex.height > 30, "{}×{}", tex.width, tex.height);
        let px = gpu.read_pixels(&tex).expect("read");
        let opaque = px.as_chunks::<4>().0.iter().filter(|p| p[3] > 200).count();
        assert!(opaque > 200, "непрозрачных пикселей: {opaque}");
        assert!(!r.system_fonts().is_empty());
    }
}
