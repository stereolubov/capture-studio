//! Окно фильтров канала (§8.3): цепочка по порядку применения — добавить, включить/выключить,
//! переставить, удалить, параметры. Каждое изменение уходит в движок всей цепочкой.

use super::App;
use super::widgets::{self as w, Icon};
use crate::i18n::s;
use crate::theme;
use cs_core::Command;
use cs_core::config::profile::{AudioFilter, AudioFilterKind};
use egui::{Area, Context, Id, Order, Rect, RichText, Sense, pos2, vec2};

const WIDTH: f32 = 340.0;
const PAD: f32 = 16.0;

pub struct FiltersUi {
    pub channel: usize,
    pos: Option<egui::Pos2>,
}

impl FiltersUi {
    pub fn new(channel: usize) -> Self {
        Self { channel, pos: None }
    }
}

/// Плавающее окно; `false` — закрыто.
pub fn window(app: &mut App, ctx: &Context, st: &mut FiltersUi, screen: Rect) -> bool {
    let Some(ch) = app.engine.state().audio.get(st.channel).cloned() else { return false };
    let p = app.palette();
    let default = pos2(screen.center().x - WIDTH / 2.0, screen.top() + 80.0);
    let mut keep = true;
    let mut filters = ch.filters.clone();
    let area = Area::new(Id::unique("filters"))
        .fade_in(false)
        .order(Order::Middle)
        .default_pos(st.pos.unwrap_or(default))
        .constrain_to(screen)
        .movable(true)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(p.bg)
                .stroke(egui::Stroke::new(1.0, p.line))
                .corner_radius(8.0)
                .inner_margin(PAD)
                .show(ui, |ui| {
                    ui.set_width(WIDTH - 2.0 * PAD);
                    let title = app.i18n.t_args("filters-title", &[("channel", s(&ch.name))]);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(title).color(p.fg).font(theme::sans_medium(theme::TEXT)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (rect, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                            let color = if resp.hovered() { p.fg } else { p.line };
                            w::icon(ui.painter(), rect.center(), Icon::Close, 7.0, color);
                            if resp.clicked() {
                                keep = false;
                            }
                        });
                    });
                    ui.add_space(8.0);
                    if filters.is_empty() {
                        ui.label(
                            RichText::new(app.i18n.t("filters-empty"))
                                .color(p.mute)
                                .font(theme::sans(theme::TEXT_SMALL)),
                        );
                    }
                    let n = filters.len();
                    let mut action = None;
                    for (i, f) in filters.iter_mut().enumerate() {
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut f.enabled, app.i18n.t(f.kind.name_key()));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("✕").on_hover_text(app.i18n.t("filters-remove")).clicked() {
                                    action = Some(Action::Remove(i));
                                }
                                if ui.add_enabled(i + 1 < n, egui::Button::new("▼").small()).clicked() {
                                    action = Some(Action::Swap(i, i + 1));
                                }
                                if ui.add_enabled(i > 0, egui::Button::new("▲").small()).clicked() {
                                    action = Some(Action::Swap(i - 1, i));
                                }
                            });
                        });
                        params(app, ui, &mut f.kind);
                    }
                    ui.separator();
                    ui.menu_button(app.i18n.t("filters-add"), |ui| {
                        for kind in AudioFilterKind::ALL {
                            if ui.button(app.i18n.t(kind.name_key())).clicked() {
                                action = Some(Action::Add(kind));
                                ui.close();
                            }
                        }
                    });
                    match action {
                        Some(Action::Remove(i)) => {
                            filters.remove(i);
                        }
                        Some(Action::Swap(a, b)) => filters.swap(a, b),
                        Some(Action::Add(kind)) => filters.push(AudioFilter { enabled: true, kind }),
                        None => {}
                    }
                });
        });
    st.pos = Some(area.response.rect.min);
    if filters != ch.filters {
        app.send(Command::SetFilters { channel: st.channel, filters });
    }
    keep
}

enum Action {
    Remove(usize),
    Swap(usize, usize),
    Add(AudioFilterKind),
}

/// Параметры фильтра.
fn params(app: &App, ui: &mut egui::Ui, kind: &mut AudioFilterKind) {
    let p = app.palette();
    match kind {
        AudioFilterKind::Gain { db } => {
            let (lo, hi) = cs_audio::filters::GAIN_RANGE_DB;
            slider_full_width(ui);
            ui.add(egui::Slider::new(db, lo..=hi).step_by(0.1).suffix(" dB"));
        }
        AudioFilterKind::NoiseSuppress => {
            ui.label(
                RichText::new(app.i18n.t("filter-noise-hint")).color(p.mute).font(theme::sans(theme::TEXT_CAPTION)),
            );
        }
        AudioFilterKind::Gate { open_db, close_db, attack_ms, hold_ms, release_ms } => {
            use cs_audio::filters::{GATE_ATTACK_MS, GATE_HOLD_MS, GATE_RELEASE_MS, GATE_THRESHOLD_DB};
            fn db(v: &mut f32) -> egui::Slider<'_> {
                egui::Slider::new(v, GATE_THRESHOLD_DB.0..=GATE_THRESHOLD_DB.1).step_by(0.5).suffix(" dB")
            }
            fn ms(v: &mut f32, r: (f32, f32)) -> egui::Slider<'_> {
                egui::Slider::new(v, r.0..=r.1).logarithmic(true).step_by(1.0).fixed_decimals(0).suffix(" ms")
            }
            let (open0, close0) = (*open_db, *close_db);
            labeled(app, ui, "gate-open", |ui| ui.add(db(open_db)));
            labeled(app, ui, "gate-close", |ui| ui.add(db(close_db)));
            labeled(app, ui, "gate-attack", |ui| ui.add(ms(attack_ms, GATE_ATTACK_MS)));
            labeled(app, ui, "gate-hold", |ui| ui.add(ms(hold_ms, GATE_HOLD_MS)));
            labeled(app, ui, "gate-release", |ui| ui.add(ms(release_ms, GATE_RELEASE_MS)));
            // Порог закрытия не выше порога открытия: тянется за тем, который двигали.
            if *close_db > *open_db {
                if *open_db != open0 {
                    *close_db = *open_db;
                } else if *close_db != close0 {
                    *open_db = *close_db;
                }
            }
        }
    }
}

/// Строка параметра: подпись слева, ползунок до поля значения.
fn labeled(app: &App, ui: &mut egui::Ui, key: &str, add: impl FnOnce(&mut egui::Ui) -> egui::Response) {
    const LABEL_W: f32 = 84.0;
    let p = app.palette();
    ui.horizontal(|ui| {
        // Колонка подписей фиксированной ширины: ползунки начинаются ровно друг под другом.
        let (rect, _) = ui.allocate_exact_size(vec2(LABEL_W, ui.spacing().interact_size.y), Sense::hover());
        let font = theme::sans(theme::TEXT_SMALL);
        ui.painter().text(rect.left_center(), egui::Align2::LEFT_CENTER, app.i18n.t(key), font, p.mute);
        slider_full_width(ui);
        add(ui);
    });
}

/// Ползунок на всю ширину окна, поле значения справа.
fn slider_full_width(ui: &mut egui::Ui) {
    ui.spacing_mut().slider_width = ui.available_width() - 80.0;
}
