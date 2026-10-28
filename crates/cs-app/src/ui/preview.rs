//! Превью сцены (§7.2, §11.2): текстура холста без копий, выделение и трансформации мышью.
//!
//! * Клик — выбрать верхний элемент под курсором; перетаскивание — переместить.
//! * Углы — масштаб с сохранением пропорций (Shift — без сохранения).
//! * Стороны — растяжение по оси; Alt + сторона — обрезка.
//! * Привязка к краям и центру холста с порогом 10 px экрана.
//! * Стрелки — сдвиг на 1 px (Shift — 10 px), Delete — удалить, ПКМ — контекстное меню.
//!
//! Жест мышью — один шаг отмены: в начале отправляется `Checkpoint`, промежуточные
//! трансформации — с `record: false`.

use super::App;
use super::widgets as w;
use crate::theme::Palette;
use cs_core::Command;
use cs_core::geometry::{self, CanvasRect, SnapLines};
use cs_core::model::{Crop, Rot90, SourceId, Transform, Vec2};
use cs_core::state::{ItemAction, SourceView};
use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

const SNAP_PX: f32 = 10.0;
const HANDLE: f32 = 7.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Handle {
    Move,
    /// Угол: 0 TL, 1 TR, 2 BR, 3 BL.
    Corner(u8),
    /// Сторона: 0 верх, 1 право, 2 низ, 3 лево.
    Edge(u8),
    /// Alt + сторона — обрезка.
    Crop(u8),
}

struct Drag {
    handle: Handle,
    item: usize,
    start_pointer: Pos2,
    start_t: Transform,
    start_crop: Crop,
    size: (u32, u32),
    current: (Transform, Crop),
}

/// Состояние превью между кадрами.
#[derive(Default)]
pub struct PreviewUi {
    pub selected: Option<SourceId>,
    drag: Option<Drag>,
    snap: SnapLines,
}

/// Отображение холста в прямоугольник экрана с сохранением пропорций.
#[derive(Clone, Copy)]
struct View {
    disp: Rect,
    k: f32,
}

impl View {
    fn new(area: Rect, canvas: (u32, u32)) -> Self {
        let (cw, ch) = (canvas.0.max(1) as f32, canvas.1.max(1) as f32);
        let k = (area.width() / cw).min(area.height() / ch);
        let size = vec2(cw * k, ch * k);
        let min = area.center() - size / 2.0;
        Self { disp: Rect::from_min_size(pos2(min.x.round(), min.y.round()), size), k }
    }
    fn to_screen(self, x: f32, y: f32) -> Pos2 {
        self.disp.min + vec2(x, y) * self.k
    }
    fn to_canvas(self, p: Pos2) -> Pos2 {
        ((p - self.disp.min) / self.k).to_pos2()
    }
    fn rect(self, r: &CanvasRect) -> Rect {
        Rect::from_min_max(self.to_screen(r.x, r.y), self.to_screen(r.right(), r.bottom()))
    }
}

fn rect_of(t: &Transform, crop: &Crop, size: (u32, u32)) -> CanvasRect {
    geometry::item_rect(t, crop, size)
}

/// Устанавливает отображаемый размер элемента (с учётом поворота на 90°).
fn set_display_size(t: &mut Transform, crop: &Crop, size: (u32, u32), w: f32, h: f32) {
    let (cw, ch) = geometry::cropped_size(size, crop);
    match t.rotation {
        Rot90::R0 | Rot90::R180 => t.scale = Vec2 { x: w / cw, y: h / ch },
        Rot90::R90 | Rot90::R270 => t.scale = Vec2 { x: h / cw, y: w / ch },
    }
}

fn handles(r: Rect) -> [(Handle, Pos2); 8] {
    let c = r.center();
    [
        (Handle::Corner(0), r.left_top()),
        (Handle::Corner(1), r.right_top()),
        (Handle::Corner(2), r.right_bottom()),
        (Handle::Corner(3), r.left_bottom()),
        (Handle::Edge(0), pos2(c.x, r.top())),
        (Handle::Edge(1), pos2(r.right(), c.y)),
        (Handle::Edge(2), pos2(c.x, r.bottom())),
        (Handle::Edge(3), pos2(r.left(), c.y)),
    ]
}

