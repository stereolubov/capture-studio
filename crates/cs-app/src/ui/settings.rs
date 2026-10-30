//! Окно настроек (1c, §11.3): модальная панель 720 px, вкладки слева (170 px), поля `150 px | 1fr`.

use super::widgets::{self as w, ButtonKind};
use super::{App, centered};
use crate::i18n::s;
use crate::theme::{self, Palette};
use cs_core::Command;
use cs_core::config::profile::{ColorRange, ColorSpace, EncoderKind, RateControl, RecordFormat, ScaleFilter};
use cs_core::config::settings::{ProcessPriority, ThemePref};
use cs_core::config::{Profile, Settings};
use cs_core::estimate;
use egui::{Align2, Area, Color32, Context, CursorIcon, Id, Order, Painter, Rect, Sense, Ui, pos2, vec2};
use std::collections::HashMap;

const WIDTH: f32 = 720.0;
const TABS_W: f32 = 170.0;
const LABEL_W: f32 = 150.0;
const ROW_GAP: f32 = 14.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Stream,
    Output,
    Audio,
    Video,
    Hotkeys,
}

const TABS: [(SettingsTab, &str); 6] = [
    (SettingsTab::General, "settings-tab-general"),
    (SettingsTab::Stream, "settings-tab-stream"),
    (SettingsTab::Output, "settings-tab-output"),
    (SettingsTab::Audio, "settings-tab-audio"),
    (SettingsTab::Video, "settings-tab-video"),
    (SettingsTab::Hotkeys, "settings-tab-hotkeys"),
];

/// Черновик настроек: изменения применяются только по «Применить».
pub struct SettingsDialog {
    pub tab: SettingsTab,
    settings: Settings,
    profile: Profile,
    /// Буферы текстовых полей (по id поля).
    buffers: HashMap<&'static str, String>,
    stream_key: String,
    show_key: bool,
    advanced: bool,
    hotkey_search: String,
    /// Действие, для которого ждём нажатия сочетания.
    capturing: Option<String>,
    /// Измеренная высота панели (для центрирования и скриншотов).
    pub last_height: f32,
    /// Фоновый опрос устройств и приложений (запускается при первом показе вкладки «Аудио»).
    devices: Option<cs_audio::wasapi::DeviceWatch>,
}

/// Звук приложения (process loopback) — с Windows 10 2004 (сборка 19041), §2.
const APP_AUDIO_MIN_BUILD: u32 = 19041;
/// Каналов микшера не больше (поля смещения синхронизации и место в микшере).
const MAX_CHANNELS: usize = 8;

impl SettingsDialog {
    pub fn new(app: &App, tab: SettingsTab) -> Self {
        Self {
            tab,
            settings: app.settings.clone(),
            profile: app.profile.clone(),
            buffers: HashMap::new(),
            stream_key: String::new(),
            show_key: false,
            advanced: false,
            hotkey_search: String::new(),
            capturing: None,
            last_height: 425.0,
            devices: None,
        }
    }
}

pub fn panel_size(app: &mut App, _ctx: &Context) -> egui::Vec2 {
    let h = app.overlays.settings.as_ref().map_or(425.0, |d| d.last_height);
    vec2(WIDTH, h)
}

/// Модальное окно поверх главного. Возвращает `false`, когда диалог закрыт.
pub fn modal(app: &mut App, ctx: &Context, d: &mut SettingsDialog, screen: Rect) -> bool {
    let p = app.palette();
    // Затемнение фона и блокировка кликов под модальным окном.
    let mut keep = true;
    Area::new(Id::unique("settings-dim")).fade_in(false).order(Order::Foreground).fixed_pos(screen.min).show(
        ctx,
        |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha(if p.dark { 110 } else { 70 }));
            let _ = resp;
        },
    );
    let rect = centered(screen, vec2(WIDTH, d.last_height));
    Area::new(Id::unique("settings-panel")).fade_in(false).order(Order::Foreground).fixed_pos(rect.min).show(
        ctx,
        |ui| {
            let (r, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
            keep = draw_panel(app, ui, d, r);
        },
    );
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && d.capturing.is_none() {
        keep = false;
    }
    keep
}

