//! Окно добавления и свойств источника (M1): экран, окно, изображение, цвет, текст.

use super::widgets::{self as w, ButtonKind};
use super::{App, centered};
use crate::theme;
use cs_capture::enumerate::{self, MonitorInfo, WindowInfo};
use cs_core::Command;
use cs_core::model::{MonitorMethod, SourceId, SourceKind};
use cs_core::state::SourceView;
use egui::{Area, Color32, Context, Id, Order, Rect, Sense, vec2};

/// Типы источников, доступные для добавления (остальные — будущие этапы).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewKind {
    Monitor,
    Window,
    Image,
    Color,
    Text,
}

pub const ADDABLE: [(NewKind, &str); 5] = [
    (NewKind::Monitor, "kind-monitor"),
    (NewKind::Window, "kind-window"),
    (NewKind::Image, "kind-image"),
    (NewKind::Color, "kind-color"),
    (NewKind::Text, "kind-text"),
];

/// Типы будущих этапов — показываются в меню неактивными.
pub const LATER: [&str; 3] = ["kind-game", "kind-camera", "kind-browser"];

pub struct SourceDialog {
    editing: Option<SourceId>,
    name: String,
    kind: SourceKind,
    monitors: Vec<MonitorInfo>,
    windows: Vec<WindowInfo>,
    fonts: Vec<String>,
}

fn default_kind(k: NewKind) -> SourceKind {
    match k {
        NewKind::Monitor => {
            SourceKind::MonitorCapture { monitor_index: 0, capture_cursor: true, method: MonitorMethod::Auto }
        }
        NewKind::Window => {
            SourceKind::WindowCapture { target_exe: String::new(), window_class: String::new(), capture_cursor: true }
        }
        NewKind::Image => SourceKind::Image { path: String::new() },
        NewKind::Color => SourceKind::Color { rgba: [237, 74, 73, 255], width: 1920, height: 1080 },
        NewKind::Text => SourceKind::Text {
            text: String::new(),
            font: "Segoe UI".into(),
            size: 64.0,
            rgba: [255, 255, 255, 255],
            outline_rgba: [0, 0, 0, 255],
            outline_width: 0.0,
        },
    }
}

impl SourceDialog {
    fn load_lists(&mut self) {
        match &self.kind {
            SourceKind::MonitorCapture { .. } => self.monitors = enumerate::monitors(),
            SourceKind::WindowCapture { .. } => self.windows = enumerate::windows(),
            SourceKind::Text { .. } => {
                self.fonts = cs_gpu::TextRenderer::new().map(|r| r.system_fonts()).unwrap_or_default();
            }
            _ => {}
        }
    }

    pub fn new_source(app: &App, k: NewKind) -> Self {
        let key = ADDABLE.iter().find(|(x, _)| *x == k).map_or("kind-color", |(_, key)| key);
        let mut d = Self {
            editing: None,
            name: app.i18n.t(key),
            kind: default_kind(k),
            monitors: vec![],
            windows: vec![],
            fonts: vec![],
        };
        d.load_lists();
        d
    }

    pub fn edit(src: &SourceView) -> Option<Self> {
        let kind = src.kind.clone()?;
        let mut d = Self {
            editing: Some(src.id),
            name: src.name.clone(),
            kind,
            monitors: vec![],
            windows: vec![],
            fonts: vec![],
        };
        d.load_lists();
        Some(d)
    }
}

fn label(ui: &mut egui::Ui, text: String, mute: Color32) {
    ui.label(egui::RichText::new(text).color(mute));
}