fn cursor_for(h: Handle) -> CursorIcon {
    match h {
        Handle::Move => CursorIcon::Move,
        Handle::Corner(0 | 2) => CursorIcon::ResizeNwSe,
        Handle::Corner(_) => CursorIcon::ResizeNeSw,
        Handle::Edge(0 | 2) => CursorIcon::ResizeVertical,
        Handle::Edge(_) => CursorIcon::ResizeHorizontal,
        Handle::Crop(_) => CursorIcon::Crosshair,
    }
}

/// Новая трансформация по ходу жеста.
fn drag_result(
    d: &Drag,
    pointer: Pos2,
    shift: bool,
    canvas: (u32, u32),
    snap_threshold: f32,
) -> ((Transform, Crop), SnapLines) {
    let r0 = rect_of(&d.start_t, &d.start_crop, d.size);
    let mut t = d.start_t;
    let mut crop = d.start_crop;
    let delta = pointer - d.start_pointer;
    let mut lines = SnapLines::default();
    match d.handle {
        Handle::Move => {
            t.pos = Vec2 { x: (d.start_t.pos.x + delta.x).round(), y: (d.start_t.pos.y + delta.y).round() };
            let r = rect_of(&t, &crop, d.size);
            let ((dx, dy), l) = geometry::snap(&r, canvas, snap_threshold);
            t.pos.x += dx;
            t.pos.y += dy;
            lines = l;
        }
        Handle::Corner(c) => {
            // Противоположный угол неподвижен.
            let anchor = match c {
                0 => pos2(r0.right(), r0.bottom()),
                1 => pos2(r0.x, r0.bottom()),
                2 => pos2(r0.x, r0.y),
                _ => pos2(r0.right(), r0.y),
            };
            let mut w = (pointer.x - anchor.x).abs().max(4.0);
            let mut h = (pointer.y - anchor.y).abs().max(4.0);
            if !shift {
                let k = (w / r0.w).max(h / r0.h);
                w = r0.w * k;
                h = r0.h * k;
            }
            let x = if matches!(c, 0 | 3) { anchor.x - w } else { anchor.x };
            let y = if matches!(c, 0 | 1) { anchor.y - h } else { anchor.y };
            set_display_size(&mut t, &crop, d.size, w.round(), h.round());
            t.pos = Vec2 { x: x.round(), y: y.round() };
        }
        Handle::Edge(e) => {
            let (mut x0, mut y0, mut x1, mut y1) = (r0.x, r0.y, r0.right(), r0.bottom());
            match e {
                0 => y0 = pointer.y.min(y1 - 4.0),
                1 => x1 = pointer.x.max(x0 + 4.0),
                2 => y1 = pointer.y.max(y0 + 4.0),
                _ => x0 = pointer.x.min(x1 - 4.0),
            }
            set_display_size(&mut t, &crop, d.size, (x1 - x0).round(), (y1 - y0).round());
            t.pos = Vec2 { x: x0.round(), y: y0.round() };
        }
        Handle::Crop(e) => {
            // Обрезка мышью — только без поворота; пиксели источника = экранные / масштаб.
            if t.rotation == Rot90::R0 {
                let (sx, sy) = (t.scale.x.abs().max(0.001), t.scale.y.abs().max(0.001));
                let (sw, sh) = (d.size.0 as f32, d.size.1 as f32);
                let clamp_x = |v: f32, other: u32| v.clamp(0.0, (sw - other as f32 - 2.0).max(0.0)).round() as u32;
                let clamp_y = |v: f32, other: u32| v.clamp(0.0, (sh - other as f32 - 2.0).max(0.0)).round() as u32;
                match e {
                    0 => {
                        crop.top = clamp_y(d.start_crop.top as f32 + delta.y / sy, crop.bottom);
                        t.pos.y = d.start_t.pos.y + (crop.top as f32 - d.start_crop.top as f32) * sy;
                    }
                    1 => crop.right = clamp_x(d.start_crop.right as f32 - delta.x / sx, crop.left),
                    2 => crop.bottom = clamp_y(d.start_crop.bottom as f32 - delta.y / sy, crop.top),
                    _ => {
                        crop.left = clamp_x(d.start_crop.left as f32 + delta.x / sx, crop.right);
                        t.pos.x = d.start_t.pos.x + (crop.left as f32 - d.start_crop.left as f32) * sx;
                    }
                }
            }
        }
    }
    ((t, crop), lines)
}