/// Рисует панель настроек. Возвращает `false` при «Отмена»/«Применить».
pub fn draw_panel(app: &mut App, ui: &mut Ui, d: &mut SettingsDialog, rect: Rect) -> bool {
    let p = app.palette();
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 8.0, p.bg);

    // Вкладки.
    let tabs = Rect::from_min_size(rect.min, vec2(TABS_W, rect.height()));
    painter.rect_filled(tabs, egui::CornerRadius { nw: 8, sw: 8, ne: 0, se: 0 }, p.panel);
    w::vline(&painter, tabs.right() - 1.0, rect.top(), rect.bottom(), p.line);
    let lh13 = w::line_height(&painter, &theme::sans(theme::TEXT));
    let mut y = rect.top() + 12.0;
    for (tab, key) in TABS {
        let row = Rect::from_min_size(pos2(tabs.left(), y), vec2(TABS_W - 1.0, 8.0 + lh13 + 8.0));
        let resp = w::hit(ui, row, ("tab", key), Sense::click());
        let active = d.tab == tab;
        if active {
            painter.rect_filled(row, 0.0, p.selected);
        } else if resp.hovered() {
            painter.rect_filled(row, 0.0, p.hover);
        }
        let color = if active { p.fg } else { p.mute };
        w::text(
            &painter,
            pos2(row.left() + 16.0, y + 8.0),
            Align2::LEFT_TOP,
            &app.i18n.t(key),
            theme::sans(theme::TEXT),
            color,
        );
        if resp.clicked() {
            d.tab = tab;
        }
        y += row.height();
    }

    // Содержимое: отступ 20×24, промежуток 14.
    let content = Rect::from_min_max(
        pos2(tabs.right() + 24.0, rect.top() + 20.0),
        pos2(rect.right() - 24.0, rect.bottom() - 20.0),
    );
    let title_key = TABS.iter().find(|(t, _)| *t == d.tab).map_or("settings-tab-output", |(_, k)| k);
    let title =
        w::text(&painter, content.min, Align2::LEFT_TOP, &app.i18n.t(title_key), theme::sans_semibold(16.0), p.fg);
    let live = app.engine.state().stream.is_active();
    let mut form = Form {
        painter: painter.clone(),
        p,
        x: content.left(),
        width: content.width(),
        y: title.bottom() + ROW_GAP,
        lh12m: w::line_height(&painter, &theme::mono(theme::TEXT_SMALL)),
        locked_hint: app.i18n.t("settings-locked-live"),
    };

    match d.tab {
        SettingsTab::General => general_tab(app, ui, d, &mut form),
        SettingsTab::Stream => stream_tab(app, ui, d, &mut form),
        SettingsTab::Output => output_tab(app, ui, d, &mut form, live),
        SettingsTab::Audio => audio_tab(app, ui, d, &mut form),
        SettingsTab::Video => video_tab(app, ui, d, &mut form, live),
        SettingsTab::Hotkeys => hotkeys_tab(app, ui, d, &mut form),
    }

    // Разделитель и оценка (§9.3).
    let mut y = form.y;
    if d.tab == SettingsTab::Output {
        w::hline(&painter, content.left(), content.right(), y, p.line);
        let o = &d.profile.output;
        let gb = estimate::format_gb_floor(estimate::gigabytes_per_hour(o.bitrate_kbps, o.audio_bitrate_kbps));
        let gpu = format!("{:.0}", app.engine.state().stats.encoder_load_percent);
        let key = if o.rate_control == RateControl::Cqp { "settings-estimate-cq" } else { "settings-estimate" };
        let t = w::text(
            &painter,
            pos2(content.left(), y + 13.0),
            Align2::LEFT_TOP,
            &app.i18n.t_args(key, &[("gb", s(gb)), ("gpu", s(gpu))]),
            theme::sans(theme::TEXT_SMALL),
            p.mute,
        );
        y = t.bottom() + ROW_GAP;
    }

    // Кнопки «Отмена» и «Применить» (выравнивание по правому краю, промежуток 8).
    let bh = 1.0 + 7.0 + lh13 + 7.0 + 1.0;
    let apply_label = app.i18n.t("settings-apply");
    let cancel_label = app.i18n.t("settings-cancel");
    let aw = w::text_width(&painter, &apply_label, theme::sans_medium(theme::TEXT)) + 28.0;
    let cw = w::text_width(&painter, &cancel_label, theme::sans(theme::TEXT)) + 28.0 + 2.0;
    let apply = Rect::from_min_size(pos2(content.right() - aw, y), vec2(aw, bh));
    let cancel = Rect::from_min_size(pos2(apply.left() - 8.0 - cw, y), vec2(cw, bh));
    let mut keep = true;
    if w::button(ui, cancel, "settings-cancel", &cancel_label, ButtonKind::Outline, true, &p).clicked() {
        keep = false;
    }
    if w::button(ui, apply, "settings-apply", &apply_label, ButtonKind::Primary, true, &p).clicked() {
        apply_changes(app, d);
        keep = false;
    }
    if d.tab == SettingsTab::Output {
        let label = format!("{} {}", app.i18n.t("settings-advanced"), if d.advanced { "▾" } else { "▸" });
        let r = w::text(
            &painter,
            pos2(content.left(), apply.center().y),
            Align2::LEFT_CENTER,
            &label,
            theme::sans(theme::TEXT_SMALL),
            p.mute,
        );
        if w::hit(ui, r.expand(4.0), "advanced", Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
            d.advanced = !d.advanced;
        }
    }

    let used = (apply.bottom() + 20.0 - rect.top()).max(12.0 + 6.0 * (16.0 + lh13) + 12.0);
    d.last_height = used.round();
    w::border(&painter, rect, 8.0, p.line);
    keep
}

