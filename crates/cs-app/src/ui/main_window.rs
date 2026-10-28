//! Главное окно (1a/1b, §11.2): заголовок 36 px | центральная часть | статус-бар 44 px;
//! колонки 240 px | 1fr | 280 px.

use super::widgets::{self as w, ButtonKind, Icon, Orientation};
use super::{App, SettingsTab, WindowCommand, format};
use crate::i18n::s;
use crate::theme::{self, Palette};
use cs_core::Command;
use cs_core::clock::format_hms;
use cs_core::state::OutputStatus;
use egui::{Align2, Color32, CursorIcon, Rect, Sense, Ui, pos2, vec2};

pub const TITLE_H: f32 = 36.0;
pub const STATUS_H: f32 = 44.0;
pub const LEFT_W: f32 = 240.0;
pub const RIGHT_W: f32 = 280.0;
const PAD_X: f32 = 14.0;
const BTN_W: f32 = 46.0;

/// Состояние главного окна между кадрами.
#[derive(Default)]
pub struct MainUi {
    /// Переименование сцены: (индекс, текст).
    rename: Option<(usize, String)>,
    /// Время первого нажатия «Остановить» при включённом подтверждении.
    stop_armed: Option<f64>,
    /// Перетаскиваемый источник (позиция в списке сверху вниз).
    drag_source: Option<usize>,
}

pub fn draw(app: &mut App, ui: &mut Ui, rect: Rect) {
    let p = app.palette();
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0.0, p.bg);

    let maximized = app.info.maximized;
    // Рамка окна 1 px (как в дизайне); в развёрнутом состоянии рамки нет.
    let c = if maximized { rect } else { rect.shrink(1.0) };

    title_bar(app, ui, &p, Rect::from_min_size(c.min, vec2(c.width(), TITLE_H)));
    let body = Rect::from_min_max(pos2(c.left(), c.top() + TITLE_H), pos2(c.right(), c.bottom() - STATUS_H));
    let left = Rect::from_min_max(body.min, pos2(body.left() + LEFT_W, body.bottom()));
    let right = Rect::from_min_max(pos2(body.right() - RIGHT_W, body.top()), body.max);
    let center = Rect::from_min_max(pos2(left.right(), body.top()), pos2(right.left(), body.bottom()));
    left_panel(app, ui, &p, left);
    center_panel(app, ui, &p, center);
    right_panel(app, ui, &p, right);
    status_bar(app, ui, &p, Rect::from_min_max(pos2(c.left(), c.bottom() - STATUS_H), c.max));

    if !maximized {
        w::border(&painter, rect, 8.0, p.line);
    }
}