/// Контекстное меню элемента (общее для превью и списка источников).
pub fn item_menu(app: &mut App, ui: &mut Ui, src: &SourceView) {
    let tr: std::collections::HashMap<&str, String> = [
        "source-properties",
        "item-fit",
        "item-stretch",
        "item-center",
        "item-rotate",
        "item-reset",
        "item-up",
        "item-down",
        "item-lock",
        "item-unlock",
        "item-remove",
    ]
    .into_iter()
    .map(|k| (k, app.i18n.t(k)))
    .collect();
    let t = |k: &str| tr.get(k).cloned().unwrap_or_default();
    if ui.button(t("source-properties")).clicked() {
        app.open_source_dialog(Some(src.clone()));
        ui.close();
    }
    ui.separator();
    for (key, action) in [
        ("item-fit", ItemAction::Fit),
        ("item-stretch", ItemAction::Stretch),
        ("item-center", ItemAction::Center),
        ("item-rotate", ItemAction::RotateCw),
        ("item-reset", ItemAction::Reset),
    ] {
        if ui.button(t(key)).clicked() {
            app.send(Command::ItemAction { item: src.item, action });
            ui.close();
        }
    }
    ui.separator();
    let n = app.engine.state().sources.len();
    if src.item + 1 < n && ui.button(t("item-up")).clicked() {
        app.send(Command::MoveItem { from: src.item, to: src.item + 1 });
        ui.close();
    }
    if src.item > 0 && ui.button(t("item-down")).clicked() {
        app.send(Command::MoveItem { from: src.item, to: src.item - 1 });
        ui.close();
    }
    let lock_key = if src.locked { "item-unlock" } else { "item-lock" };
    if ui.button(t(lock_key)).clicked() {
        app.send(Command::ToggleItemLocked(src.item));
        ui.close();
    }
    if ui.button(t("item-remove")).clicked() {
        app.send(Command::RemoveItem(src.item));
        ui.close();
    }
}