fn apply_changes(app: &mut App, d: &SettingsDialog) {
    app.settings = d.settings.clone();
    app.profile = d.profile.clone();
    app.send(Command::ApplyProfile(Box::new(d.profile.clone())));
    let offsets: Vec<(usize, i32)> = d.profile.audio.channels.iter().map(|c| c.sync_offset_ms).enumerate().collect();
    for (channel, ms) in offsets {
        if app.engine.state().audio.get(channel).is_some_and(|c| c.sync_offset_ms != ms) {
            app.send(Command::SetSyncOffset { channel, ms });
        }
    }
    let dir = app.recordings_dir();
    app.send(Command::SetRecordingsDir(dir));
    let msg = match app.save_config() {
        Ok(()) => app.i18n.t("settings-saved"),
        Err(e) => app.i18n.t_args("settings-save-failed", &[("err", s(e))]),
    };
    app.toast(msg);
}

/// Помощник раскладки полей.
struct Form {
    painter: Painter,
    p: Palette,
    x: f32,
    width: f32,
    y: f32,
    lh12m: f32,
    locked_hint: String,
}

impl Form {
    fn box_h(&self) -> f32 {
        1.0 + 7.0 + self.lh12m + 7.0 + 1.0
    }

    /// Заголовок группы полей.
    fn caption(&mut self, text: &str) {
        self.y += 6.0;
        w::caption(&self.painter, pos2(self.x, self.y), Align2::LEFT_TOP, text, &self.p);
        self.y += self.lh12m + 6.0;
    }

