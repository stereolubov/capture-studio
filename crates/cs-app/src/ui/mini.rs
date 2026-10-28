//! Мини-режим (1f, §11.6): компактное окно 340 px поверх всех окон, фон `#17181bee`,
//! исключено из захвата экрана (`WDA_EXCLUDEFROMCAPTURE` выставляет платформенный слой).

use super::widgets::{self as w, Icon};
use super::{App, Mode, WindowCommand};
use crate::i18n::s;
use crate::theme;
use cs_core::Command;
use cs_core::clock::format_hms;
use egui::{Align2, Color32, Rect, Sense, Ui, Vec2, pos2, vec2};

/// Логический размер окна мини-режима.
/// 340 px содержимого + отступы 2×14 + рамка (content-box в макете, фактически 370 px).
pub const SIZE: Vec2 = vec2(370.0, 104.0);
/// Удержание кнопки «стоп» для остановки трансляции.
const HOLD_SECS: f64 = 0.5;

#[derive(Default)]
pub struct MiniUi {
    /// Время начала удержания кнопки «стоп».
    hold_started: Option<f64>,
}

pub fn draw(app: &mut App, ui: &mut Ui, rect: Rect) {
    let p = app.palette();
    let painter = ui.painter().clone();
    let bg = Color32::from_rgba_unmultiplied(p.panel.r(), p.panel.g(), p.panel.b(), 0xee);
    painter.rect_filled(rect, 8.0, bg);
    w::border(&painter, rect, 8.0, p.line);

    let inner = Rect::from_min_max(rect.min + vec2(15.0, 13.0), rect.max - vec2(15.0, 13.0));
    let lh12m = w::line_height(&painter, &theme::mono(theme::TEXT_SMALL));
    let lh12 = w::line_height(&painter, &theme::sans(theme::TEXT_SMALL));
    let st = app.engine.state().clone();

    // Строка 1: ● LIVE 01:24:07 слева, ⤢ ✕ справа.
    let r1 = lh12m.max(lh12);
    let mid1 = inner.top() + r1 / 2.0;
    if st.stream.is_active() {
        let label = format!("{} {}", app.i18n.t("indicator-live"), format_hms(st.stream_elapsed_secs));
        painter.circle_filled(pos2(inner.left() + 3.5, mid1), 3.5, p.live);
        w::text(
            &painter,
            pos2(inner.left() + 13.0, mid1),
            Align2::LEFT_CENTER,
            &label,
            theme::mono_medium(theme::TEXT_SMALL),
            p.live,
        );
    } else if st.record.is_active() {
        let label = format!("{} {}", app.i18n.t("indicator-rec"), format_hms(st.record_elapsed_secs));
        painter.circle_filled(pos2(inner.left() + 3.5, mid1), 3.5, p.live);
        w::text(
            &painter,
            pos2(inner.left() + 13.0, mid1),
            Align2::LEFT_CENTER,
            &label,
            theme::mono_medium(theme::TEXT_SMALL),
            p.live,
        );
    }
    let close_c = pos2(inner.right() - 5.0, mid1);
    let expand_c = pos2(close_c.x - 15.0, mid1);
    let close_r = Rect::from_center_size(close_c, vec2(16.0, 18.0));
    let expand_r = Rect::from_center_size(expand_c, vec2(16.0, 18.0));
    let er = w::hit(ui, expand_r, "mini-expand", Sense::click()).on_hover_text(app.i18n.t("mini-expand"));
    let cr = w::hit(ui, close_r, "mini-close", Sense::click()).on_hover_text(app.i18n.t("mini-close"));
    w::icon(&painter, expand_c, Icon::Expand, 9.0, if er.hovered() { p.fg } else { p.mute });
    w::icon(&painter, close_c, Icon::Close, 8.0, if cr.hovered() { p.fg } else { p.mute });
    app.chrome_interactive(expand_r);
    app.chrome_interactive(close_r);
    if er.clicked() {
        app.mode = Mode::Main;
        app.push_command(WindowCommand::ExitMini);
    }
    if cr.clicked() {
        // Работа в трее — M3; пока окно сворачивается в панель задач.
        app.mode = Mode::Main;
        app.push_command(WindowCommand::ExitMini);
        app.push_command(WindowCommand::Minimize);
        let msg = app.i18n.t("toast-tray-soon");
        app.toast(msg);
    }

    // Строка 2: 6012 kbps · 60 fps · 0.0% drop.
    let y2 = inner.top() + r1 + 10.0;
    let mut x = inner.left();
    let stats = &st.stats;
    for item in [
        format!("{:.0} {}", stats.bitrate_kbps, app.i18n.t("unit-kbps")),
        format!("{} fps", super::format::fps(stats.fps)),
        app.i18n.t_args("mini-drop", &[("p", s(format!("{:.1}", stats.dropped_percent())))]),
    ] {
        let t = w::text(&painter, pos2(x, y2), Align2::LEFT_TOP, &item, theme::mono(theme::TEXT_SMALL), p.mute);
        x = t.right() + 16.0;
    }

    // Строка 3: кнопки сцен F1–F4 и «стоп».
    let y3 = y2 + lh12m + 10.0;
    let bh = 1.0 + 4.0 + lh12 + 4.0 + 1.0;
    let mut x = inner.left();
    for (i, scene) in st.scenes.iter().enumerate().take(4) {
        let key = scene.hotkey.clone().unwrap_or_else(|| format!("{}", i + 1));
        let tw = w::text_width(&painter, &key, theme::sans(theme::TEXT_SMALL));
        let r = Rect::from_min_size(pos2(x, y3), vec2(tw + 18.0, bh));
        let active = i == st.active_scene;
        let resp =
            w::badge_button(ui, r, ("mini-scene", i), active.then_some(p.selected), &p).on_hover_text(&scene.name);
        w::text(&painter, r.center(), Align2::CENTER_CENTER, &key, theme::sans(theme::TEXT_SMALL), p.fg);
        if resp.clicked() {
            app.send(Command::SelectScene(i));
        }
        app.chrome_interactive(r);
        x = r.right() + 6.0;
    }

    if st.stream.is_active() {
        let sw = w::text_width(&painter, "■", theme::sans(theme::TEXT_SMALL)).max(8.0) + 16.0;
        let r = Rect::from_min_size(pos2(inner.right() - sw, y3), vec2(sw, bh));
        let resp = w::hit(ui, r, "mini-stop", Sense::click_and_drag()).on_hover_text(app.i18n.t("mini-stop-hint"));
        let now = ui.input(|i| i.time);
        // Защита от случайного нажатия: двойной клик или удержание 0.5 с.
        let mut progress = 0.0;
        if resp.is_pointer_button_down_on() {
            let start = *app.mini.hold_started.get_or_insert(now);
            progress = ((now - start) / HOLD_SECS).min(1.0) as f32;
            if progress >= 1.0 {
                app.mini.hold_started = None;
                app.send(Command::StopStream);
            }
            ui.ctx().request_repaint();
        } else {
            app.mini.hold_started = None;
        }
        if resp.double_clicked() {
            app.send(Command::StopStream);
        }
        painter.rect_filled(r, 3.0, p.live);
        if progress > 0.0 {
            let fill = Rect::from_min_size(r.min, vec2(r.width() * progress, r.height()));
            painter.rect_filled(fill, 3.0, Color32::from_white_alpha(60));
        }
        w::icon(&painter, r.center(), Icon::Stop, 8.0, Color32::WHITE);
        app.chrome_interactive(r);
    }
}