fn title_bar(app: &mut App, ui: &mut Ui, p: &Palette, r: Rect) {
    let painter = ui.painter().clone();
    painter.rect_filled(r, 0.0, p.panel);
    w::hline(&painter, r.left(), r.right(), r.bottom() - 1.0, p.line);
    let mid = r.center().y - 0.5;
    app.chrome.caption = r;

    // Логотип: красная точка 10 px + «Capture Studio» (600).
    let mut x = r.left() + PAD_X;
    painter.circle_filled(pos2(x + 5.0, mid), 5.0, p.live);
    x += 10.0 + 8.0;
    let t = w::text(
        &painter,
        pos2(x, mid),
        Align2::LEFT_CENTER,
        &app.i18n.t("app-name"),
        theme::sans_semibold(theme::TEXT),
        p.fg,
    );
    x = t.right() + 20.0;

    // Меню: цвет mute, промежуток 16 px.
    for (kind, key) in super::menus::TOP_MENUS {
        let label = app.i18n.t(key);
        let width = w::text_width(&painter, &label, theme::sans(theme::TEXT));
        let hit_rect = Rect::from_min_max(pos2(x - 6.0, r.top() + 6.0), pos2(x + width + 6.0, r.bottom() - 7.0));
        let resp = w::hit(ui, hit_rect, ("menu", key), Sense::click());
        let open = app.menu.is_open_kind(kind);
        if resp.hovered() || open {
            painter.rect_filled(hit_rect, 4.0, p.selected);
        }
        let color = if resp.hovered() || open { p.fg } else { p.mute };
        w::text(&painter, pos2(x, mid), Align2::LEFT_CENTER, &label, theme::sans(theme::TEXT), color);
        if resp.clicked() {
            if kind == super::menus::MenuKind::Settings {
                app.menu.close();
                app.open_settings(SettingsTab::General);
            } else {
                app.menu.toggle(kind, hit_rect);
            }
        } else if resp.hovered() && app.menu.is_open() && !open && kind != super::menus::MenuKind::Settings {
            app.menu.toggle(kind, hit_rect);
        }
        app.chrome_interactive(hit_rect);
        x += width + 16.0;
    }

    // Кнопки окна: по 46 px.
    let close = Rect::from_min_max(pos2(r.right() - BTN_W, r.top()), pos2(r.right(), r.bottom() - 1.0));
    let maxb = close.translate(vec2(-BTN_W, 0.0));
    let minb = maxb.translate(vec2(-BTN_W, 0.0));

    let resp = w::hit(ui, minb, "win-min", Sense::click()).on_hover_text(app.i18n.t("window-minimize"));
    if resp.hovered() {
        painter.rect_filled(minb, 0.0, p.selected);
    }
    w::icon(&painter, minb.center(), Icon::Minimize, 10.0, if resp.hovered() { p.fg } else { p.mute });
    if resp.clicked() {
        app.push_command(WindowCommand::Minimize);
    }
    app.chrome_interactive(minb);

    // «Развернуть» обрабатывается окном (HTMAXBUTTON → Snap Layouts), здесь только визуал.
    let max_hover = app.info.max_hover;
    if max_hover {
        painter.rect_filled(maxb, 0.0, p.selected);
    }
    let max_icon = if app.info.maximized { Icon::Restore } else { Icon::Maximize };
    w::icon(&painter, maxb.center(), max_icon, 10.0, if max_hover { p.fg } else { p.mute });
    app.chrome.max_button = Some(maxb);
    // Запасной путь (скриншот-режим, отсутствие HTMAXBUTTON): клик внутри клиента.
    if w::hit(ui, maxb, "win-max", Sense::click()).clicked() {
        app.push_command(WindowCommand::ToggleMaximize);
    }

    let resp = w::hit(ui, close, "win-close", Sense::click()).on_hover_text(app.i18n.t("window-close"));
    if resp.hovered() {
        painter.rect_filled(close, 0.0, Color32::from_rgb(0xc4, 0x2b, 0x1c));
    }
    w::icon(&painter, close.center(), Icon::Close, 10.0, if resp.hovered() { Color32::WHITE } else { p.mute });
    if resp.clicked() {
        app.push_command(WindowCommand::Close);
    }
    app.chrome_interactive(close);

    // Индикаторы LIVE / REC справа, промежуток 20 px до кнопок окна.
    let st = app.engine.state();
    let mut right_x = minb.left() - 20.0;
    let mut indicators = Vec::new();
    if st.stream.is_active() {
        indicators.push(format!("{} {}", app.i18n.t("indicator-live"), format_hms(st.stream_elapsed_secs)));
    }
    if st.record.is_active() {
        indicators.push(format!("{} {}", app.i18n.t("indicator-rec"), format_hms(st.record_elapsed_secs)));
    }
    for label in indicators {
        let t = w::text(
            &painter,
            pos2(right_x, mid),
            Align2::RIGHT_CENTER,
            &label,
            theme::mono_medium(theme::TEXT_CAPTION),
            p.live,
        );
        painter.circle_filled(pos2(t.left() - 6.0 - 3.5, mid), 3.5, p.live);
        right_x = t.left() - 6.0 - 7.0 - 16.0;
    }
}