    /// Подпись и рамка поля; возвращает прямоугольник рамки.
    fn row(&mut self, label: &str) -> Rect {
        let h = self.box_h();
        let rect = Rect::from_min_size(pos2(self.x + LABEL_W + 12.0, self.y), vec2(self.width - LABEL_W - 12.0, h));
        w::text(
            &self.painter,
            pos2(self.x, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            theme::sans(theme::TEXT),
            self.p.mute,
        );
        w::border(&self.painter, rect, 4.0, self.p.line);
        self.y += h + ROW_GAP;
        rect
    }

    fn value(&self, rect: Rect, value: &str, unit: &str, _enabled: bool) {
        let color = self.p.fg;
        w::text(
            &self.painter,
            pos2(rect.left() + 11.0, rect.center().y),
            Align2::LEFT_CENTER,
            value,
            theme::mono(theme::TEXT_SMALL),
            color,
        );
        w::text(
            &self.painter,
            pos2(rect.right() - 11.0, rect.center().y),
            Align2::RIGHT_CENTER,
            unit,
            theme::mono(theme::TEXT_SMALL),
            self.p.mute,
        );
    }

    /// Поле-меню: показывает `value`, по щелчку — список действий. Возвращает выбранное.
    fn menu(&mut self, ui: &mut Ui, id: &str, label: &str, value: &str, items: &[String]) -> Option<usize> {
        let rect = self.row(label);
        let resp = w::hit(ui, rect, ("menu", id), Sense::click());
        if resp.hovered() {
            self.painter.rect_filled(rect.shrink(1.0), 3.0, self.p.hover);
        }
        self.value(rect, value, "▾", true);
        let mut chosen = None;
        let p = self.p;
        egui::Popup::menu(&resp).width(rect.width()).gap(2.0).show(|ui| {
            ui.set_min_width(rect.width() - 12.0);
            for (i, o) in items.iter().enumerate() {
                let text = egui::RichText::new(o).font(theme::mono(theme::TEXT_SMALL)).color(p.fg);
                if ui.selectable_label(false, text).clicked() {
                    chosen = Some(i);
                    ui.close();
                }
            }
        });
        chosen
    }

    /// Выпадающий список. Возвращает `true`, если значение изменилось.
    fn dropdown(
        &mut self,
        ui: &mut Ui,
        id: &str,
        label: &str,
        options: &[String],
        sel: &mut usize,
        enabled: bool,
    ) -> bool {
        let rect = self.row(label);
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let mut resp = w::hit(ui, rect, ("dd", id), sense);
        if !enabled {
            resp = resp.on_hover_text(&self.locked_hint);
        } else if resp.hovered() {
            self.painter.rect_filled(rect.shrink(1.0), 3.0, self.p.hover);
        }
        let current = options.get(*sel).map_or("", String::as_str);
        self.value(rect, current, "▾", enabled);
        let mut changed = false;
        if enabled {
            let p = self.p;
            egui::Popup::menu(&resp).width(rect.width()).gap(2.0).show(|ui| {
                ui.set_min_width(rect.width() - 12.0);
                for (i, o) in options.iter().enumerate() {
                    let text = egui::RichText::new(o).font(theme::mono(theme::TEXT_SMALL)).color(p.fg);
                    if ui.selectable_label(i == *sel, text).clicked() {
                        *sel = i;
                        changed = true;
                        ui.close();
                    }
                }
            });
        }
        changed
    }

    fn toggle(&mut self, ui: &mut Ui, id: &str, label: &str, v: &mut bool, yes: &str, no: &str) {
        let opts = [yes.to_owned(), no.to_owned()];
        let mut sel = usize::from(!*v);
        if self.dropdown(ui, id, label, &opts, &mut sel, true) {
            *v = sel == 0;
        }
    }

    /// Текстовое/числовое поле с буфером редактирования.
    #[expect(clippy::too_many_arguments, reason = "плоский вызов читается проще, чем builder для одного поля")]
    fn text(
        &mut self,
        ui: &mut Ui,
        buf: &mut String,
        id: &str,
        label: &str,
        unit: &str,
        enabled: bool,
        password: bool,
    ) -> egui::Response {
        let rect = self.row(label);
        let unit_w = if unit.is_empty() {
            0.0
        } else {
            w::text_width(&self.painter, unit, theme::mono(theme::TEXT_SMALL)) + 8.0
        };
        let edit_rect = Rect::from_min_max(
            pos2(rect.left() + 10.0, rect.top() + 1.0),
            pos2(rect.right() - 11.0 - unit_w, rect.bottom() - 1.0),
        );
        // Заблокированное поле выглядит как обычное (как в макете), но не редактируется.
        let resp = ui.put(
            edit_rect,
            egui::TextEdit::singleline(buf)
                .id(Id::unique(("field", id)))
                .font(theme::mono(theme::TEXT_SMALL))
                .frame(egui::Frame::NONE)
                .margin(vec2(1.0, 0.0))
                .align(Align2::LEFT_CENTER)
                .min_size(edit_rect.size())
                .password(password)
                .interactive(enabled)
                .text_color(self.p.fg),
        );
        let resp = if enabled { resp } else { resp.on_hover_text(&self.locked_hint) };
        if !unit.is_empty() {
            w::text(
                &self.painter,
                pos2(rect.right() - 11.0, rect.center().y),
                Align2::RIGHT_CENTER,
                unit,
                theme::mono(theme::TEXT_SMALL),
                self.p.mute,
            );
        }
        resp
    }
}

#[expect(clippy::too_many_arguments, reason = "см. Form::text")]
fn number_field(
    form: &mut Form,
    ui: &mut Ui,
    d: &mut SettingsDialog,
    id: &'static str,
    label: &str,
    value: u32,
    unit: &str,
    enabled: bool,
) -> u32 {
    let buf = d.buffers.entry(id).or_insert_with(|| value.to_string());
    if !ui.ctx().memory(|m| m.has_focus(Id::unique(("field", id)))) {
        *buf = value.to_string();
    }
    let mut b = buf.clone();
    form.text(ui, &mut b, id, label, unit, enabled, false);
    b.retain(|c| c.is_ascii_digit());
    let parsed = b.parse::<u32>().ok();
    d.buffers.insert(id, b);
    parsed.unwrap_or(value)
}

fn index_of<T: PartialEq + Copy>(all: &[T], v: T) -> usize {
    all.iter().position(|x| *x == v).unwrap_or(0)
}

fn general_tab(app: &App, ui: &mut Ui, d: &mut SettingsDialog, f: &mut Form) {
    let t = |k: &str| app.i18n.t(k);
    let (yes, no) = (t("yes"), t("no"));
    let langs = ["ru", "en"];
    let mut sel = index_of(&langs, d.settings.language.as_str());
    if f.dropdown(ui, "lang", &t("field-language"), &[t("lang-ru"), t("lang-en")], &mut sel, true) {
        d.settings.language = langs[sel].into();
    }
    let themes = [ThemePref::Dark, ThemePref::Light, ThemePref::System];
    let mut sel = index_of(&themes, d.settings.theme);
    if f.dropdown(
        ui,
        "theme",
        &t("field-theme"),
        &[t("theme-dark"), t("theme-light"), t("theme-system")],
        &mut sel,
        true,
    ) {
        d.settings.theme = themes[sel];
    }
    f.toggle(ui, "start-min", &t("field-start-minimized"), &mut d.settings.start_minimized, &yes, &no);
    f.toggle(ui, "confirm-stop", &t("field-confirm-stop"), &mut d.settings.confirm_stop_stream, &yes, &no);
    f.toggle(ui, "updates", &t("field-check-updates"), &mut d.settings.check_updates, &yes, &no);
    let prios = [ProcessPriority::Normal, ProcessPriority::AboveNormal, ProcessPriority::High];
    let mut sel = index_of(&prios, d.settings.process_priority);
    if f.dropdown(
        ui,
        "prio",
        &t("field-priority"),
        &[t("priority-normal"), t("priority-above-normal"), t("priority-high")],
        &mut sel,
        true,
    ) {
        d.settings.process_priority = prios[sel];
    }
    let mut dir = d.settings.recordings_dir.clone().unwrap_or_else(|| t("default-recordings-dir"));
    let before = dir.clone();
    f.text(ui, &mut dir, "rec-dir", &t("field-recordings-dir"), "", true, false);
    if dir != before {
        d.settings.recordings_dir = Some(dir);
    }
}

const SERVICES: [&str; 7] = ["twitch", "youtube", "vk", "rutube", "kick", "trovo", "custom"];

fn stream_tab(app: &App, ui: &mut Ui, d: &mut SettingsDialog, f: &mut Form) {
    let t = |k: &str| app.i18n.t(k);
    let names: Vec<String> = ["Twitch", "YouTube", "VK Видео Live", "Rutube", "Kick", "Trovo"]
        .iter()
        .map(|s| (*s).to_owned())
        .chain(std::iter::once(t("service-custom")))
        .collect();
    let mut sel = index_of(&SERVICES, d.profile.stream.service.as_str());
    if f.dropdown(ui, "service", &t("field-service"), &names, &mut sel, true) {
        d.profile.stream.service = SERVICES[sel].into();
    }
    let mut server = if d.profile.stream.server == "auto" { t("server-auto") } else { d.profile.stream.server.clone() };
    let before = server.clone();
    f.text(ui, &mut server, "server", &t("field-server"), "", true, false);
    if server != before {
        d.profile.stream.server = server;
    }
    // Ключ хранится только в Windows Credential Manager (M3); в TOML не попадает.
    let show = d.show_key;
    let key_unit = if show { t("key-hide") } else { t("key-show") };
    let row_y = f.y;
    let mut key = d.stream_key.clone();
    f.text(ui, &mut key, "key", &t("field-stream-key"), &key_unit, true, !show);
    d.stream_key = key;
    let unit_w = w::text_width(&f.painter, &key_unit, theme::mono(theme::TEXT_SMALL));
    let unit_rect =
        Rect::from_min_size(pos2(f.x + f.width - 11.0 - unit_w - 4.0, row_y), vec2(unit_w + 15.0, f.box_h()));
    if w::hit(ui, unit_rect, "key-toggle", Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
        d.show_key = !d.show_key;
    }
    let (yes, no) = (t("yes"), t("no"));
    f.toggle(ui, "reconnect", &t("field-reconnect"), &mut d.profile.stream.reconnect, &yes, &no);
    f.toggle(ui, "dynbr", &t("field-dynamic-bitrate"), &mut d.profile.stream.dynamic_bitrate, &yes, &no);
}

fn output_tab(app: &App, ui: &mut Ui, d: &mut SettingsDialog, f: &mut Form, live: bool) {
    let t = |k: &str| app.i18n.t(k);
    let encoders = [
        EncoderKind::Nvenc,
        EncoderKind::Amf,
        EncoderKind::Qsv,
        EncoderKind::MediaFoundation,
        EncoderKind::Software,
        EncoderKind::Auto,
    ];
    let enc_names =
        ["encoder-nvenc", "encoder-amf", "encoder-qsv", "encoder-mf", "encoder-software", "encoder-auto"].map(t);
    // «Авто» в мок-режиме показывает выбранный автоматически NVENC (как в дизайне).
    let shown =
        if d.profile.output.encoder == EncoderKind::Auto { EncoderKind::Nvenc } else { d.profile.output.encoder };
    let mut sel = index_of(&encoders, shown);
    if f.dropdown(ui, "encoder", &t("field-encoder"), &enc_names, &mut sel, !live) {
        d.profile.output.encoder = encoders[sel];
    }
    let rcs = [RateControl::Cbr, RateControl::Vbr, RateControl::Cqp];
    let mut sel = index_of(&rcs, d.profile.output.rate_control);
    if f.dropdown(ui, "rc", &t("field-rate-control"), &["CBR".into(), "VBR".into(), "CQP".into()], &mut sel, !live) {
        d.profile.output.rate_control = rcs[sel];
    }
    if d.profile.output.rate_control == RateControl::Cqp {
        d.profile.output.cq_level =
            number_field(f, ui, d, "cq", "CQ", d.profile.output.cq_level, "", !live).clamp(1, 51);
    } else {
        d.profile.output.bitrate_kbps = number_field(
            f,
            ui,
            d,
            "bitrate",
            &t("field-bitrate"),
            d.profile.output.bitrate_kbps,
            &t("unit-kbps"),
            true,
        )
        .clamp(100, 100_000);
    }
    d.profile.output.keyframe_interval_s = number_field(
        f,
        ui,
        d,
        "keyint",
        &t("field-keyint"),
        d.profile.output.keyframe_interval_s,
        &t("unit-seconds"),
        !live,
    )
    .clamp(1, 10);
    let presets: Vec<String> = (1..=7).map(|i| t(&format!("preset-{i}"))).collect();
    let mut sel = usize::from(d.profile.output.preset.clamp(1, 7) - 1);
    if f.dropdown(ui, "preset", &t("field-preset"), &presets, &mut sel, !live) {
        d.profile.output.preset = sel as u8 + 1;
    }
    let formats = [RecordFormat::MkvRemuxMp4, RecordFormat::Mkv, RecordFormat::Mp4, RecordFormat::FragmentedMp4];
    let mut sel = index_of(&formats, d.profile.output.record_format);
    let fmt_names = ["record-mkv-remux", "record-mkv", "record-mp4", "record-fmp4"].map(t);
    if f.dropdown(ui, "recfmt", &t("field-record-format"), &fmt_names, &mut sel, !app.engine.state().record.is_active())
    {
        d.profile.output.record_format = formats[sel];
    }
    if d.advanced {
        let profiles = ["baseline", "main", "high"];
        let mut sel = index_of(&profiles, d.profile.output.h264_profile.as_str());
        if f.dropdown(
            ui,
            "profile",
            &t("field-profile"),
            &["Baseline".into(), "Main".into(), "High".into()],
            &mut sel,
            !live,
        ) {
            d.profile.output.h264_profile = profiles[sel].into();
        }
        d.profile.output.b_frames =
            number_field(f, ui, d, "bframes", &t("field-bframes"), u32::from(d.profile.output.b_frames), "", !live)
                .min(4) as u8;
        let (yes, no) = (t("yes"), t("no"));
        f.toggle(ui, "lookahead", &t("field-lookahead"), &mut d.profile.output.lookahead, &yes, &no);
        f.toggle(ui, "sep-enc", &t("field-separate-encoder"), &mut d.profile.output.separate_record_encoder, &yes, &no);
    }
}

fn audio_tab(app: &App, ui: &mut Ui, d: &mut SettingsDialog, f: &mut Form) {
    use cs_audio::wasapi;
    use cs_core::config::profile::{AudioChannelKind, AudioChannelSettings};
    let t = |k: &str| app.i18n.t(k);
    let rates = [48_000u32, 44_100];
    let mut sel = index_of(&rates, d.profile.audio.sample_rate);
    if f.dropdown(ui, "rate", &t("field-sample-rate"), &["48 kHz".into(), "44.1 kHz".into()], &mut sel, true) {
        d.profile.audio.sample_rate = rates[sel];
    }
    // Устройства: «По умолчанию» (со следованием за сменой) или конкретное по id. Списки
    // обновляет фоновый поток: подключённое устройство появляется само.
    let snap = d.devices.get_or_insert_with(wasapi::DeviceWatch::start).snapshot();
    let (render, capture) = (snap.render.clone(), snap.capture.clone());
    let a = &mut d.profile.audio;
    for (id, key, list, value) in [
        ("dev-desk", "field-desktop-device", &render, &mut a.desktop_device),
        ("dev-mic", "field-mic-device", &capture, &mut a.mic_device),
        ("dev-mon", "field-monitor-device", &render, &mut a.monitoring_device),
    ] {
        let mut names = vec![t("device-default")];
        names.extend(list.iter().map(|x| x.name.clone()));
        // Сохранённое устройство сейчас не подключено — показываем его отдельной строкой.
        let missing = value.as_ref().filter(|v| !list.iter().any(|x| &x.id == *v)).cloned();
        if missing.is_some() || (value.is_some() && !snap.ready) {
            names.push(t("device-missing"));
        }
        let mut sel = match value {
            None => 0,
            Some(v) => list.iter().position(|x| &x.id == v).map_or(names.len() - 1, |i| i + 1),
        };
        if f.dropdown(ui, id, &t(key), &names, &mut sel, true) {
            *value = if sel == 0 { None } else { list.get(sel - 1).map(|x| x.id.clone()).or(missing) };
        }
    }

    // Звук приложений (§8.1): каналы process loopback по имени exe.
    if crate::platform::system::os_build() >= APP_AUDIO_MIN_BUILD {
        f.caption(&t("audio-apps"));
        let mut remove = None;
        for (i, ch) in d.profile.audio.channels.iter().enumerate().filter(|(_, c)| c.kind == AudioChannelKind::App) {
            let exe = ch.app.clone().unwrap_or_default();
            if f.menu(ui, &format!("app{i}"), &ch.name, &exe, &[t("audio-app-remove")]).is_some() {
                remove = Some(i);
            }
        }
        if let Some(i) = remove {
            d.profile.audio.channels.remove(i);
        }
        if d.profile.audio.channels.len() < MAX_CHANNELS {
            let added: Vec<String> = d.profile.audio.channels.iter().filter_map(|c| c.app.clone()).collect();
            let apps: Vec<_> =
                snap.apps.iter().filter(|x| !added.iter().any(|e| e.eq_ignore_ascii_case(&x.exe))).cloned().collect();
            let items: Vec<String> = if apps.is_empty() {
                vec![t("audio-apps-none")]
            } else {
                apps.iter().map(|x| format!("{} · {}", wasapi::app_title(&x.exe), x.exe)).collect()
            };
            if let Some(i) = f.menu(ui, "app-add", &t("audio-app-add"), &t("audio-app-choose"), &items)
                && let Some(x) = apps.get(i)
            {
                d.profile.audio.channels.push(AudioChannelSettings {
                    name: wasapi::app_title(&x.exe),
                    kind: AudioChannelKind::App,
                    fader_db: 0.0,
                    muted: false,
                    tracks: 0b1,
                    sync_offset_ms: 0,
                    filters: Vec::new(),
                    app: Some(x.exe.clone()),
                });
            }
        }
    }
    // Смещение синхронизации каналов (§8.2): «+» — звук канала позже картинки.
    const IDS: [&str; 8] = ["sync0", "sync1", "sync2", "sync3", "sync4", "sync5", "sync6", "sync7"];
    f.caption(&t("mixer-sync-offset"));
    for (i, id) in IDS.iter().enumerate().take(d.profile.audio.channels.len()) {
        let ch = &d.profile.audio.channels[i];
        let label = ch.name.clone();
        let v = signed_field(f, ui, d, id, &label, ch.sync_offset_ms, &t("unit-ms"));
        d.profile.audio.channels[i].sync_offset_ms = v.clamp(-950, 2000);
    }
}

/// Целое со знаком (поле ввода).
fn signed_field(
    form: &mut Form,
    ui: &mut Ui,
    d: &mut SettingsDialog,
    id: &'static str,
    label: &str,
    value: i32,
    unit: &str,
) -> i32 {
    let buf = d.buffers.entry(id).or_insert_with(|| value.to_string());
    if !ui.ctx().memory(|m| m.has_focus(Id::unique(("field", id)))) {
        *buf = value.to_string();
    }
    let mut b = buf.clone();
    form.text(ui, &mut b, id, label, unit, true, false);
    let negative = b.starts_with('-');
    b.retain(|c| c.is_ascii_digit());
    let parsed = b.parse::<i32>().ok().map(|v| if negative { -v } else { v });
    d.buffers.insert(id, if negative { format!("-{b}") } else { b });
    parsed.unwrap_or(if negative { value.min(0) } else { value })
}

const RESOLUTIONS: [Option<(u32, u32)>; 5] =
    [None, Some((3840, 2160)), Some((2560, 1440)), Some((1920, 1080)), Some((1280, 720))];
const FPS: [(u32, u32); 9] =
    [(24, 1), (25, 1), (30, 1), (48, 1), (50, 1), (60, 1), (120, 1), (30000, 1001), (60000, 1001)];

fn video_tab(app: &App, ui: &mut Ui, d: &mut SettingsDialog, f: &mut Form, live: bool) {
    let t = |k: &str| app.i18n.t(k);
    let res_names: Vec<String> =
        RESOLUTIONS.iter().map(|r| r.map_or_else(|| t("res-monitor"), |(w, h)| format!("{w}×{h}"))).collect();
    let mut sel = index_of(&RESOLUTIONS, d.profile.video.canvas);
    if f.dropdown(ui, "canvas", &t("field-canvas"), &res_names, &mut sel, !live) {
        d.profile.video.canvas = RESOLUTIONS[sel];
    }
    let mut sel = index_of(&RESOLUTIONS, d.profile.video.output);
    if f.dropdown(ui, "outres", &t("field-output-res"), &res_names, &mut sel, !live) {
        d.profile.video.output = RESOLUTIONS[sel];
    }
    let filters = [ScaleFilter::Bilinear, ScaleFilter::Bicubic, ScaleFilter::Lanczos];
    let mut sel = index_of(&filters, d.profile.video.scale_filter);
    if f.dropdown(
        ui,
        "filter",
        &t("field-scale-filter"),
        &["filter-bilinear", "filter-bicubic", "filter-lanczos"].map(t),
        &mut sel,
        true,
    ) {
        d.profile.video.scale_filter = filters[sel];
    }
    let fps_names: Vec<String> = FPS.iter().map(|(n, den)| super::format::fps(*n as f32 / *den as f32)).collect();
    let mut sel = index_of(&FPS, d.profile.video.fps);
    if f.dropdown(ui, "fps", &t("field-fps"), &fps_names, &mut sel, !live) {
        d.profile.video.fps = FPS[sel];
    }
    let spaces = [ColorSpace::Bt709, ColorSpace::Bt601];
    let mut sel = index_of(&spaces, d.profile.video.color_space);
    if f.dropdown(ui, "cs", &t("field-color-space"), &["BT.709".into(), "BT.601".into()], &mut sel, !live) {
        d.profile.video.color_space = spaces[sel];
    }
    let ranges = [ColorRange::Limited, ColorRange::Full];
    let mut sel = index_of(&ranges, d.profile.video.color_range);
    if f.dropdown(ui, "range", &t("field-color-range"), &["range-limited", "range-full"].map(t), &mut sel, !live) {
        d.profile.video.color_range = ranges[sel];
    }
    d.profile.video.sdr_white_nits =
        number_field(f, ui, d, "sdr", &t("field-sdr-white"), d.profile.video.sdr_white_nits, &t("unit-nits"), true)
            .clamp(80, 1000);
}

/// Действия горячих клавиш (§13) и их ключи в профиле.
fn hotkey_actions(app: &App) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> =
        (1..=4).map(|n| (format!("scene.{n}"), app.i18n.t_args("hotkey-action-scene", &[("n", s(n))]))).collect();
    for (id, key) in [
        ("stream.toggle", "hotkey-action-stream"),
        ("record.toggle", "hotkey-action-record"),
        ("record.pause", "hotkey-action-pause"),
        ("mic.mute", "hotkey-action-mute-mic"),
        ("mic.ptt", "hotkey-action-ptt"),
        ("view.mini", "hotkey-action-mini"),
    ] {
        v.push((id.into(), app.i18n.t(key)));
    }
    v
}

