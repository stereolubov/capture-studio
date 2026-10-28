//! Дизайн-токены (§11.1): цвета тем, шрифты, размеры.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

/// Палитра темы.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub line: Color32,
    pub fg: Color32,
    pub mute: Color32,
    pub selected: Color32,
    pub live: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub primary_bg: Color32,
    pub primary_fg: Color32,
    /// Столбцы графика битрейта (1e).
    pub chart_bar: Color32,
    /// Фон при наведении на кнопки/строки.
    pub hover: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// `rgba(127,127,140,0.14)` → premultiplied.
const SELECTED: Color32 = Color32::from_rgba_premultiplied(18, 18, 20, 36);
const HOVER: Color32 = Color32::from_rgba_premultiplied(10, 10, 11, 20);

impl Palette {
    pub const DARK: Self = Self {
        dark: true,
        bg: rgb(0x111214),
        panel: rgb(0x17181b),
        line: rgb(0x26282c),
        fg: rgb(0xe6e7ea),
        mute: rgb(0x8b8f97),
        selected: SELECTED,
        live: rgb(0xed4a49),
        ok: rgb(0x53be70),
        warn: rgb(0xe3b831),
        primary_bg: rgb(0xe6e7ea),
        primary_fg: rgb(0x111214),
        chart_bar: rgb(0x3b3f47),
        hover: HOVER,
    };

    pub const LIGHT: Self = Self {
        dark: false,
        bg: rgb(0xf4f4f5),
        panel: rgb(0xffffff),
        line: rgb(0xe2e3e6),
        fg: rgb(0x18191b),
        mute: rgb(0x6b6f76),
        selected: SELECTED,
        live: rgb(0xed4a49),
        ok: rgb(0x53be70),
        warn: rgb(0xe3b831),
        primary_bg: rgb(0x18191b),
        primary_fg: rgb(0xf4f4f5),
        chart_bar: rgb(0xc9ccd2),
        hover: HOVER,
    };

    /// Цвет индикатора уровня в точке шкалы `t` (0…1, −60…0 dBFS).
    /// Переходы зон — как градиент дизайна: зелёный до 70%, к жёлтому до 85%, к красному до 100%.
    pub fn meter_color(&self, t: f32) -> Color32 {
        let lerp = |a: Color32, b: Color32, k: f32| {
            let k = k.clamp(0.0, 1.0);
            let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * k).round() as u8;
            Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
        };
        if t <= 0.70 {
            self.ok
        } else if t <= 0.85 {
            lerp(self.ok, self.warn, (t - 0.70) / 0.15)
        } else {
            lerp(self.warn, self.live, (t - 0.85) / 0.15)
        }
    }
}

/// Имена семейств шрифтов.
pub mod family {
    pub const SANS: &str = "sans";
    pub const SANS_MEDIUM: &str = "sans-500";
    pub const SANS_SEMIBOLD: &str = "sans-600";
    pub const MONO: &str = "mono";
    pub const MONO_MEDIUM: &str = "mono-500";
}

pub fn sans(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(family::SANS.into()))
}
pub fn sans_medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(family::SANS_MEDIUM.into()))
}
pub fn sans_semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(family::SANS_SEMIBOLD.into()))
}
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(family::MONO.into()))
}
pub fn mono_medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(family::MONO_MEDIUM.into()))
}

/// Типографика (§11.1).
pub const TEXT: f32 = 13.0;
pub const TEXT_SMALL: f32 = 12.0;
pub const TEXT_CAPTION: f32 = 11.0;
/// `letter-spacing: 0.08em` для подписей секций 11 px.
pub const CAPTION_SPACING: f32 = 0.08 * TEXT_CAPTION;

/// Встроенные шрифты (OFL): IBM Plex Sans 400/500/600, JetBrains Mono 400/500.
pub fn font_definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::empty();
    let fonts: [(&str, &'static [u8]); 5] = [
        (family::SANS, include_bytes!("../../../assets/fonts/IBMPlexSans-Regular.ttf")),
        (family::SANS_MEDIUM, include_bytes!("../../../assets/fonts/IBMPlexSans-Medium.ttf")),
        (family::SANS_SEMIBOLD, include_bytes!("../../../assets/fonts/IBMPlexSans-SemiBold.ttf")),
        (family::MONO, include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf")),
        (family::MONO_MEDIUM, include_bytes!("../../../assets/fonts/JetBrainsMono-Medium.ttf")),
    ];
    for (name, data) in fonts {
        defs.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(data)));
    }
    // Каждое семейство: свой шрифт, затем запасные (символы вне Plex — из JetBrains Mono и наоборот).
    for (name, _) in fonts {
        let fallback = if name.starts_with("mono") { family::SANS } else { family::MONO };
        defs.families.insert(FontFamily::Name(name.into()), vec![name.to_owned(), fallback.to_owned()]);
    }
    defs.families.insert(FontFamily::Proportional, vec![family::SANS.to_owned(), family::MONO.to_owned()]);
    defs.families.insert(FontFamily::Monospace, vec![family::MONO.to_owned(), family::SANS.to_owned()]);
    defs
}

/// Настраивает стиль egui под палитру (для стандартных виджетов: TextEdit, скролл, подсказки).
pub fn apply_style(ctx: &egui::Context, p: &Palette) {
    let mut visuals = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    visuals.panel_fill = p.panel;
    visuals.window_fill = p.panel;
    visuals.window_stroke = egui::Stroke::new(1.0, p.line);
    visuals.extreme_bg_color = p.bg;
    visuals.override_text_color = Some(p.fg);
    visuals.selection.bg_fill = p.live.gamma_multiply(0.45);
    visuals.selection.stroke = egui::Stroke::new(1.0, p.fg);
    visuals.text_cursor.stroke = egui::Stroke::new(1.0, p.fg);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.line);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, p.fg);
    // Поля ввода и списки в диалогах: фон `panel`, рамка `line`, радиус 4 (как поля 1c).
    visuals.extreme_bg_color = p.panel;
    let radius = egui::CornerRadius::same(4);
    for (w, stroke) in [
        (&mut visuals.widgets.inactive, p.line),
        (&mut visuals.widgets.hovered, p.mute),
        (&mut visuals.widgets.active, p.fg),
        (&mut visuals.widgets.open, p.mute),
    ] {
        w.bg_fill = p.panel;
        w.weak_bg_fill = p.panel;
        w.bg_stroke = egui::Stroke::new(1.0, stroke);
        w.corner_radius = radius;
        w.fg_stroke = egui::Stroke::new(1.0, p.fg);
    }
    visuals.window_corner_radius = egui::CornerRadius::same(4);
    visuals.popup_shadow = egui::epaint::Shadow::NONE;
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    ctx.set_visuals(visuals);
    ctx.global_style_mut(|s| {
        s.text_styles = [
            (egui::TextStyle::Body, sans(TEXT)),
            (egui::TextStyle::Button, sans(TEXT)),
            (egui::TextStyle::Small, sans(TEXT_CAPTION)),
            (egui::TextStyle::Monospace, mono(TEXT_SMALL)),
            (egui::TextStyle::Heading, sans_semibold(16.0)),
        ]
        .into();
        s.interaction.tooltip_delay = 0.4;
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
    });
}
