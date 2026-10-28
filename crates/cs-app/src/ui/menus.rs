//! Меню заголовка: Файл, Сцены, Вид, Настройки (§11.2).

use super::widgets as w;
use super::{App, Mode, WindowCommand};
use crate::theme;
use cs_core::Command;
use cs_core::config::settings::ThemePref;
use cs_core::model::{Transition, TransitionKind};
use egui::{Align2, Area, Context, Id, Order, Rect, Sense, pos2, vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    File,
    Scenes,
    View,
    Settings,
}

pub const TOP_MENUS: [(MenuKind, &str); 4] = [
    (MenuKind::File, "menu-file"),
    (MenuKind::Scenes, "menu-scenes"),
    (MenuKind::View, "menu-view"),
    (MenuKind::Settings, "menu-settings"),
];

#[derive(Default)]
pub struct MenuState {
    open: Option<(MenuKind, Rect)>,
    /// Кадр открытия: клик, открывший меню, не должен его сразу закрыть.
    opened_now: bool,
}

impl MenuState {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn is_open_kind(&self, k: MenuKind) -> bool {
        self.open.is_some_and(|(o, _)| o == k)
    }

    pub fn toggle(&mut self, k: MenuKind, anchor: Rect) {
        if self.is_open_kind(k) {
            self.open = None;
        } else {
            self.open = Some((k, anchor));
            self.opened_now = true;
        }
    }

    pub fn close(&mut self) {
        self.open = None;
    }
}

/// Пункт меню.
enum Item {
    Action { label: String, enabled: bool, checked: Option<bool>, action: Action },
    Separator,
}

#[derive(Clone, Copy)]
enum Action {
    None,
    OpenRecordings,
    Exit,
    AddScene,
    DuplicateScene,
    RemoveScene,
    Theme(ThemePref),
    Mini,
    Mixer,
    Stats,
    TogglePreview,
    Undo,
    Redo,
    Transition(TransitionKind, u32),
}

fn items(app: &App, kind: MenuKind) -> Vec<Item> {
    let t = |k: &str| app.i18n.t(k);
    let soon = |k: &str| format!("{} · {}", app.i18n.t(k), app.i18n.t("menu-soon"));
    let act = |label: String, action: Action| Item::Action { label, enabled: true, checked: None, action };
    match kind {
        MenuKind::File => vec![
            Item::Action { label: soon("menu-file-profiles"), enabled: false, checked: None, action: Action::None },
            Item::Action { label: soon("menu-file-collections"), enabled: false, checked: None, action: Action::None },
            act(t("menu-file-open-recordings"), Action::OpenRecordings),
            Item::Separator,
            act(t("menu-file-exit"), Action::Exit),
        ],
        MenuKind::Scenes => {
            let st = app.engine.state();
            let tr = st.transition;
            let mut v = vec![
                act(t("menu-scenes-add"), Action::AddScene),
                act(t("menu-scenes-duplicate"), Action::DuplicateScene),
                act(t("menu-scenes-remove"), Action::RemoveScene),
                Item::Separator,
                Item::Action {
                    label: t("menu-scenes-undo"),
                    enabled: st.can_undo,
                    checked: None,
                    action: Action::Undo,
                },
                Item::Action {
                    label: t("menu-scenes-redo"),
                    enabled: st.can_redo,
                    checked: None,
                    action: Action::Redo,
                },
                Item::Separator,
                Item::Action {
                    label: format!("{}: {}", t("menu-scenes-transition"), t("transition-cut")),
                    enabled: true,
                    checked: Some(tr.kind == TransitionKind::Cut),
                    action: Action::Transition(TransitionKind::Cut, tr.duration_ms),
                },
            ];
            for ms in [300u32, 500, 1000] {
                v.push(Item::Action {
                    label: app.i18n.t_args("transition-fade-ms", &[("ms", crate::i18n::s(ms))]),
                    enabled: true,
                    checked: Some(tr.kind == TransitionKind::Fade && tr.duration_ms == ms),
                    action: Action::Transition(TransitionKind::Fade, ms),
                });
            }
            v
        }
        MenuKind::View => {
            let th = app.settings.theme;
            let preview =
                if app.engine.state().preview_enabled { "menu-view-hide-preview" } else { "menu-view-show-preview" };
            vec![
                Item::Action {
                    label: t("theme-dark"),
                    enabled: true,
                    checked: Some(th == ThemePref::Dark),
                    action: Action::Theme(ThemePref::Dark),
                },
                Item::Action {
                    label: t("theme-light"),
                    enabled: true,
                    checked: Some(th == ThemePref::Light),
                    action: Action::Theme(ThemePref::Light),
                },
                Item::Action {
                    label: t("theme-system"),
                    enabled: true,
                    checked: Some(th == ThemePref::System),
                    action: Action::Theme(ThemePref::System),
                },
                Item::Separator,
                act(t("menu-view-mini"), Action::Mini),
                act(t("menu-view-mixer"), Action::Mixer),
                act(t("menu-view-stats"), Action::Stats),
                act(t(preview), Action::TogglePreview),
            ]
        }
        MenuKind::Settings => vec![],
    }
}

