//! Микшер аудио (1d, §11.4): плавающая панель внутри главного окна (ADR-0006).
//!
//! Канал — колонка 84 px: значение дБ, область 240 px с индикаторами L/R и фейдером,
//! название, кнопки `M` и `⚙`.

use super::widgets::{self as w, Icon, Orientation};
use super::{App, format};
use crate::theme::{self, Palette};
use cs_audio::fader;
use cs_core::Command;
use cs_core::state::MonitorMode;
use egui::{Align2, Area, Context, CursorIcon, Id, Order, Rect, Sense, Ui, Vec2, pos2, vec2};

const PAD: f32 = 20.0;
const COL_W: f32 = 84.0;
const GAP: f32 = 20.0;
const METER_H: f32 = 240.0;

pub struct MixerUi {
    pos: Option<egui::Pos2>,
    focus: Option<usize>,
    /// Ввод значения по Ctrl+клику: (канал, текст).
    edit: Option<(usize, String)>,
}

impl MixerUi {
    pub fn new(focus: Option<usize>) -> Self {
        Self { pos: None, focus, edit: None }
    }
}

fn lh(painter: &egui::Painter) -> (f32, f32) {
    (
        w::line_height(painter, &theme::mono(theme::TEXT_CAPTION)),
        w::line_height(painter, &theme::sans(theme::TEXT_SMALL)),
    )
}

pub fn panel_size(app: &App, ctx: &Context) -> Vec2 {
    let n = app.engine.state().audio.len().max(1) as f32;
    let lh11m = w::line_height_ctx(ctx, &theme::mono(theme::TEXT_CAPTION));
    let lh12 = w::line_height_ctx(ctx, &theme::sans(theme::TEXT_SMALL));
    let badge_h = lh12 + 4.0 + 2.0;
    vec2(
        PAD * 2.0 + n * COL_W + (n - 1.0) * GAP + 2.0,
        PAD * 2.0 + lh11m + 10.0 + METER_H + 10.0 + lh12 + 10.0 + badge_h,
    )
}

/// Плавающее окно. Возвращает `false`, если панель закрыта.
pub fn window(app: &mut App, ctx: &Context, m: &mut MixerUi, screen: Rect) -> bool {
    let size = panel_size(app, ctx);
    let default = pos2(screen.right() - 280.0 - size.x - 16.0, screen.top() + 52.0);
    let mut keep = true;
    let area = Area::new(Id::unique("mixer"))
        .fade_in(false)
        .order(Order::Middle)
        .default_pos(m.pos.unwrap_or(default))
        .constrain_to(screen)
        .movable(true)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            keep = draw_panel(app, ui, m, rect);
        });
    m.pos = Some(area.response.rect.min);
    keep
}