/// Поля свойств. Возвращает `true`, если конфигурация готова к применению.
fn fields(app: &App, ui: &mut egui::Ui, d: &mut SourceDialog) -> bool {
    let p = app.palette();
    let t = |k: &str| app.i18n.t(k);
    let mut ready = true;
    egui::Grid::new("source-fields").num_columns(2).spacing(vec2(12.0, 10.0)).min_col_width(140.0).show(ui, |ui| {
        label(ui, t("field-name"), p.mute);
        ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(300.0));
        ui.end_row();
        match &mut d.kind {
            SourceKind::MonitorCapture { monitor_index, capture_cursor, method } => {
                label(ui, t("field-monitor"), p.mute);
                let name = |m: &MonitorInfo| {
                    let mut s = app.i18n.t_args(
                        "meta-monitor",
                        &[
                            ("n", crate::i18n::s(m.index + 1)),
                            ("w", crate::i18n::s(m.width)),
                            ("h", crate::i18n::s(m.height)),
                        ],
                    );
                    if m.primary {
                        s.push_str(&format!(" · {}", t("monitor-primary")));
                    }
                    s
                };
                let current = d.monitors.iter().find(|m| m.index == *monitor_index).map(name).unwrap_or_default();
                egui::ComboBox::from_id_salt("monitor").width(300.0).selected_text(current).show_ui(ui, |ui| {
                    for m in &d.monitors {
                        ui.selectable_value(monitor_index, m.index, name(m));
                    }
                });
                ui.end_row();
                label(ui, t("field-capture-method"), p.mute);
                egui::ComboBox::from_id_salt("method")
                    .width(300.0)
                    .selected_text(match method {
                        MonitorMethod::Auto => t("method-auto"),
                        MonitorMethod::Wgc => "Windows Graphics Capture".into(),
                        MonitorMethod::Dxgi => "DXGI Desktop Duplication".into(),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(method, MonitorMethod::Auto, t("method-auto"));
                        ui.selectable_value(method, MonitorMethod::Wgc, "Windows Graphics Capture");
                        ui.selectable_value(method, MonitorMethod::Dxgi, "DXGI Desktop Duplication");
                    });
                ui.end_row();
                label(ui, t("field-cursor"), p.mute);
                ui.checkbox(capture_cursor, "");
                ui.end_row();
                ready = !d.monitors.is_empty();
            }
            SourceKind::WindowCapture { target_exe, window_class, capture_cursor } => {
                label(ui, t("field-window"), p.mute);
                ui.horizontal(|ui| {
                    let current = d
                        .windows
                        .iter()
                        .find(|w| w.exe.eq_ignore_ascii_case(target_exe) && w.class == *window_class)
                        .map(|w| format!("{} — {}", w.exe, w.title))
                        .unwrap_or_else(|| {
                            if target_exe.is_empty() {
                                t("window-choose")
                            } else {
                                format!("{target_exe} ({})", t("meta-waiting"))
                            }
                        });
                    egui::ComboBox::from_id_salt("window").width(260.0).selected_text(current).show_ui(ui, |ui| {
                        for win in &d.windows {
                            let text = format!("{} — {}", win.exe, win.title);
                            if ui.selectable_label(win.exe == *target_exe && win.class == *window_class, text).clicked()
                            {
                                *target_exe = win.exe.clone();
                                *window_class = win.class.clone();
                            }
                        }
                    });
                    if ui.button("⟳").on_hover_text(t("refresh")).clicked() {
                        d.windows = enumerate::windows();
                    }
                });
                ui.end_row();
                label(ui, t("field-cursor"), p.mute);
                ui.checkbox(capture_cursor, "");
                ui.end_row();
                ready = !target_exe.is_empty();
            }
            SourceKind::Image { path } => {
                label(ui, t("field-file"), p.mute);
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(path).desired_width(220.0));
                    if ui.button(t("browse")).clicked()
                        && let Some(f) = crate::platform::dialogs::open_image(&t("kind-image"), &t("image-files"))
                    {
                        *path = f;
                    }
                });
                ui.end_row();
                ready = !path.trim().is_empty();
            }
            SourceKind::Color { rgba, width, height } => {
                label(ui, t("field-color"), p.mute);
                let mut c = Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
                if ui.color_edit_button_srgba(&mut c).changed() {
                    *rgba = c.to_srgba_unmultiplied();
                }
                ui.end_row();
                label(ui, t("field-size"), p.mute);
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(width).range(1..=8192).suffix(" px"));
                    ui.label("×");
                    ui.add(egui::DragValue::new(height).range(1..=8192).suffix(" px"));
                });
                ui.end_row();
            }
            SourceKind::Text { text, font, size, rgba, outline_rgba, outline_width } => {
                label(ui, t("field-text"), p.mute);
                ui.add(egui::TextEdit::multiline(text).desired_width(300.0).desired_rows(3));
                ui.end_row();
                label(ui, t("field-font"), p.mute);
                egui::ComboBox::from_id_salt("font").width(300.0).selected_text(font.as_str()).height(320.0).show_ui(
                    ui,
                    |ui| {
                        for f in &d.fonts {
                            ui.selectable_value(font, f.clone(), f);
                        }
                    },
                );
                ui.end_row();
                label(ui, t("field-font-size"), p.mute);
                ui.add(egui::DragValue::new(size).range(4.0..=512.0).suffix(" px"));
                ui.end_row();
                label(ui, t("field-color"), p.mute);
                let mut c = Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
                if ui.color_edit_button_srgba(&mut c).changed() {
                    *rgba = c.to_srgba_unmultiplied();
                }
                ui.end_row();
                label(ui, t("field-outline"), p.mute);
                ui.horizontal(|ui| {
                    let mut c = Color32::from_rgba_unmultiplied(
                        outline_rgba[0],
                        outline_rgba[1],
                        outline_rgba[2],
                        outline_rgba[3],
                    );
                    if ui.color_edit_button_srgba(&mut c).changed() {
                        *outline_rgba = c.to_srgba_unmultiplied();
                    }
                    ui.add(egui::DragValue::new(outline_width).range(0.0..=20.0).speed(0.2).suffix(" px"));
                });
                ui.end_row();
                ready = !text.trim().is_empty();
            }
            _ => {}
        }
    });
    ready
}