fn hotkeys_tab(app: &App, ui: &mut Ui, d: &mut SettingsDialog, f: &mut Form) {
    let p = f.p;
    let t = |k: &str| app.i18n.t(k);
    // Поиск.
    let mut search = d.hotkey_search.clone();
    let r = f.text(ui, &mut search, "hk-search", &t("hotkeys-search"), "", true, false);
    let _ = r;
    d.hotkey_search = search;

    // Захват сочетания.
    if let Some(action) = d.capturing.clone() {
        let combo = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        if let Some((key, m)) = combo {
            d.capturing = None;
            if key != egui::Key::Escape {
                let mut parts = Vec::new();
                if m.ctrl {
                    parts.push("Ctrl");
                }
                if m.shift {
                    parts.push("Shift");
                }
                if m.alt {
                    parts.push("Alt");
                }
                let name = key.name();
                parts.push(name);
                d.profile.hotkeys.insert(action, parts.join("+"));
            }
        }
    }

    let filter = d.hotkey_search.to_lowercase();
    let actions = hotkey_actions(app);
    let row_h = f.box_h() - 4.0;
    for (id, label) in actions.iter().filter(|(_, l)| filter.is_empty() || l.to_lowercase().contains(&filter)) {
        let combo = d.profile.hotkeys.get(id).cloned();
        let conflict = combo.as_ref().and_then(|c| {
            d.profile.hotkeys.iter().find(|(other, v)| *other != id && *v == c).map(|(other, _)| other.clone())
        });
        let rect = Rect::from_min_size(pos2(f.x + LABEL_W + 12.0, f.y), vec2(f.width - LABEL_W - 12.0, row_h));
        w::text(&f.painter, pos2(f.x, rect.center().y), Align2::LEFT_CENTER, label, theme::sans(theme::TEXT), p.mute);
        let resp = w::hit(ui, rect, ("hk", id.as_str()), Sense::click());
        let capturing = d.capturing.as_deref() == Some(id.as_str());
        let border = if conflict.is_some() { p.warn } else { p.line };
        w::border(&f.painter, rect, 4.0, border);
        if resp.hovered() {
            f.painter.rect_filled(rect.shrink(1.0), 3.0, p.hover);
        }
        let text = if capturing { t("hotkeys-press") } else { combo.clone().unwrap_or_else(|| t("hotkeys-none")) };
        w::text(
            &f.painter,
            pos2(rect.left() + 11.0, rect.center().y),
            Align2::LEFT_CENTER,
            &text,
            theme::mono(theme::TEXT_SMALL),
            if capturing { p.warn } else { p.fg },
        );
        if let Some(other) = &conflict {
            let other_label = actions.iter().find(|(i, _)| i == other).map_or(other.as_str(), |(_, l)| l.as_str());
            resp.clone().on_hover_text(app.i18n.t_args("hotkeys-conflict", &[("action", s(other_label))]));
        }
        if resp.clicked() {
            d.capturing = Some(id.clone());
        }
        f.y += row_h + 6.0;
    }
    // Сброс к значениям по умолчанию.
    let label = t("hotkeys-reset");
    let wd = w::text_width(&f.painter, &label, theme::sans(theme::TEXT)) + 28.0;
    let r = Rect::from_min_size(pos2(f.x + LABEL_W + 12.0, f.y + 2.0), vec2(wd, row_h));
    if w::button(ui, r, "hk-reset", &label, ButtonKind::Outline, true, &p).clicked() {
        d.profile.hotkeys = Profile::default().hotkeys;
    }
    f.y += row_h + ROW_GAP + 2.0;
}