/// Рисует превью в `area`. Возвращает `true`, если превью показано (иначе — заглушка).
pub fn draw(app: &mut App, ui: &mut Ui, p: &Palette, area: Rect) -> bool {
    let Some(tex) = app.preview_texture else { return false };
    let st = app.engine.state().clone();
    let Some(info) = st.preview else { return false };
    let canvas = (info.width, info.height);
    let painter = ui.painter().clone();
    let view = View::new(area.shrink(1.0), canvas);

    painter.rect_filled(area.shrink(1.0), 3.0, p.bg);
    if st.preview_enabled {
        painter.image(tex, view.disp, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    } else {
        w::hatch(&painter, view.disp, p.panel, p.bg);
    }
    painter.rect_stroke(view.disp, 0.0, Stroke::new(1.0, p.line), StrokeKind::Outside);

    let resp = ui.interact(area, ui.scope_id().with("preview"), Sense::click_and_drag());
    // Текущая позиция и точка нажатия: жест отсчитывается от нажатия, а не от момента,
    // когда egui распознал перетаскивание (иначе первый шаг «дёргается»).
    let pointer = ui.input(|i| i.pointer.latest_pos());
    let press_origin = ui.input(|i| i.pointer.press_origin());
    let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
    let ui_state = &mut app.preview_ui;

    // Выбранный элемент (по id — переживает изменение порядка).
    let selected = ui_state.selected.and_then(|id| st.sources.iter().find(|s| s.id == id)).cloned();
    if ui_state.selected.is_some() && selected.is_none() {
        ui_state.selected = None;
    }

    // Что под курсором: маркер выделенного или верхний видимый элемент.
    let hover_handle = |pos: Pos2| -> Option<Handle> {
        let sel = selected.as_ref()?;
        if sel.locked || sel.size.0 == 0 {
            return None;
        }
        let r = view.rect(&rect_of(&sel.transform, &sel.crop, sel.size));
        for (h, c) in handles(r) {
            if (pos - c).length() <= HANDLE {
                return Some(match (h, alt) {
                    (Handle::Edge(e), true) => Handle::Crop(e),
                    _ => h,
                });
            }
        }
        r.contains(pos).then_some(Handle::Move)
    };
    let item_at = |pos: Pos2| -> Option<&SourceView> {
        let c = view.to_canvas(pos);
        st.sources
            .iter()
            .find(|s| s.visible && s.size.0 > 0 && rect_of(&s.transform, &s.crop, s.size).contains(c.x, c.y))
    };

    if let Some(pos) = pointer.filter(|p| area.contains(*p))
        && ui_state.drag.is_none()
        && let Some(h) = hover_handle(pos)
    {
        ui.ctx().set_cursor_icon(cursor_for(h));
    }

    // Начало жеста.
    if resp.drag_started()
        && let Some(pos) = press_origin.or(resp.interact_pointer_pos())
    {
        let mut handle = hover_handle(pos);
        if handle.is_none() {
            let hit = item_at(pos).cloned();
            ui_state.selected = hit.as_ref().map(|s| s.id);
            handle = hit.as_ref().filter(|s| !s.locked).map(|_| Handle::Move);
            if let Some(s) = hit {
                start_drag(ui_state, &s, handle, view.to_canvas(pos));
            }
        } else if let Some(s) = &selected {
            start_drag(ui_state, s, handle, view.to_canvas(pos));
        }
        if ui_state.drag.is_some() {
            app.engine.send(Command::Checkpoint);
        }
    }
    let ui_state = &mut app.preview_ui;
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
        && hover_handle(pos).is_none()
    {
        ui_state.selected = item_at(pos).map(|s| s.id);
    }
    if resp.secondary_clicked()
        && let Some(pos) = resp.interact_pointer_pos()
        && let Some(s) = item_at(pos)
    {
        ui_state.selected = Some(s.id);
    }

    // Ход жеста.
    let mut send = None;
    if let Some(d) = &mut ui_state.drag {
        if let Some(pos) = pointer {
            let (res, lines) = drag_result(d, view.to_canvas(pos), shift, canvas, SNAP_PX / view.k);
            if res != d.current {
                d.current = res;
                send = Some(Command::SetItemTransform { item: d.item, transform: res.0, crop: res.1, record: false });
            }
            ui_state.snap = lines;
            ui.ctx().set_cursor_icon(cursor_for(d.handle));
        }
        if !resp.dragged() {
            ui_state.drag = None;
            ui_state.snap = SnapLines::default();
        }
    }
    if let Some(cmd) = send {
        app.engine.send(cmd);
    }

    // Рамка выделения: во время жеста — по локальной трансформации (без задержки снимка).
    let ui_state = &app.preview_ui;
    if let Some(sel) = &selected {
        let (t, crop) = ui_state.drag.as_ref().map_or((sel.transform, sel.crop), |d| d.current);
        if sel.size.0 > 0 {
            let r = view.rect(&rect_of(&t, &crop, sel.size));
            let color = if sel.locked { p.mute } else { p.live };
            painter.rect_stroke(r, 0.0, Stroke::new(1.0, color), StrokeKind::Outside);
            if !sel.locked {
                for (_, c) in handles(r) {
                    let h = Rect::from_center_size(c, vec2(HANDLE, HANDLE));
                    painter.rect_filled(h, 1.0, p.fg);
                    painter.rect_stroke(h, 1.0, Stroke::new(1.0, color), StrokeKind::Inside);
                }
            }
        }
    }
    // Линии привязки.
    let painter = painter.with_clip_rect(view.disp);
    if let Some(x) = ui_state.snap.vertical {
        let sx = view.to_screen(x, 0.0).x;
        painter.line_segment([pos2(sx, view.disp.top()), pos2(sx, view.disp.bottom())], Stroke::new(1.0, p.warn));
    }
    if let Some(y) = ui_state.snap.horizontal {
        let sy = view.to_screen(0.0, y).y;
        painter.line_segment([pos2(view.disp.left(), sy), pos2(view.disp.right(), sy)], Stroke::new(1.0, p.warn));
    }

    // Контекстное меню выбранного элемента.
    if let Some(sel) = selected.clone() {
        resp.context_menu(|ui| item_menu(app, ui, &sel));
        keyboard(app, ui, &sel);
    }
    true
}

fn start_drag(ui_state: &mut PreviewUi, s: &SourceView, handle: Option<Handle>, pointer: Pos2) {
    ui_state.drag = handle.filter(|_| s.size.0 > 0).map(|handle| Drag {
        handle,
        item: s.item,
        start_pointer: pointer,
        start_t: s.transform,
        start_crop: s.crop,
        size: s.size,
        current: (s.transform, s.crop),
    });
}

/// Стрелки — сдвиг, Delete — удаление выбранного элемента.
fn keyboard(app: &mut App, ui: &Ui, sel: &SourceView) {
    if ui.ctx().egui_wants_keyboard_input() || app.modal_open() {
        return;
    }
    let (dx, dy, del, step) = ui.input(|i| {
        let step = if i.modifiers.shift { 10.0 } else { 1.0 };
        let mut dx = 0.0;
        let mut dy = 0.0;
        if i.key_pressed(egui::Key::ArrowLeft) {
            dx -= 1.0;
        }
        if i.key_pressed(egui::Key::ArrowRight) {
            dx += 1.0;
        }
        if i.key_pressed(egui::Key::ArrowUp) {
            dy -= 1.0;
        }
        if i.key_pressed(egui::Key::ArrowDown) {
            dy += 1.0;
        }
        (dx, dy, i.key_pressed(egui::Key::Delete), step)
    });
    if del {
        app.send(Command::RemoveItem(sel.item));
        app.preview_ui.selected = None;
    } else if (dx != 0.0 || dy != 0.0) && !sel.locked {
        let mut t = sel.transform;
        t.pos.x += dx * step;
        t.pos.y += dy * step;
        app.send(Command::SetItemTransform { item: sel.item, transform: t, crop: sel.crop, record: true });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drag(handle: Handle) -> Drag {
        let t = Transform { pos: Vec2 { x: 100.0, y: 100.0 }, ..Transform::default() };
        Drag {
            handle,
            item: 0,
            start_pointer: pos2(0.0, 0.0),
            start_t: t,
            start_crop: Crop::default(),
            size: (200, 100),
            current: (t, Crop::default()),
        }
    }

    #[test]
    fn corner_keeps_aspect_without_shift() {
        let d = drag(Handle::Corner(2));
        // Тянем правый нижний угол до (500, 250): по ширине ×2, по высоте ×1.5 → ×2.
        let ((t, _), _) = drag_result(&d, pos2(500.0, 250.0), false, (1920, 1080), 0.0);
        let r = rect_of(&t, &Crop::default(), (200, 100));
        assert_eq!((r.x, r.y, r.w, r.h), (100.0, 100.0, 400.0, 200.0));
        let ((t, _), _) = drag_result(&d, pos2(500.0, 250.0), true, (1920, 1080), 0.0);
        let r = rect_of(&t, &Crop::default(), (200, 100));
        assert_eq!((r.w, r.h), (400.0, 150.0));
    }

    #[test]
    fn move_snaps_to_edge() {
        let mut d = drag(Handle::Move);
        d.start_pointer = pos2(150.0, 150.0);
        // Сдвиг на −94 по X: левый край на 6 px от края холста → привязка к 0.
        let ((t, _), lines) = drag_result(&d, pos2(56.0, 150.0), false, (1920, 1080), 10.0);
        assert_eq!(t.pos.x, 0.0);
        assert_eq!(lines.vertical, Some(0.0));
    }

    #[test]
    fn crop_left_keeps_content_in_place() {
        let mut d = drag(Handle::Crop(3));
        d.start_pointer = pos2(100.0, 150.0);
        let ((t, crop), _) = drag_result(&d, pos2(140.0, 150.0), false, (1920, 1080), 0.0);
        assert_eq!(crop.left, 40);
        assert_eq!(t.pos.x, 140.0);
        let r = rect_of(&t, &crop, (200, 100));
        assert_eq!(r.right(), 300.0);
    }
}
