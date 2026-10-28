//! Статистика стрима (1e, §11.5): KPI 3×2, график битрейта за 10 минут,
//! разбивка пропусков (сворачиваемо), «Копировать отчёт».

use super::widgets as w;
use super::{App, WindowCommand, format};
use crate::i18n::s;
use crate::theme::{self, Palette};
use cs_core::clock::format_hms;
use egui::{Align2, Area, Context, CursorIcon, Id, Order, Rect, Sense, Ui, Vec2, pos2, vec2};

/// Ширина содержимого 560 px (§11.5) + отступы 2×20 + рамка: макет использует content-box,
/// фактическая ширина окна в дизайне — 602 px (см. docs/OPEN_QUESTIONS.md, п. Q2).
const WIDTH: f32 = 560.0 + 2.0 * PAD + 2.0;
const PAD: f32 = 20.0;
const GAP: f32 = 18.0;
const CHART_H: f32 = 90.0;
/// В дизайне 100% высоты графика ≈ 1.283 × целевого битрейта.
const CHART_HEADROOM: f32 = 7700.0 / 6000.0;

#[derive(Default)]
pub struct StatsUi {
    pos: Option<egui::Pos2>,
    details: bool,
}

struct Metrics {
    lh11: f32,
    lh18m: f32,
    lh12: f32,
}

fn metrics(ctx: &Context) -> Metrics {
    Metrics {
        lh11: w::line_height_ctx(ctx, &theme::sans(theme::TEXT_CAPTION)),
        lh18m: w::line_height_ctx(ctx, &theme::mono_medium(18.0)),
        lh12: w::line_height_ctx(ctx, &theme::sans(theme::TEXT_SMALL)),
    }
}

fn kpi_cell_h(m: &Metrics) -> f32 {
    12.0 + m.lh11 + 4.0 + m.lh18m + 12.0
}

fn panel_size_with(m: &Metrics, details: bool) -> Vec2 {
    let grid = 1.0 + kpi_cell_h(m) * 2.0 + 1.0 + 1.0;
    let chart = m.lh11 + 6.0 + CHART_H;
    let footer = m.lh12 + 4.0;
    let extra = if details { 3.0 * (m.lh12 + 6.0) } else { 0.0 };
    vec2(WIDTH, PAD + grid + GAP + chart + GAP + footer + extra + PAD)
}

pub fn panel_size(_app: &App, ctx: &Context) -> Vec2 {
    panel_size_with(&metrics(ctx), false)
}

pub fn window(app: &mut App, ctx: &Context, st: &mut StatsUi, screen: Rect) -> bool {
    let size = panel_size_with(&metrics(ctx), st.details);
    let default = pos2(screen.center().x - WIDTH / 2.0, screen.bottom() - 44.0 - size.y - 16.0);
    let mut keep = true;
    let area = Area::new(Id::unique("stats"))
        .fade_in(false)
        .order(Order::Middle)
        .default_pos(st.pos.unwrap_or(default))
        .constrain_to(screen)
        .movable(true)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            keep = draw_panel(app, ui, st, rect);
        });
    st.pos = Some(area.response.rect.min);
    keep
}

fn kpis(app: &App) -> [(String, String); 6] {
    let st = app.engine.state();
    let x = &st.stats;
    [
        (app.i18n.t("stats-uptime"), format_hms(st.stream_elapsed_secs)),
        (app.i18n.t("stats-bitrate"), format!("{:.0}", x.bitrate_kbps)),
        (app.i18n.t("stats-dropped"), format!("{:.1}%", x.dropped_percent())),
        (app.i18n.t("stats-render"), format!("{:.1} ms", x.render_ms)),
        (app.i18n.t("stats-rtt"), format!("{:.0} ms", x.rtt_ms)),
        (app.i18n.t("stats-transferred"), format::bytes(x.transferred_bytes)),
    ]
}

fn report(app: &App) -> String {
    let st = app.engine.state();
    let x = &st.stats;
    let mut out = format!("Capture Studio {} — {}\n", env!("CARGO_PKG_VERSION"), app.i18n.t("stats-title"));
    for (k, v) in kpis(app) {
        out.push_str(&format!("{k}: {v}\n"));
    }
    out.push_str(&format!(
        "CPU {:.0}% · GPU {:.0}% · FPS {:.1}/{:.0} · RAM {:.0} MB\n",
        x.cpu_percent, x.gpu_percent, x.fps, x.target_fps, x.ram_mb
    ));
    out.push_str(&app.i18n.t_args(
        "stats-drops-breakdown",
        &[("r", s(x.dropped_render)), ("e", s(x.dropped_encoder)), ("n", s(x.dropped_network))],
    ));
    out.push('\n');
    out.push_str(&format!(
        "{} · {}×{} · {} kbps {}\n",
        st.output.encoder, st.output.output.0, st.output.output.1, st.output.bitrate_kbps, st.output.rate_control
    ));
    out
}