/// Рисует панель в `rect`. Возвращает `false`, если нажато «закрыть».
pub fn draw_panel(app: &mut App, ui: &mut Ui, m: &mut MixerUi, rect: Rect) -> bool {
    let p = app.palette();
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 8.0, p.bg);
    w::border(&painter, rect, 8.0, p.line);
    let (lh11m, lh12) = lh(&painter);

    let audio = app.engine.state().audio.clone();
    for (i, ch) in audio.iter().enumerate() {
        let x0 = rect.left() + PAD + i as f32 * (COL_W + GAP);
        let col = Rect::from_min_size(pos2(x0, rect.top() + PAD), vec2(COL_W, rect.height() - 2.0 * PAD));
        let cx = col.center().x;
        if m.focus == Some(i) {
            painter.rect_filled(col.expand(6.0), 4.0, p.selected);
        }

        // Значение дБ (или поле ввода по Ctrl+клику).
        let db_rect = Rect::from_center_size(pos2(cx, col.top() + lh11m / 2.0), vec2(COL_W, lh11m));
        let editing = matches!(&m.edit, Some((c, _)) if *c == i);
        if editing {
            if let Some((_, buf)) = m.edit.as_mut() {
                let resp = ui.put(
                    db_rect.expand2(vec2(-12.0, 3.0)),
                    egui::TextEdit::singleline(buf)
                        .font(theme::mono(theme::TEXT_CAPTION))
                        .align(egui::Align2::CENTER_CENTER)
                        .margin(vec2(2.0, 1.0)),
                );
                resp.request_focus();
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    m.edit = None;
                } else if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let text = buf.trim().trim_end_matches("dB").trim().replace(',', ".");
                    if let Ok(v) = text.parse::<f32>() {
                        app.send(Command::SetFader { channel: i, db: v.clamp(-100.0, fader::MAX_DB) });
                    }
                    m.edit = None;
                }
            }
        } else {
            w::text(
                &painter,
                db_rect.center(),
                Align2::CENTER_CENTER,
                &format::db(ch.fader_db),
                theme::mono(theme::TEXT_CAPTION),
                p.mute,
            );
        }

        // Область индикаторов и фейдера: 6 | 6 | 6 | 8 | 2 → ширина 28, по центру колонки.
        let top = col.top() + lh11m + 10.0;
        let group_x = cx - 14.0;
        let bar_l = Rect::from_min_size(pos2(group_x, top), vec2(6.0, METER_H));
        let bar_r = bar_l.translate(vec2(12.0, 0.0));
        let track = Rect::from_min_size(pos2(group_x + 26.0, top), vec2(2.0, METER_H));
        for (c, bar) in [bar_l, bar_r].into_iter().enumerate() {
            let level = if ch.muted { -60.0 } else { ch.peak[c] };
            w::capsule_meter(&painter, bar, Orientation::Vertical, w::db_to_fraction(level), p.line, |t| {
                p.meter_color(t)
            });
            let hold = w::db_to_fraction(ch.peak_hold[c]);
            if !ch.muted && hold > 0.02 {
                let y = bar.bottom() - hold * METER_H;
                painter.rect_filled(Rect::from_min_size(pos2(bar.left(), y), vec2(6.0, 1.0)), 0.0, p.meter_color(hold));
            }
        }
        painter.rect_filled(track, 0.0, p.line);
        let pos = fader::db_to_pos(ch.fader_db);
        let thumb =
            Rect::from_min_size(pos2(track.center().x - 8.0, track.bottom() - pos * METER_H - 8.0), vec2(16.0, 8.0));

        // Взаимодействие: перетаскивание ползунка/дорожки, колесо ±1 дБ, двойной клик → 0 дБ, Ctrl+клик → ввод.
        let zone = Rect::from_min_max(
            pos2(track.center().x - 12.0, top - 6.0),
            pos2(track.center().x + 12.0, top + METER_H + 6.0),
        );
        let resp = w::hit(ui, zone, ("fader", i), Sense::click_and_drag()).on_hover_cursor(CursorIcon::ResizeVertical);
        let thumb_color = if resp.hovered() || resp.dragged() { p.fg } else { p.fg.gamma_multiply(0.92) };
        painter.rect_filled(thumb, 2.0, thumb_color);
        let ctrl = ui.input(|i| i.modifiers.ctrl);
        if resp.double_clicked() {
            app.send(Command::SetFader { channel: i, db: 0.0 });
        } else if resp.clicked() && ctrl {
            m.edit = Some((i, if ch.fader_db.is_finite() { format!("{:.1}", ch.fader_db) } else { "-inf".into() }));
        } else if (resp.dragged() || resp.drag_started())
            && !ctrl
            && let Some(pp) = resp.interact_pointer_pos()
        {
            let new_pos = ((track.bottom() - pp.y) / METER_H).clamp(0.0, 1.0);
            app.send(Command::SetFader { channel: i, db: (fader::pos_to_db(new_pos) * 10.0).round() / 10.0 });
        }
        let col_hover =
            ui.rect_contains_pointer(Rect::from_min_max(pos2(col.left(), top), pos2(col.right(), top + METER_H)));
        if col_hover {
            let scroll: f32 = ui.input(|i| {
                i.raw
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::MouseWheel { delta, .. } => Some(delta.y),
                        _ => None,
                    })
                    .sum()
            });
            if scroll.abs() > 0.0 {
                let db = fader::step(ch.fader_db, scroll.signum());
                app.send(Command::SetFader { channel: i, db });
            }
        }

        // Название по центру.
        let name_y = top + METER_H + 10.0;
        w::text(&painter, pos2(cx, name_y), Align2::CENTER_TOP, &ch.name, theme::sans(theme::TEXT_SMALL), p.fg);

        // Кнопки M и ⚙: рамка line, отступ 2×6, радиус 3, промежуток 4.
        let by = name_y + lh12 + 10.0;
        let bh = lh12 + 4.0 + 2.0;
        let mw = w::text_width(&painter, "M", theme::sans(theme::TEXT_SMALL)) + 14.0;
        let gw = w::text_width(&painter, "⚙", theme::sans(theme::TEXT_SMALL)).max(8.0) + 14.0;
        let total = mw + 4.0 + gw;
        let mrect = Rect::from_min_size(pos2(cx - total / 2.0, by), vec2(mw, bh));
        let grect = Rect::from_min_size(pos2(mrect.right() + 4.0, by), vec2(gw, bh));
        let mresp = w::badge_button(ui, mrect, ("mute", i), ch.muted.then_some(p.live), &p);
        let mcolor = if ch.muted { egui::Color32::WHITE } else { p.mute };
        w::text(
            &painter,
            mrect.center(),
            Align2::CENTER_CENTER,
            &app.i18n.t("mixer-mute"),
            theme::sans(theme::TEXT_SMALL),
            mcolor,
        );
        if mresp.clicked() {
            app.send(Command::ToggleMute(i));
        }
        let gresp = w::badge_button(ui, grect, ("gear", i), None, &p);
        w::icon(&painter, grect.center(), Icon::Gear, 9.0, p.mute);
        gear_menu(app, &gresp, i, ch.monitor, ch.tracks, ch.sync_offset_ms);
    }

    // Закрыть (✕) в правом верхнем углу — нужно для панели внутри окна.
    let close = Rect::from_center_size(pos2(rect.right() - 12.0, rect.top() + 12.0), vec2(16.0, 16.0));
    let cresp = w::hit(ui, close, "mixer-close", Sense::click());
    w::icon(&painter, close.center(), Icon::Close, 7.0, if cresp.hovered() { p.fg } else { p.line });
    !cresp.clicked()
}

