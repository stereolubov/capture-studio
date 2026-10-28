//! Геометрия элементов сцены в координатах холста (§7.2): размеры с учётом обрезки и
//! поворота, «вписать / растянуть / по центру», привязка к краям и центру холста.
//!
//! Соглашение: `Transform::pos` — левый верхний угол элемента на холсте (после поворота),
//! `scale` применяется к обрезанному размеру источника до поворота.

use crate::model::{Crop, Rot90, Transform, Vec2};

/// Прямоугольник в пикселях холста.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CanvasRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl CanvasRect {
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

/// Размер источника после обрезки (до масштаба и поворота).
pub fn cropped_size(src: (u32, u32), crop: &Crop) -> (f32, f32) {
    let w = src.0.saturating_sub(crop.left + crop.right).max(1) as f32;
    let h = src.1.saturating_sub(crop.top + crop.bottom).max(1) as f32;
    (w, h)
}

fn rotated(rot: Rot90, w: f32, h: f32) -> (f32, f32) {
    match rot {
        Rot90::R0 | Rot90::R180 => (w, h),
        Rot90::R90 | Rot90::R270 => (h, w),
    }
}

/// Прямоугольник элемента на холсте.
pub fn item_rect(t: &Transform, crop: &Crop, src: (u32, u32)) -> CanvasRect {
    let (w, h) = cropped_size(src, crop);
    let (w, h) = rotated(t.rotation, w * t.scale.x.abs(), h * t.scale.y.abs());
    CanvasRect { x: t.pos.x, y: t.pos.y, w, h }
}

/// «Вписать в холст»: максимальный масштаб с сохранением пропорций, по центру.
pub fn fit(t: &Transform, crop: &Crop, src: (u32, u32), canvas: (u32, u32)) -> Transform {
    let (w, h) = cropped_size(src, crop);
    let (rw, rh) = rotated(t.rotation, w, h);
    let k = (canvas.0 as f32 / rw).min(canvas.1 as f32 / rh);
    let mut out = *t;
    out.scale = Vec2 { x: k, y: k };
    center(&out, crop, src, canvas)
}

/// «Растянуть на холст»: без сохранения пропорций.
pub fn stretch(t: &Transform, crop: &Crop, src: (u32, u32), canvas: (u32, u32)) -> Transform {
    let (w, h) = cropped_size(src, crop);
    let (cw, ch) = rotated(t.rotation, canvas.0 as f32, canvas.1 as f32);
    let mut out = *t;
    out.scale = Vec2 { x: cw / w, y: ch / h };
    out.pos = Vec2 { x: 0.0, y: 0.0 };
    out
}

/// «По центру» холста без изменения масштаба.
pub fn center(t: &Transform, crop: &Crop, src: (u32, u32), canvas: (u32, u32)) -> Transform {
    let r = item_rect(t, crop, src);
    let mut out = *t;
    out.pos = Vec2 { x: ((canvas.0 as f32 - r.w) / 2.0).round(), y: ((canvas.1 as f32 - r.h) / 2.0).round() };
    out
}

/// Поворот на 90° по часовой стрелке вокруг центра элемента.
pub fn rotate_cw(t: &Transform, crop: &Crop, src: (u32, u32)) -> Transform {
    let before = item_rect(t, crop, src);
    let mut out = *t;
    out.rotation = match t.rotation {
        Rot90::R0 => Rot90::R90,
        Rot90::R90 => Rot90::R180,
        Rot90::R180 => Rot90::R270,
        Rot90::R270 => Rot90::R0,
    };
    let after = item_rect(&out, crop, src);
    out.pos =
        Vec2 { x: (before.x + (before.w - after.w) / 2.0).round(), y: (before.y + (before.h - after.h) / 2.0).round() };
    out
}

