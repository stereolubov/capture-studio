//! Скриншот-режим (§17.6): экраны 1a–1f на мок-данных дизайна → PNG.
//!
//! Рендер идёт на WARP (программный D3D11), поэтому результат детерминирован и не
//! зависит от GPU — пригоден для эталонных сравнений в CI.

use crate::args::ScreenshotArgs;
use crate::ui::{App, Screen};
use anyhow::Context as _;
use cs_core::config::settings::ThemePref;
use cs_core::config::{Profile, Settings};
use cs_core::mock::MockEngine;
use cs_gpu::{DriverKind, EguiRenderer, GpuDevice, OffscreenTarget};
use egui::{Pos2, Rect, ViewportId, vec2};

/// Отрисованный экран.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Рисует экран и возвращает пиксели.
pub fn render(screen: Screen, light: bool, scale: f32, lang: &str) -> anyhow::Result<Image> {
    let gpu = GpuDevice::new(DriverKind::Warp)?;
    let mut renderer = EguiRenderer::new(&gpu.device, &gpu.context)?;
    let ctx = egui::Context::default();
    ctx.set_fonts(crate::theme::font_definitions());

    let settings = Settings {
        theme: if light { ThemePref::Light } else { ThemePref::Dark },
        language: lang.to_owned(),
        ..Settings::default()
    };
    let mut app = App::new(Box::new(MockEngine::new(false)), settings, Profile::default(), None);

    let run = |app: &mut App, size: egui::Vec2| {
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(0.0),
            max_texture_side: Some(16384),
            ..Default::default()
        };
        raw.viewports.entry(ViewportId::ROOT).or_default().native_pixels_per_point = Some(scale);
        ctx.run_pass(raw, |ctx| app.screenshot_frame(ctx, screen))
    };

    // Первый проход — загрузка шрифтов и измерение; второй — итоговый кадр.
    let mut size = vec2(1280.0, 760.0);
    for _ in 0..3 {
        let mut out = run(&mut app, size);
        renderer.update_textures(&mut out.textures_delta)?;
        renderer.free_textures(&mut out.textures_delta);
        let new_size = app.screen_size(&ctx, screen);
        let stable = (new_size - size).length() < 0.5;
        size = new_size;
        if stable {
            break;
        }
    }
    let mut out = run(&mut app, size);
    let px = [(size.x * scale).round() as u32, (size.y * scale).round() as u32];
    let target = OffscreenTarget::new(&gpu, px)?;
    target.clear(&gpu, [0.0, 0.0, 0.0, 0.0]);
    renderer.update_textures(&mut out.textures_delta)?;
    let primitives = ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
    renderer.render(target.rtv(), px, out.pixels_per_point, &primitives)?;
    renderer.free_textures(&mut out.textures_delta);
    let rgba = target.read_rgba(&gpu)?;
    Ok(Image { width: px[0], height: px[1], rgba })
}

/// Сохраняет RGBA8 в PNG.
pub fn write_png(path: &std::path::Path, img: &Image) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = std::fs::File::create(path).with_context(|| format!("создание {}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header()?;
    w.write_image_data(&img.rgba)?;
    Ok(())
}

pub fn run(args: &ScreenshotArgs) -> anyhow::Result<()> {
    let screen = Screen::parse(&args.screen).with_context(|| format!("неизвестный экран «{}»", args.screen))?;
    let img = render(screen, args.light, args.scale, &args.lang)?;
    write_png(&args.out, &img)?;
    tracing::info!("скриншот {} → {} ({}×{})", args.screen, args.out.display(), img.width, img.height);
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Скриншот-тесты §17.6. Рендер на WARP, поэтому работают и в CI без GPU.
    //!
    //! * 1a/1b сравниваются с эталонами, снятыми с `Capture Studio.dc.html` (`tests/design-ref`).
    //! * Все экраны в обеих темах сравниваются со своими эталонами (`crates/cs-app/tests/golden`);
    //!   обновить эталоны: `CS_UPDATE_GOLDEN=1 cargo test -p cs-app screenshot`.

    use super::*;
    use std::path::{Path, PathBuf};

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn load_png(path: &Path) -> Option<Image> {
        let file = std::fs::File::open(path).ok()?;
        let mut dec = png::Decoder::new(std::io::BufReader::new(file));
        dec.set_transformations(png::Transformations::EXPAND);
        let mut reader = dec.read_info().ok()?;
        let mut buf = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        let px = &buf[..info.buffer_size()];
        let rgba = match info.color_type {
            png::ColorType::Rgba => px.to_vec(),
            png::ColorType::Rgb => px.as_chunks::<3>().0.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            _ => return None,
        };
        Some(Image { width: info.width, height: info.height, rgba })
    }

    /// Доля пикселей, у которых хотя бы один канал отличается больше чем на `tol`.
    fn diff_ratio(a: &Image, b: &Image, tol: u8) -> f64 {
        assert_eq!((a.width, a.height), (b.width, b.height), "размеры различаются");
        let differ = a
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(b.rgba.as_chunks::<4>().0)
            .filter(|(x, y)| x.iter().zip(y.iter()).take(3).any(|(p, q)| p.abs_diff(*q) > tol))
            .count();
        differ as f64 / f64::from(a.width * a.height)
    }

    #[test]
    fn main_window_matches_design() {
        for (name, light) in [("1a", false), ("1b", true)] {
            let img = render(Screen::Main, light, 1.0, "ru").expect("render");
            let reference =
                load_png(&root().join("../../tests/design-ref").join(format!("{name}.png"))).expect("эталон дизайна");
            let r = diff_ratio(&img, &reference, 40);
            // Отличия — сглаживание шрифтов (DirectWrite в браузере vs растеризатор egui)
            // и зоны индикаторов (ТЗ §8.2 считает их от шкалы, а не от ширины заливки).
            assert!(r < 0.015, "{name}: отличается {:.2}% пикселей", r * 100.0);
        }
    }

    #[test]
    fn screens_match_golden() {
        let update = std::env::var_os("CS_UPDATE_GOLDEN").is_some();
        let dir = root().join("tests/golden");
        let mut failures = Vec::new();
        for (name, screen) in [
            ("main", Screen::Main),
            ("settings", Screen::Settings),
            ("mixer", Screen::Mixer),
            ("stats", Screen::Stats),
            ("mini", Screen::Mini),
        ] {
            for (theme, light) in [("dark", false), ("light", true)] {
                for scale in [1.0f32, 1.5] {
                    let img = render(screen, light, scale, "ru").expect("render");
                    let path = dir.join(format!("{name}-{theme}@{}.png", (scale * 100.0) as u32));
                    match load_png(&path) {
                        Some(golden) if !update => {
                            let same_size = (golden.width, golden.height) == (img.width, img.height);
                            let r = if same_size { diff_ratio(&img, &golden, 8) } else { 1.0 };
                            if r > 0.001 {
                                failures.push(format!("{}: {:.3}%", path.display(), r * 100.0));
                            }
                        }
                        _ => write_png(&path, &img).expect("write golden"),
                    }
                }
            }
        }
        assert!(failures.is_empty(), "расхождения с эталонами: {failures:#?}");
    }
}