fn gear_menu(app: &mut App, resp: &egui::Response, ch: usize, monitor: MonitorMode, tracks: u8, offset_ms: i32) {
    let p: Palette = app.palette();
    egui::Popup::menu(resp).show(|ui| {
        ui.set_min_width(200.0);
        ui.label(egui::RichText::new(app.i18n.t("mixer-filters")).color(p.fg));
        ui.label(
            egui::RichText::new(app.i18n.t("mixer-filters-soon")).color(p.mute).font(theme::sans(theme::TEXT_CAPTION)),
        );
        ui.separator();
        ui.label(egui::RichText::new(app.i18n.t("mixer-monitor")).color(p.mute).font(theme::sans(theme::TEXT_CAPTION)));
        for (mode, key) in [
            (MonitorMode::Off, "mixer-monitor-off"),
            (MonitorMode::MonitorOnly, "mixer-monitor-only"),
            (MonitorMode::MonitorAndOutput, "mixer-monitor-output"),
        ] {
            if ui.radio(monitor == mode, app.i18n.t(key)).clicked() {
                app.send(Command::SetMonitor { channel: ch, mode });
            }
        }
        ui.separator();
        ui.label(
            egui::RichText::new(app.i18n.t("mixer-sync-offset")).color(p.mute).font(theme::sans(theme::TEXT_CAPTION)),
        );
        // Смещение синхронизации (§8.2): «+» — звук канала позже картинки.
        let mut ms = offset_ms;
        if ui.add(egui::DragValue::new(&mut ms).range(-950..=2000).speed(1.0).suffix(" ms")).changed() {
            app.send(Command::SetSyncOffset { channel: ch, ms });
        }
        ui.separator();
        ui.label(egui::RichText::new(app.i18n.t("mixer-tracks")).color(p.mute).font(theme::sans(theme::TEXT_CAPTION)));
        // Дорожки 1–6 (§8.2): на трансляцию идёт дорожка 1.
        ui.horizontal(|ui| {
            for t in 0..6u8 {
                let mut on = tracks & (1 << t) != 0;
                if ui.checkbox(&mut on, format!("{}", t + 1)).changed() {
                    let new = if on { tracks | (1 << t) } else { tracks & !(1 << t) };
                    app.send(Command::SetTracks { channel: ch, tracks: new });
                }
            }
        });
    });
}
