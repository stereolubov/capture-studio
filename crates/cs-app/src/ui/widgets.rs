//! Примитивы отрисовки под дизайн: точная раскладка в логических пикселях (допуск ±2 px, §11).
//!
//! Стандартные виджеты egui добавляют свои отступы, поэтому экраны собираются из
//! прямоугольников, текста и `ui.interact` вручную.

use crate::theme::{self, Palette};
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align2, Color32, CornerRadius, FontId, Galley, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use std::sync::Arc;

/// Высота строки CSS `line-height: normal` для шрифта (совпадает с высотой строки egui).
pub fn line_height(painter: &Painter, font: &FontId) -> f32 {
    painter.layout_no_wrap("Ag".into(), font.clone(), Color32::WHITE).size().y
}

/// То же, что [`line_height`], но через контекст (до начала отрисовки).
pub fn line_height_ctx(ctx: &egui::Context, font: &FontId) -> f32 {
    ctx.fonts_mut(|f| f.layout_no_wrap("Ag".into(), font.clone(), Color32::WHITE).size().y)
}

/// Раскладка текста одной строкой.
pub fn galley(painter: &Painter, text: &str, font: FontId, color: Color32) -> Arc<Galley> {
    painter.layout_no_wrap(text.to_owned(), font, color)
}

/// Раскладка с межбуквенным интервалом (подписи секций, `letter-spacing: .08em`).
pub fn galley_spaced(painter: &Painter, text: &str, font: FontId, color: Color32, spacing: f32) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(text, 0.0, TextFormat { font_id: font, color, extra_letter_spacing: spacing, ..Default::default() });
    painter.layout_job(job)
}

/// Рисует текст, выравнивая галерею относительно точки; возвращает занятый прямоугольник.
pub fn text(painter: &Painter, pos: Pos2, align: Align2, text: &str, font: FontId, color: Color32) -> Rect {
    let g = galley(painter, text, font, color);
    let rect = align.anchor_size(pos, g.size());
    painter.galley(rect.min, g, color);
    rect
}

/// Подпись секции: 11 px, ВЕРХНИЙ РЕГИСТР, `mute`, интервал 0.08em.
pub fn caption(painter: &Painter, pos: Pos2, align: Align2, s: &str, p: &Palette) -> Rect {
    let g = galley_spaced(painter, s, theme::sans(theme::TEXT_CAPTION), p.mute, theme::CAPTION_SPACING);
    let rect = align.anchor_size(pos, g.size());
    painter.galley(rect.min, g, p.mute);
    rect
}

/// Ширина текста.
pub fn text_width(painter: &Painter, s: &str, font: FontId) -> f32 {
    galley(painter, s, font, Color32::WHITE).size().x
}

/// Горизонтальная линия толщиной 1 px (пиксельно чёткая).
pub fn hline(painter: &Painter, x0: f32, x1: f32, y: f32, color: Color32) {
    painter.rect_filled(Rect::from_min_max(pos2(x0, y), pos2(x1, y + 1.0)), 0.0, color);
}

/// Вертикальная линия толщиной 1 px.
pub fn vline(painter: &Painter, x: f32, y0: f32, y1: f32, color: Color32) {
    painter.rect_filled(Rect::from_min_max(pos2(x, y0), pos2(x + 1.0, y1)), 0.0, color);
}

/// Рамка 1 px внутри прямоугольника (CSS `border: 1px`).
pub fn border(painter: &Painter, rect: Rect, radius: f32, color: Color32) {
    painter.rect_stroke(rect, radius, Stroke::new(1.0, color), StrokeKind::Inside);
}

/// Диагональная штриховка 45° (`repeating-linear-gradient(45deg, a 0 10px, b 10px 20px)`).
pub fn hatch(painter: &Painter, rect: Rect, a: Color32, b: Color32) {
    painter.rect_filled(rect, 0.0, b);
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    // Полосы постоянного цвета идут вдоль (1,1); период по нормали 20 px → по оси X 20·√2.
    let period = 20.0 * std::f32::consts::SQRT_2;
    let band = period / 2.0;
    let h = rect.height();
    let mut mesh = Mesh::default();
    let mut x = rect.left() - h - period;
    while x < rect.right() + period {
        let i = mesh.vertices.len() as u32;
        for (px, py) in [(x, rect.bottom()), (x + band, rect.bottom()), (x + band + h, rect.top()), (x + h, rect.top())]
        {
            mesh.vertices.push(Vertex { pos: pos2(px, py), uv: WHITE_UV, color: a });
        }
        mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
        x += period;
    }
    painter.add(mesh);
}

/// Ориентация индикатора уровня.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    /// Заполнение снизу вверх.
    Vertical,
}