pub fn draw_panel(app: &mut App, ui: &mut Ui, st: &mut StatsUi, rect: Rect) -> bool {
    let p = app.palette();
    let painter = ui.painter().clone();
    let m = metrics(ui.ctx());
    painter.rect_filled(rect, 8.0, p.bg);
    w::border(&painter, rect, 8.0, p.line);
    let inner = rect.shrink(PAD + 1.0);

    // KPI 3×2: разделители 1 px через фон `line`.
    let cell_h = kpi_cell_h(&m);
    let grid = Rect::from_min_size(inner.min, vec2(inner.width(), 1.0 + cell_h * 2.0 + 1.0 + 1.0));
    painter.rect_filled(grid, 4.0, p.line);
    let cell_w = (grid.width() - 2.0 - 2.0) / 3.0;
    for (i, (k, v)) in kpis(app).iter().enumerate() {
        let (col, row) = ((i % 3) as f32, (i / 3) as f32);
        let min = pos2(grid.left() + 1.0 + col * (cell_w + 1.0), grid.top() + 1.0 + row * (cell_h + 1.0));
        let cell = Rect::from_min_size(min, vec2(cell_w, cell_h));
        let radius = egui::CornerRadius {
            nw: if i == 0 { 3 } else { 0 },
            ne: if i == 2 { 3 } else { 0 },
            sw: if i == 3 { 3 } else { 0 },
            se: if i == 5 { 3 } else { 0 },
        };
        painter.rect_filled(cell, radius, p.bg);
        w::text(&painter, min + vec2(12.0, 12.0), Align2::LEFT_TOP, k, theme::sans(theme::TEXT_CAPTION), p.mute);
        w::text(&painter, min + vec2(12.0, 12.0 + m.lh11 + 4.0), Align2::LEFT_TOP, v, theme::mono_medium(18.0), p.fg);
    }

    // График БИТРЕЙТ · 10 МИН: 60 столбцов, промежуток 2 px, базовая линия `line`.
    let mut y = grid.bottom() + GAP;
    w::caption(&painter, pos2(inner.left(), y), Align2::LEFT_TOP, &app.i18n.t("stats-chart"), &p);
    y += m.lh11 + 6.0;
    let chart = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), CHART_H));
    draw_chart(app, ui, &p, chart);

    // Нижняя строка: «Подробнее» и «Копировать отчёт».
    y = chart.bottom() + GAP;
    let details_label = format!("{} {}", app.i18n.t("stats-details"), if st.details { "▾" } else { "▸" });
    let dr = w::text(
        &painter,
        pos2(inner.left(), y),
        Align2::LEFT_TOP,
        &details_label,
        theme::sans(theme::TEXT_SMALL),
        p.mute,
    );
    if w::hit(ui, dr.expand(3.0), "stats-details", Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
        st.details = !st.details;
    }
    let copy = app.i18n.t("stats-copy");
    let cr =
        w::text(&painter, pos2(inner.right(), y), Align2::RIGHT_TOP, &copy, theme::sans(theme::TEXT_SMALL), p.mute);
    let cresp = w::hit(ui, cr.expand(3.0), "stats-copy", Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    if cresp.hovered() {
        w::hline(&painter, cr.left(), cr.right(), cr.bottom(), p.mute);
    }
    if cresp.clicked() {
        let text = report(app);
        app.push_command(WindowCommand::CopyText(text));
        let msg = app.i18n.t("stats-copied");
        app.toast(msg);
    }

    if st.details {
        let x = app.engine.state().stats.clone();
        let mut ly = y + m.lh12 + 4.0 + 6.0;
        let lines = [
            app.i18n.t_args(
                "stats-drops-breakdown",
                &[("r", s(x.dropped_render)), ("e", s(x.dropped_encoder)), ("n", s(x.dropped_network))],
            ),
            app.i18n.t_args("stats-encoder-load", &[("p", s(format!("{:.0}", x.encoder_load_percent)))]),
            format!("CPU {:.0}% · GPU {:.0}% · RAM {:.0} MB", x.cpu_percent, x.gpu_percent, x.ram_mb),
        ];
        for l in lines {
            w::text(&painter, pos2(inner.left(), ly), Align2::LEFT_TOP, &l, theme::mono(theme::TEXT_SMALL), p.mute);
            ly += m.lh12 + 6.0;
        }
    }

    let close = Rect::from_center_size(pos2(rect.right() - 12.0, rect.top() + 12.0), vec2(16.0, 16.0));
    let resp = w::hit(ui, close, "stats-close", Sense::click());
    w::icon(&painter, close.center(), w::Icon::Close, 7.0, if resp.hovered() { p.fg } else { p.line });
    !resp.clicked()
}

fn draw_chart(app: &App, ui: &Ui, p: &Palette, chart: Rect) {
    let painter = ui.painter();
    let hist = &app.engine.state().stats.bitrate_history;
    let target = app.engine.state().output.bitrate_kbps as f32;
    let max = hist.iter().copied().fold(target * CHART_HEADROOM, f32::max).max(1.0);
    let n = 60usize;
    let bar_w = (chart.width() - 2.0 * (n as f32 - 1.0)) / n as f32;
    let offset = n.saturating_sub(hist.len());
    let hover = ui.ctx().pointer_hover_pos().filter(|pp| chart.contains(*pp));
    for (i, v) in hist.iter().enumerate() {
        let slot = offset + i;
        let x = chart.left() + slot as f32 * (bar_w + 2.0);
        let h = (v / max).clamp(0.0, 1.0) * CHART_H;
        let bar = Rect::from_min_max(pos2(x, chart.bottom() - h), pos2(x + bar_w, chart.bottom()));
        let hovered = hover.is_some_and(|pp| pp.x >= x - 1.0 && pp.x < x + bar_w + 1.0);
        painter.rect_filled(bar, 0.0, if hovered { p.mute } else { p.chart_bar });
        if hovered {
            let ago_s = (hist.len() - 1 - i) * 10;
            let ago = format!("{}:{:02}", ago_s / 60, ago_s % 60);
            let tip = app.i18n.t_args("stats-chart-tip", &[("kbps", s(format!("{v:.0}"))), ("ago", s(ago))]);
            egui::Tooltip::always_open(
                ui.ctx().clone(),
                ui.layer_id(),
                Id::unique("chart-tip"),
                egui::PopupAnchor::Pointer,
            )
            .show(|ui| ui.label(egui::RichText::new(tip).font(theme::mono(theme::TEXT_SMALL))));
        }
    }
    w::hline(painter, chart.left(), chart.right(), chart.bottom(), p.line);
}