pub fn draw(app: &mut App, ctx: &Context) {
    let Some((kind, anchor)) = app.menu.open else {
        return;
    };
    let p = app.palette();
    let list = items(app, kind);
    let font = theme::sans(theme::TEXT);
    let row_h = 28.0;
    let width = list
        .iter()
        .filter_map(|i| match i {
            Item::Action { label, .. } => {
                Some(ctx.fonts_mut(|f| f.layout_no_wrap(label.clone(), font.clone(), p.fg).size().x))
            }
            Item::Separator => None,
        })
        .fold(160.0f32, f32::max)
        + 28.0
        + 24.0;
    let height: f32 = list.iter().map(|i| if matches!(i, Item::Separator) { 9.0 } else { row_h }).sum::<f32>() + 8.0;

    let mut chosen = None;
    let area = Area::new(Id::unique("menu-popup"))
        .fade_in(false)
        .order(Order::Foreground)
        .fixed_pos(pos2(anchor.left(), anchor.bottom() + 4.0))
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(rect, 4.0, p.panel);
            w::border(painter, rect, 4.0, p.line);
            let mut y = rect.top() + 4.0;
            for (n, item) in list.iter().enumerate() {
                match item {
                    Item::Separator => {
                        w::hline(painter, rect.left() + 8.0, rect.right() - 8.0, y + 4.0, p.line);
                        y += 9.0;
                    }
                    Item::Action { label, enabled, checked, action } => {
                        let row = Rect::from_min_size(pos2(rect.left() + 4.0, y), vec2(width - 8.0, row_h));
                        let sense = if *enabled { Sense::click() } else { Sense::hover() };
                        let resp = ui.interact(row, Id::unique(("menu-item", n)), sense);
                        if *enabled && resp.hovered() {
                            painter.rect_filled(row, 3.0, p.selected);
                        }
                        if *checked == Some(true) {
                            painter.circle_filled(pos2(row.left() + 12.0, row.center().y), 3.0, p.fg);
                        }
                        let color = if *enabled { p.fg } else { p.mute };
                        w::text(
                            painter,
                            pos2(row.left() + 24.0, row.center().y),
                            Align2::LEFT_CENTER,
                            label,
                            font.clone(),
                            color,
                        );
                        if resp.clicked() {
                            chosen = Some(*action);
                        }
                        y += row_h;
                    }
                }
            }
        });

    let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
        && !app.menu.opened_now
        && ctx.pointer_interact_pos().is_some_and(|pos| !area.response.rect.contains(pos) && !anchor.contains(pos));
    app.menu.opened_now = false;
    if clicked_outside {
        app.menu.close();
    }
    if let Some(a) = chosen {
        app.menu.close();
        run(app, a);
    }
}

fn run(app: &mut App, action: Action) {
    let active = app.engine.state().active_scene;
    match action {
        Action::None => {}
        Action::Undo => app.send(Command::Undo),
        Action::Redo => app.send(Command::Redo),
        Action::Transition(kind, duration_ms) => app.send(Command::SetTransition(Transition { kind, duration_ms })),
        Action::OpenRecordings => {
            let dir = app.recordings_dir();
            app.push_command(WindowCommand::OpenFolder(dir));
        }
        Action::Exit => app.push_command(WindowCommand::Close),
        Action::AddScene => {
            let n = app.engine.state().scenes.len() + 1;
            let name = app.i18n.t_args("scene-new-name", &[("n", crate::i18n::s(n))]);
            app.send(Command::AddScene(name));
        }
        Action::DuplicateScene => app.send(Command::DuplicateScene(active)),
        Action::RemoveScene => app.send(Command::RemoveScene(active)),
        Action::Theme(t) => {
            app.settings.theme = t;
            if let Err(e) = app.save_config() {
                tracing::warn!("не удалось сохранить тему: {e}");
            }
        }
        Action::Mini => {
            app.mode = Mode::Mini;
            app.push_command(WindowCommand::EnterMini);
        }
        Action::Mixer => app.open_mixer(None),
        Action::Stats => app.open_stats(),
        Action::TogglePreview => {
            let v = !app.engine.state().preview_enabled;
            app.settings.preview_enabled = v;
            app.send(Command::SetPreviewEnabled(v));
        }
    }
}