/// Капсульный индикатор: дорожка `track` (радиус = половина толщины) и заливка `fill` (0…1),
/// цвет заливки в точке шкалы задаётся `color_at(t)`. Повторяет CSS `overflow:hidden` + градиент.
pub fn capsule_meter(
    painter: &Painter,
    rect: Rect,
    orientation: Orientation,
    fill: f32,
    track: Color32,
    color_at: impl Fn(f32) -> Color32,
) {
    // Прижимаем к физическим пикселям: при дробном масштабе (150%) край без сглаживания
    // у разных сегментов растеризовался бы по-разному и часть полосы «прыгала» на 1 px.
    let ppp = painter.pixels_per_point();
    let snap = |v: f32| (v * ppp).round() / ppp;
    let rect =
        Rect::from_min_max(pos2(snap(rect.left()), snap(rect.top())), pos2(snap(rect.right()), snap(rect.bottom())));
    let thickness = match orientation {
        Orientation::Horizontal => rect.height(),
        Orientation::Vertical => rect.width(),
    };
    let r = thickness / 2.0;
    painter.rect_filled(rect, r, track);
    let fill = fill.clamp(0.0, 1.0);
    if fill <= 0.0 {
        return;
    }
    let len = match orientation {
        Orientation::Horizontal => rect.width(),
        Orientation::Vertical => rect.height(),
    };
    let end = len * fill;
    // Сэмплы вдоль оси: мелкий шаг на скруглениях, плюс точки смены зон.
    let mut ts: Vec<f32> = Vec::with_capacity(48);
    let step = 0.5;
    let mut d = 0.0;
    while d < r.min(end) {
        ts.push(d);
        d += step;
    }
    for z in [0.70, 0.85] {
        let zd = z * len;
        if zd > r && zd < end {
            ts.push(zd);
        }
    }
    let mut d = (len - r).max(r);
    while d < end {
        ts.push(d);
        d += step;
    }
    ts.push(end);
    ts.sort_by(f32::total_cmp);
    ts.dedup_by(|a, b| (*a - *b).abs() < 0.01);

    let half_at = |d: f32| -> f32 {
        let edge = if d < r {
            r - d
        } else if d > len - r {
            d - (len - r)
        } else {
            0.0
        };
        (r * r - edge * edge).max(0.0).sqrt()
    };
    let mut mesh = Mesh::default();
    for &d in &ts {
        let half = half_at(d);
        let color = color_at(d / len);
        let (p0, p1) = match orientation {
            Orientation::Horizontal => {
                let c = rect.center().y;
                (pos2(rect.left() + d, c - half), pos2(rect.left() + d, c + half))
            }
            Orientation::Vertical => {
                let c = rect.center().x;
                (pos2(c - half, rect.bottom() - d), pos2(c + half, rect.bottom() - d))
            }
        };
        mesh.vertices.push(Vertex { pos: p0, uv: WHITE_UV, color });
        mesh.vertices.push(Vertex { pos: p1, uv: WHITE_UV, color });
    }
    for i in 0..(ts.len() as u32).saturating_sub(1) {
        let a = i * 2;
        mesh.indices.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
    }
    painter.add(mesh);
}

/// Доля шкалы −60…0 dBFS.
pub fn db_to_fraction(db: f32) -> f32 {
    ((db + 60.0) / 60.0).clamp(0.0, 1.0)
}

/// Кликабельная область без визуала. Возвращает ответ egui.
pub fn hit(ui: &Ui, rect: Rect, id: impl std::hash::Hash + std::fmt::Debug, sense: Sense) -> egui::Response {
    ui.interact(rect, ui.scope_id().with(id), sense)
}

/// Стиль кнопки.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Контур `line`, текст `fg`.
    Outline,
    /// Заливка `accent-live`, белый текст 600.
    Live,
    /// `primary-btn`: светлая заливка, тёмный текст 500.
    Primary,
}

/// Кнопка фиксированного прямоугольника с текстом по центру.
pub fn button(
    ui: &Ui,
    rect: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    kind: ButtonKind,
    enabled: bool,
    p: &Palette,
) -> egui::Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let resp = hit(ui, rect, id, sense);
    let painter = ui.painter();
    let (bg, fg, font) = match kind {
        ButtonKind::Outline => (Color32::TRANSPARENT, p.fg, theme::sans(theme::TEXT)),
        ButtonKind::Live => (p.live, Color32::WHITE, theme::sans_semibold(theme::TEXT)),
        ButtonKind::Primary => (p.primary_bg, p.primary_fg, theme::sans_medium(theme::TEXT)),
    };
    painter.rect_filled(rect, 4.0, bg);
    if kind == ButtonKind::Outline {
        border(painter, rect, 4.0, p.line);
    }
    if enabled && resp.hovered() {
        painter.rect_filled(rect, 4.0, p.hover);
    }
    if resp.has_focus() {
        painter.rect_stroke(rect.expand(1.0), 5.0, Stroke::new(1.0, p.mute), StrokeKind::Outside);
    }
    let fg = if enabled { fg } else { fg.gamma_multiply(0.55) };
    text(painter, rect.center(), Align2::CENTER_CENTER, label, font, fg);
    resp
}