fn left_panel(app: &mut App, ui: &mut Ui, p: &Palette, r: Rect) {
    let painter = ui.painter().clone();
    painter.rect_filled(r, 0.0, p.panel);
    w::vline(&painter, r.right() - 1.0, r.top(), r.bottom(), p.line);
    let inner_right = r.right() - 1.0;
    let lh13 = w::line_height(&painter, &theme::sans(theme::TEXT));
    let lh11 = w::line_height(&painter, &theme::sans(theme::TEXT_CAPTION));
    let lh11m = w::line_height(&painter, &theme::mono(theme::TEXT_CAPTION));

    // Секция СЦЕНЫ.
    let mut y = r.top() + 12.0;
    w::caption(&painter, pos2(r.left() + PAD_X, y), Align2::LEFT_TOP, &app.i18n.t("section-scenes"), p);
    add_button(app, ui, p, pos2(inner_right - PAD_X, y + lh11 / 2.0), "add-scene", |app| {
        let n = app.engine.state().scenes.len() + 1;
        let name = app.i18n.t_args("scene-new-name", &[("n", s(n))]);
        app.send(Command::AddScene(name));
    });
    y += lh11 + 6.0;

    let scenes = app.engine.state().scenes.clone();
    let active = app.engine.state().active_scene;
    let row_h = 7.0 + lh13 + 7.0;
    for (i, scene) in scenes.iter().enumerate() {
        let row = Rect::from_min_size(pos2(r.left(), y), vec2(inner_right - r.left(), row_h));
        let resp = w::hit(ui, row, ("scene", i), Sense::click());
        if i == active {
            painter.rect_filled(row, 0.0, p.selected);
            painter.rect_filled(Rect::from_min_size(row.min, vec2(2.0, row_h)), 0.0, p.live);
        } else if resp.hovered() {
            painter.rect_filled(row, 0.0, p.hover);
        }
        let text_y = y + 7.0;
        let renaming = matches!(&app.main.rename, Some((idx, _)) if *idx == i);
        if renaming {
            if let Some((_, buf)) = app.main.rename.as_mut() {
                let edit_rect =
                    Rect::from_min_max(pos2(r.left() + 12.0, y + 3.0), pos2(inner_right - 40.0, y + row_h - 3.0));
                let out = ui.put(
                    edit_rect,
                    egui::TextEdit::singleline(buf).font(theme::sans(theme::TEXT)).margin(vec2(4.0, 3.0)),
                );
                out.request_focus();
                let done = out.lost_focus();
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
                if escape {
                    app.main.rename = None;
                } else if done || enter {
                    let name = buf.trim().to_owned();
                    app.main.rename = None;
                    if !name.is_empty() {
                        app.send(Command::RenameScene(i, name));
                    }
                }
            }
        } else {
            w::text(
                &painter,
                pos2(r.left() + 2.0 + PAD_X, text_y),
                Align2::LEFT_TOP,
                &scene.name,
                theme::sans(theme::TEXT),
                p.fg,
            );
        }
        if let Some(key) = &scene.hotkey {
            // Как в макете: строка mono 11 px выровнена по верху строки, а не по центру.
            let _ = lh11m;
            w::text(
                &painter,
                pos2(inner_right - PAD_X, text_y),
                Align2::RIGHT_TOP,
                key,
                theme::mono(theme::TEXT_CAPTION),
                p.mute,
            );
        }
        if resp.double_clicked() {
            app.main.rename = Some((i, scene.name.clone()));
        } else if resp.clicked() {
            app.send(Command::SelectScene(i));
        }
        resp.context_menu(|ui| {
            if ui.button(app.i18n.t("scene-rename")).clicked() {
                app.main.rename = Some((i, scene.name.clone()));
                ui.close();
            }
            if ui.button(app.i18n.t("menu-scenes-duplicate")).clicked() {
                app.send(Command::DuplicateScene(i));
                ui.close();
            }
            if ui.button(app.i18n.t("menu-scenes-remove")).clicked() {
                app.send(Command::RemoveScene(i));
                ui.close();
            }
        });
        y += row_h;
    }

    // Секция ИСТОЧНИКИ: верхняя граница, отступ сверху 12 px, внутренний 18 px.
    y += 12.0;
    w::hline(&painter, r.left(), inner_right, y, p.line);
    y += 1.0 + 18.0;
    w::caption(&painter, pos2(r.left() + PAD_X, y), Align2::LEFT_TOP, &app.i18n.t("section-sources"), p);
    let add = add_button(app, ui, p, pos2(inner_right - PAD_X, y + lh11 / 2.0), "add-source", |_| {});
    egui::Popup::menu(&add).show(|ui| {
        ui.set_min_width(200.0);
        for (kind, key) in super::source_dialog::ADDABLE {
            if ui.button(app.i18n.t(key)).clicked() {
                app.open_new_source(kind);
                ui.close();
            }
        }
        ui.separator();
        for key in super::source_dialog::LATER {
            ui.add_enabled(false, egui::Button::new(format!("{} · {}", app.i18n.t(key), app.i18n.t("menu-soon"))));
        }
    });
    y += lh11 + 6.0;

    let sources = app.engine.state().sources.clone();
    let src_h = 7.0 + lh13 + 2.0 + lh11 + 7.0;
    let list_top = y;
    let n = sources.len();
    let mut drop_target: Option<usize> = None;
    let dragging = app.main.drag_source;
    for (i, src) in sources.iter().enumerate() {
        let row = Rect::from_min_size(pos2(r.left(), y), vec2(inner_right - r.left(), src_h));
        if row.top() > r.bottom() {
            break;
        }
        let resp = w::hit(ui, row, ("source", src.id.0), Sense::click_and_drag());
        let selected = app.preview_ui.selected == Some(src.id);
        if selected {
            painter.rect_filled(row, 0.0, p.selected);
        } else if resp.hovered() {
            painter.rect_filled(row, 0.0, p.hover);
        }
        let ty = y + 7.0;
        w::text(&painter, pos2(r.left() + PAD_X, ty), Align2::LEFT_TOP, &src.name, theme::sans(theme::TEXT), p.fg);
        let meta = format::source_meta(&app.i18n, src);
        let meta_color = if src.error.is_some() || src.waiting { p.warn } else { p.mute };
        let meta_rect = Rect::from_min_max(
            pos2(r.left() + PAD_X, ty + lh13 + 2.0),
            pos2(inner_right - PAD_X, ty + lh13 + 2.0 + lh11),
        );
        w::text(
            &painter.with_clip_rect(meta_rect),
            meta_rect.min,
            Align2::LEFT_TOP,
            &meta,
            theme::sans(theme::TEXT_CAPTION),
            meta_color,
        );

        // Иконка видимости ◉/○ и замок: кликабельны.
        let vis_c = pos2(inner_right - PAD_X - 3.5, ty + lh11m / 2.0);
        let vis_rect = egui::Rect::from_center_size(vis_c, vec2(18.0, 18.0));
        let tip = if src.visible { "source-visible" } else { "source-hidden" };
        let vresp = w::hit(ui, vis_rect, ("vis", src.id.0), Sense::click()).on_hover_text(app.i18n.t(tip));
        let vcolor = if vresp.hovered() { p.fg } else { p.mute };
        w::visibility_icon(&painter, vis_c, src.visible, vcolor);
        if vresp.clicked() {
            app.send(Command::ToggleSourceVisible(src.item));
        }
        if src.locked {
            w::lock_icon(&painter, pos2(vis_c.x - 16.0, vis_c.y), p.mute);
        }

        if resp.double_clicked() {
            app.open_source_dialog(Some(src.clone()));
        } else if resp.clicked() {
            app.preview_ui.selected = Some(src.id);
        }
        if resp.drag_started() {
            app.main.drag_source = Some(i);
        }
        resp.context_menu(|ui| super::preview::item_menu(app, ui, src));
        y += src_h;
    }

    // Перетаскивание порядка (drag & drop, §7.2): линия вставки и перенос по отпусканию.
    if let Some(from) = dragging {
        if let Some(pos) = ui.ctx().pointer_hover_pos() {
            let slot = (((pos.y - list_top) / src_h).round().max(0.0) as usize).min(n);
            drop_target = Some(slot);
            let ly = list_top + slot as f32 * src_h;
            painter.rect_filled(
                Rect::from_min_size(pos2(r.left() + 6.0, ly - 1.0), vec2(inner_right - r.left() - 12.0, 2.0)),
                1.0,
                p.live,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            app.main.drag_source = None;
            if let Some(slot) = drop_target {
                // Позиции списка сверху вниз → индексы элементов сцены снизу вверх.
                let to_display = if slot > from { slot - 1 } else { slot };
                if to_display != from && from < n {
                    let item_from = sources[from].item;
                    let item_to = n - 1 - to_display;
                    app.send(Command::MoveItem { from: item_from, to: item_to });
                }
            }
        }
    }
}

/// Кнопка `+` в заголовке секции.
fn add_button(
    app: &mut App,
    ui: &mut Ui,
    p: &Palette,
    right_center: egui::Pos2,
    id: &str,
    on_click: impl FnOnce(&mut App),
) -> egui::Response {
    let c = pos2(right_center.x - 3.5, right_center.y);
    let rect = Rect::from_center_size(c, vec2(18.0, 18.0));
    let resp = w::hit(ui, rect, id, Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, p.selected);
    }
    w::icon(ui.painter(), c, Icon::Plus, 7.0, if resp.hovered() { p.fg } else { p.mute });
    if resp.clicked() {
        on_click(app);
    }
    resp
}