/// Модальное окно. Возвращает `false`, когда окно закрыто.
pub fn modal(app: &mut App, ctx: &Context, d: &mut SourceDialog, screen: Rect) -> bool {
    let p = app.palette();
    Area::new(Id::unique("source-dim")).fade_in(false).order(Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        let (r, _) = ui.allocate_exact_size(screen.size(), Sense::click());
        ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha(if p.dark { 110 } else { 70 }));
    });
    let size = vec2(520.0, d.last_height());
    let rect = centered(screen, size);
    let mut keep = true;
    let mut apply = false;
    Area::new(Id::unique("source-dialog")).fade_in(false).order(Order::Foreground).fixed_pos(rect.min).show(
        ctx,
        |ui| {
            let frame = egui::Frame::NONE
                .fill(p.bg)
                .stroke(egui::Stroke::new(1.0, p.line))
                .corner_radius(8)
                .inner_margin(egui::Margin::symmetric(24, 20));
            frame.show(ui, |ui| {
                ui.set_width(size.x - 48.0);
                let title =
                    if d.editing.is_some() { app.i18n.t("source-properties") } else { app.i18n.t("source-add") };
                ui.label(egui::RichText::new(title).font(theme::sans_semibold(16.0)).color(p.fg));
                ui.add_space(14.0);
                let ready = fields(app, ui, d);
                ui.add_space(16.0);
                let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 33.0), Sense::hover());
                let apply_label = app.i18n.t("settings-apply");
                let cancel_label = app.i18n.t("settings-cancel");
                let aw = w::text_width(ui.painter(), &apply_label, theme::sans_medium(theme::TEXT)) + 28.0;
                let cw = w::text_width(ui.painter(), &cancel_label, theme::sans(theme::TEXT)) + 30.0;
                let ar = Rect::from_min_size(egui::pos2(row.right() - aw, row.top()), vec2(aw, 33.0));
                let cr = Rect::from_min_size(egui::pos2(ar.left() - 8.0 - cw, row.top()), vec2(cw, 33.0));
                if w::button(ui, cr, "src-cancel", &cancel_label, ButtonKind::Outline, true, &p).clicked() {
                    keep = false;
                }
                if w::button(ui, ar, "src-apply", &apply_label, ButtonKind::Primary, ready, &p).clicked() {
                    apply = true;
                }
            });
        },
    );
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        keep = false;
    }
    if apply {
        let name = if d.name.trim().is_empty() { app.i18n.t("kind-color") } else { d.name.trim().to_owned() };
        let cmd = match d.editing {
            Some(id) => Command::UpdateSource { id, name, kind: d.kind.clone() },
            None => Command::AddSource { name, kind: d.kind.clone() },
        };
        app.send(cmd);
        keep = false;
    }
    keep
}

impl SourceDialog {
    /// Высота окна по типу (оценка для центрирования).
    fn last_height(&self) -> f32 {
        let rows = match self.kind {
            SourceKind::MonitorCapture { .. } => 4.0,
            SourceKind::WindowCapture { .. } => 3.0,
            SourceKind::Image { .. } => 2.0,
            SourceKind::Color { .. } => 3.0,
            SourceKind::Text { .. } => 7.5,
            _ => 2.0,
        };
        20.0 + 21.0 + 14.0 + rows * 32.0 + 16.0 + 33.0 + 20.0
    }
}