/// Маленькая кнопка-бейдж (`M`, `⚙`, клавиши сцен): рамка `line`, радиус 3.
pub fn badge_button(
    ui: &Ui,
    rect: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    active_bg: Option<Color32>,
    p: &Palette,
) -> egui::Response {
    let resp = hit(ui, rect, id, Sense::click());
    let painter = ui.painter();
    if let Some(bg) = active_bg {
        painter.rect_filled(rect, 3.0, bg);
    }
    border(painter, rect, 3.0, p.line);
    if resp.hovered() {
        painter.rect_filled(rect, 3.0, p.hover);
    }
    resp
}

/// Иконки, которых нет в шрифтах интерфейса: рисуются векторно.
#[derive(Clone, Copy)]
pub enum Icon {
    Minimize,
    Maximize,
    Restore,
    Close,
    Expand,
    Gear,
    Stop,
    Plus,
}

pub fn icon(painter: &Painter, center: Pos2, icon: Icon, size: f32, color: Color32) {
    let s = size / 2.0;
    let stroke = Stroke::new(1.0, color);
    match icon {
        Icon::Minimize => {
            painter.line_segment([center + vec2(-s, 0.5), center + vec2(s, 0.5)], stroke);
        }
        Icon::Maximize => {
            painter.rect_stroke(Rect::from_center_size(center, Vec2::splat(size)), 1.0, stroke, StrokeKind::Inside);
        }
        Icon::Restore => {
            let r = Rect::from_center_size(center + vec2(-1.5, 1.5), Vec2::splat(size - 3.0));
            painter.rect_stroke(r, 1.0, stroke, StrokeKind::Inside);
            let o = vec2(3.0, -3.0);
            painter.line_segment([r.left_top() + o, r.right_top() + o], stroke);
            painter.line_segment([r.right_top() + o, r.right_bottom() + o], stroke);
            painter.line_segment([r.left_top() + o, r.left_top() + vec2(3.0, 0.0)], stroke);
        }
        Icon::Close => {
            painter.line_segment([center + vec2(-s, -s), center + vec2(s, s)], stroke);
            painter.line_segment([center + vec2(-s, s), center + vec2(s, -s)], stroke);
        }
        Icon::Expand => {
            // ⤢: диагональ со стрелками.
            painter.line_segment([center + vec2(-s, s), center + vec2(s, -s)], stroke);
            painter.line_segment([center + vec2(s, -s), center + vec2(s - 4.0, -s)], stroke);
            painter.line_segment([center + vec2(s, -s), center + vec2(s, -s + 4.0)], stroke);
            painter.line_segment([center + vec2(-s, s), center + vec2(-s + 4.0, s)], stroke);
            painter.line_segment([center + vec2(-s, s), center + vec2(-s, s - 4.0)], stroke);
        }
        Icon::Gear => {
            painter.circle_stroke(center, s * 0.45, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let d = vec2(a.cos(), a.sin());
                painter.line_segment([center + d * s * 0.62, center + d * s], Stroke::new(1.6, color));
            }
        }
        Icon::Stop => {
            painter.rect_filled(Rect::from_center_size(center, Vec2::splat(size)), 1.0, color);
        }
        Icon::Plus => {
            painter.line_segment([center + vec2(-s, 0.0), center + vec2(s, 0.0)], stroke);
            painter.line_segment([center + vec2(0.0, -s), center + vec2(0.0, s)], stroke);
        }
    }
}

/// Индикатор видимости: `◉` видим, `○` скрыт (11 px).
pub fn visibility_icon(painter: &Painter, center: Pos2, visible: bool, color: Color32) {
    painter.circle_stroke(center, 3.8, Stroke::new(1.0, color));
    if visible {
        painter.circle_filled(center, 2.0, color);
    }
}

/// Скруглённая подложка (бейдж `PROGRAM`).
pub fn badge(painter: &Painter, min: Pos2, s: &str, font: FontId, pad: Vec2, bg: Color32, fg: Color32) -> Rect {
    let g = galley(painter, s, font, fg);
    let rect = Rect::from_min_size(min, g.size() + pad * 2.0);
    painter.rect_filled(rect, CornerRadius::same(3), bg);
    painter.galley(min + pad, g, fg);
    rect
}