fn center_panel(app: &mut App, ui: &mut Ui, p: &Palette, r: Rect) {
    let painter = ui.painter().clone();
    let inner = r.shrink(16.0);
    let lh12m = w::line_height(&painter, &theme::mono(theme::TEXT_SMALL));
    let preview = Rect::from_min_max(inner.min, pos2(inner.right(), inner.bottom() - lh12m - 12.0));
    let st = app.engine.state().clone();
    let out = &st.output;

    // Превью сцены (текстура холста); без движка (мок, скриншоты) — заглушка со штриховкой.
    if !super::preview::draw(app, ui, p, preview) {
        w::hatch(&painter, preview.shrink(1.0), p.panel, p.bg);
        let label = app.i18n.t_args(
            "preview-placeholder",
            &[("w", s(out.canvas.0)), ("h", s(out.canvas.1)), ("fps", s(format::fps(out.fps)))],
        );
        w::text(&painter, preview.center(), Align2::CENTER_CENTER, &label, theme::mono(theme::TEXT_SMALL), p.mute);
    }
    w::border(&painter, preview, 4.0, p.line);
    w::badge(
        &painter,
        preview.min + vec2(11.0, 10.0),
        &app.i18n.t("preview-program"),
        theme::mono(theme::TEXT_CAPTION),
        vec2(7.0, 3.0),
        p.panel,
        p.fg,
    );

    // Строка параметров: клик — настройки вывода.
    let y = preview.bottom() + 12.0;
    let items = [
        app.i18n.t_args("param-canvas", &[("w", s(out.canvas.0)), ("h", s(out.canvas.1))]),
        app.i18n.t_args("param-output", &[("w", s(out.output.0)), ("h", s(out.output.1))]),
        out.encoder.clone(),
        app.i18n.t_args("param-bitrate", &[("kbps", s(out.bitrate_kbps)), ("rc", s(&out.rate_control))]),
    ];
    let mut x = inner.left();
    let row = Rect::from_min_max(pos2(inner.left(), y), pos2(inner.right(), y + lh12m));
    let resp = w::hit(ui, row, "params", Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let color = if resp.hovered() { p.fg } else { p.mute };
    for item in items {
        let t = w::text(&painter, pos2(x, y), Align2::LEFT_TOP, &item, theme::mono(theme::TEXT_SMALL), color);
        x = t.right() + 24.0;
    }
    if resp.clicked() {
        app.open_settings(SettingsTab::Output);
    }
}

fn right_panel(app: &mut App, ui: &mut Ui, p: &Palette, r: Rect) {
    let painter = ui.painter().clone();
    painter.rect_filled(r, 0.0, p.panel);
    w::vline(&painter, r.left(), r.top(), r.bottom(), p.line);
    let left = r.left() + 1.0;
    let lh13 = w::line_height(&painter, &theme::sans(theme::TEXT));
    let lh11 = w::line_height(&painter, &theme::sans(theme::TEXT_CAPTION));

    let mut y = r.top() + 12.0;
    w::caption(&painter, pos2(left + PAD_X, y), Align2::LEFT_TOP, &app.i18n.t("section-audio"), p);
    y += lh11 + 6.0;

    let audio = app.engine.state().audio.clone();
    let ch_h = 10.0 + lh13 + 6.0 + 6.0 + 10.0;
    for (i, ch) in audio.iter().enumerate() {
        let row = Rect::from_min_size(pos2(left, y), vec2(r.right() - left, ch_h));
        let resp = w::hit(ui, row, ("audio", i), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        if resp.hovered() {
            painter.rect_filled(row, 0.0, p.hover);
        }
        let ty = y + 10.0;
        let name_color = if ch.device_missing { p.warn } else { p.fg };
        let resp = if ch.device_missing { resp.on_hover_text(app.i18n.t("audio-no-device")) } else { resp };
        w::text(&painter, pos2(left + PAD_X, ty), Align2::LEFT_TOP, &ch.name, theme::sans(theme::TEXT), name_color);
        w::text(
            &painter,
            pos2(r.right() - PAD_X, ty),
            Align2::RIGHT_TOP,
            &format::db(ch.fader_db),
            theme::mono(theme::TEXT_CAPTION),
            p.mute,
        );
        let meter = Rect::from_min_size(pos2(left + PAD_X, ty + lh13 + 6.0), vec2(r.right() - left - 2.0 * PAD_X, 6.0));
        let level = if ch.muted { -60.0 } else { ch.peak[0].max(ch.peak[1]) };
        w::capsule_meter(&painter, meter, Orientation::Horizontal, w::db_to_fraction(level), p.line, |t| {
            p.meter_color(t)
        });
        if resp.clicked() {
            app.open_mixer(Some(i));
        }
        y += ch_h;
    }
    if audio.is_empty() && app.engine.state().live_engine {
        let hint = app.i18n.t("audio-later");
        w::text(&painter, pos2(left + PAD_X, y + 4.0), Align2::LEFT_TOP, &hint, theme::sans(theme::TEXT_SMALL), p.mute);
    }

    // Кнопки управления внизу.
    let hs = 9.0 + lh13 + 9.0;
    let hr = 1.0 + 8.0 + lh13 + 8.0 + 1.0;
    let area_h = 1.0 + 14.0 + hs + 8.0 + hr + 14.0;
    let top = r.bottom() - area_h;
    w::hline(&painter, left, r.right(), top, p.line);
    let bx0 = left + PAD_X;
    let bx1 = r.right() - PAD_X;
    let stream_rect = Rect::from_min_max(pos2(bx0, top + 15.0), pos2(bx1, top + 15.0 + hs));
    let rec_rect =
        Rect::from_min_max(pos2(bx0, stream_rect.bottom() + 8.0), pos2(bx1, stream_rect.bottom() + 8.0 + hr));

    let st = app.engine.state().clone();
    let now = ui.input(|i| i.time);
    if app.main.stop_armed.is_some_and(|t| now - t > 3.0) {
        app.main.stop_armed = None;
    }
    let (label, kind, enabled) = match &st.stream {
        OutputStatus::Idle | OutputStatus::Error(_) => (app.i18n.t("btn-start-stream"), ButtonKind::Outline, true),
        OutputStatus::Starting => (app.i18n.t("btn-connecting"), ButtonKind::Outline, false),
        OutputStatus::Live | OutputStatus::Reconnecting { .. } => {
            let key = if app.main.stop_armed.is_some() { "btn-confirm-stop" } else { "btn-stop-stream" };
            (app.i18n.t(key), ButtonKind::Live, true)
        }
        OutputStatus::Stopping => (app.i18n.t("btn-stopping"), ButtonKind::Live, false),
    };
    if w::button(ui, stream_rect, "btn-stream", &label, kind, enabled, p).clicked() {
        if st.stream.is_active() {
            if app.settings.confirm_stop_stream && app.main.stop_armed.is_none() {
                app.main.stop_armed = Some(now);
            } else {
                app.main.stop_armed = None;
                app.send(Command::StopStream);
            }
        } else if st.live_engine {
            let msg = app.i18n.t("stream-later");
            app.toast(msg);
        } else {
            app.send(Command::StartStream);
        }
    }
    let rec_label = if st.record.is_active() || st.record == OutputStatus::Stopping {
        app.i18n.t("btn-stop-record")
    } else {
        app.i18n.t("btn-start-record")
    };
    if w::button(ui, rec_rect, "btn-record", &rec_label, ButtonKind::Outline, !st.record.is_transitional(), p).clicked()
    {
        if st.record.is_active() {
            app.send(Command::StopRecording);
        } else {
            app.send(Command::StartRecording);
        }
    }
}

fn status_bar(app: &mut App, ui: &mut Ui, p: &Palette, r: Rect) {
    let painter = ui.painter().clone();
    w::hline(&painter, r.left(), r.right(), r.top(), p.line);
    let mid = r.center().y + 0.5;
    let resp = w::hit(ui, r, "statusbar", Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let st = app.engine.state().clone();
    let stats = &st.stats;
    let font = theme::mono(theme::TEXT_SMALL);

    let drop_pct = stats.dropped_percent();
    let level_color = |bad: bool, worse: bool| {
        if worse {
            p.live
        } else if bad {
            p.warn
        } else {
            p.fg
        }
    };
    let fps_color = level_color(stats.fps < stats.target_fps * 0.95, false);
    let drop_color = level_color(drop_pct > 1.0, drop_pct > 5.0);
    let rec = if st.record.is_active() { format_hms(st.record_elapsed_secs) } else { "—".into() };
    let items: [(String, String, Color32); 6] = [
        (app.i18n.t("status-cpu"), format!("{:.0}%", stats.cpu_percent), p.fg),
        (
            app.i18n.t("status-gpu"),
            if stats.gpu_percent.is_finite() { format!("{:.0}%", stats.gpu_percent) } else { "—".into() },
            p.fg,
        ),
        (app.i18n.t("status-fps"), format!("{:.1}", stats.fps), fps_color),
        (app.i18n.t("status-bitrate"), format!("{:.0} {}", stats.bitrate_kbps, app.i18n.t("unit-kbps")), p.fg),
        (app.i18n.t("status-drops"), format!("{} ({:.1}%)", stats.dropped_frames, drop_pct), drop_color),
        (app.i18n.t("status-rec"), rec, p.fg),
    ];
    let mut x = r.left() + 16.0;
    for (k, v, vc) in items {
        let t = w::text(&painter, pos2(x, mid), Align2::LEFT_CENTER, &format!("{k} "), font.clone(), p.mute);
        let t2 = w::text(&painter, pos2(t.right(), mid), Align2::LEFT_CENTER, &v, font.clone(), vc);
        x = t2.right() + 28.0;
    }

    let conn = match &st.stream {
        OutputStatus::Live => Some((app.i18n.t("status-connection-ok"), p.ok)),
        OutputStatus::Reconnecting { attempt, max } => {
            Some((app.i18n.t_args("status-reconnecting", &[("n", s(attempt)), ("max", s(max))]), p.warn))
        }
        OutputStatus::Error(_) => Some((app.i18n.t("status-no-connection"), p.live)),
        _ => None,
    };
    if let Some((text, color)) = conn {
        w::text(&painter, pos2(r.right() - 16.0, mid), Align2::RIGHT_CENTER, &text, font, color);
    }
    if resp.clicked() {
        app.open_stats();
    }
}