/// Смещение для привязки прямоугольника к краям и центру холста (порог в пикселях холста).
/// Возвращает поправку `(dx, dy)` и линии привязки (для подсветки в превью).
pub fn snap(r: &CanvasRect, canvas: (u32, u32), threshold: f32) -> ((f32, f32), SnapLines) {
    let (cw, ch) = (canvas.0 as f32, canvas.1 as f32);
    let best = |edges: [f32; 3], targets: [f32; 3]| -> Option<(f32, f32)> {
        let mut best: Option<(f32, f32)> = None;
        for e in edges {
            for t in targets {
                let d = t - e;
                if d.abs() <= threshold && best.is_none_or(|(bd, _)| d.abs() < bd.abs()) {
                    best = Some((d, t));
                }
            }
        }
        best
    };
    let x = best([r.x, r.x + r.w / 2.0, r.right()], [0.0, cw / 2.0, cw]);
    let y = best([r.y, r.y + r.h / 2.0, r.bottom()], [0.0, ch / 2.0, ch]);
    (
        (x.map_or(0.0, |v| v.0), y.map_or(0.0, |v| v.0)),
        SnapLines { vertical: x.map(|v| v.1), horizontal: y.map(|v| v.1) },
    )
}

/// Линии, к которым произошла привязка (координаты холста).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SnapLines {
    pub vertical: Option<f32>,
    pub horizontal: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: (u32, u32) = (1280, 720);
    const CANVAS: (u32, u32) = (1920, 1080);

    #[test]
    fn rect_with_crop_scale_rotation() {
        let mut t = Transform::default();
        let crop = Crop { left: 80, right: 0, top: 0, bottom: 20 };
        t.scale = Vec2 { x: 0.5, y: 0.5 };
        let r = item_rect(&t, &crop, SRC);
        assert_eq!((r.w, r.h), (600.0, 350.0));
        t.rotation = Rot90::R90;
        let r = item_rect(&t, &crop, SRC);
        assert_eq!((r.w, r.h), (350.0, 600.0));
    }

    #[test]
    fn fit_and_stretch() {
        let t = fit(&Transform::default(), &Crop::default(), (1000, 1000), CANVAS);
        let r = item_rect(&t, &Crop::default(), (1000, 1000));
        assert_eq!((r.x, r.y, r.w, r.h), (420.0, 0.0, 1080.0, 1080.0));
        let t = stretch(&Transform::default(), &Crop::default(), SRC, CANVAS);
        let r = item_rect(&t, &Crop::default(), SRC);
        assert_eq!((r.w, r.h), (1920.0, 1080.0));
    }

    #[test]
    fn rotation_keeps_center() {
        let t = center(&Transform::default(), &Crop::default(), SRC, CANVAS);
        let r0 = item_rect(&t, &Crop::default(), SRC);
        let t2 = rotate_cw(&t, &Crop::default(), SRC);
        let r1 = item_rect(&t2, &Crop::default(), SRC);
        assert_eq!(r0.x + r0.w / 2.0, r1.x + r1.w / 2.0);
        assert_eq!(r0.y + r0.h / 2.0, r1.y + r1.h / 2.0);
        assert_eq!(
            rotate_cw(&rotate_cw(&rotate_cw(&t2, &Crop::default(), SRC), &Crop::default(), SRC), &Crop::default(), SRC)
                .rotation,
            Rot90::R0
        );
    }

    #[test]
    fn snapping() {
        let r = CanvasRect { x: 6.0, y: 400.0, w: 100.0, h: 100.0 };
        let ((dx, dy), lines) = snap(&r, CANVAS, 10.0);
        assert_eq!(dx, -6.0);
        assert_eq!(dy, 0.0);
        assert_eq!(lines.vertical, Some(0.0));
        // Центр по вертикали: 489 + 50 = 539 → 540.
        let r = CanvasRect { x: 500.0, y: 489.0, w: 100.0, h: 100.0 };
        let ((_, dy), lines) = snap(&r, CANVAS, 10.0);
        assert_eq!(dy, 1.0);
        assert_eq!(lines.horizontal, Some(540.0));
    }
}
