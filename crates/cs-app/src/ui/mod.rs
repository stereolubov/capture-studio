//! UI по дизайн-проекту (§11): экраны 1a–1f.
//!
//! UI не содержит логики эфира: читает снимок `AppState` у движка и отправляет `Command`.
//! Окну (платформенному слою) UI сообщает геометрию собственного заголовка ([`Chrome`])
//! и команды ([`WindowCommand`]).

mod format;
mod main_window;
mod menus;
mod mini;
mod mixer;
pub mod preview;
mod settings;
mod source_dialog;
mod stats;
mod toasts;
pub mod widgets;

use crate::i18n::I18n;
use crate::theme::{self, Palette};
use cs_core::config::settings::ThemePref;
use cs_core::config::{self, DataDirs, Profile, Settings};
use cs_core::state::Engine;
use cs_core::{Command, Event};
use egui::{Context, Pos2, Rect, Vec2, pos2};
use std::path::PathBuf;

pub use settings::SettingsTab;

/// Режим окна.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Main,
    Mini,
}

/// Команды платформенному окну.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowCommand {
    Minimize,
    ToggleMaximize,
    Close,
    EnterMini,
    ExitMini,
    OpenFolder(PathBuf),
    CopyText(String),
}

/// Геометрия собственного заголовка для `WM_NCHITTEST` (в логических точках).
#[derive(Debug, Clone)]
pub struct Chrome {
    /// Область перетаскивания окна.
    pub caption: Rect,
    /// Кнопка «развернуть» (`HTMAXBUTTON` — для Snap Layouts Windows 11).
    pub max_button: Option<Rect>,
    /// Интерактивные элементы внутри `caption` (меню, кнопки) — не перетаскивают окно.
    pub interactive: Vec<Rect>,
    /// Мини-режим: окно перетаскивается за любое место, кроме `interactive`.
    pub drag_everywhere: bool,
    /// Окно можно растягивать за края.
    pub resizable: bool,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            caption: Rect::NOTHING,
            max_button: None,
            interactive: Vec::new(),
            drag_everywhere: false,
            resizable: false,
        }
    }
}

/// Состояние окна от платформы.
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowInfo {
    pub maximized: bool,
    /// Курсор над кнопкой «развернуть» (наведение приходит через non-client сообщения).
    pub max_hover: bool,
    pub system_dark: bool,
}

/// Открытые плавающие панели и диалоги.
#[derive(Default)]
struct Overlays {
    settings: Option<settings::SettingsDialog>,
    mixer: Option<mixer::MixerUi>,
    stats: Option<stats::StatsUi>,
    source: Option<source_dialog::SourceDialog>,
}

/// Корневой объект UI.
pub struct App {
    pub engine: Box<dyn Engine>,
    pub i18n: I18n,
    pub settings: Settings,
    pub profile: Profile,
    dirs: Option<DataDirs>,
    pub mode: Mode,
    info: WindowInfo,
    overlays: Overlays,
    menu: menus::MenuState,
    toasts: toasts::Toasts,
    main: main_window::MainUi,
    mini: mini::MiniUi,
    chrome: Chrome,
    commands: Vec<WindowCommand>,
    /// Последний применённый язык/тема (чтобы перенастраивать egui только при изменении).
    applied: Option<(String, Palette)>,
    /// Текстура превью сцены в egui (регистрирует платформенный слой).
    pub preview_texture: Option<egui::TextureId>,
    pub preview_ui: preview::PreviewUi,
    audio_dirty_since: Option<std::time::Instant>,
}

impl App {
    pub fn new(engine: Box<dyn Engine>, settings: Settings, profile: Profile, dirs: Option<DataDirs>) -> Self {
        Self {
            engine,
            i18n: I18n::new(&settings.language),
            settings,
            profile,
            dirs,
            mode: Mode::Main,
            info: WindowInfo::default(),
            overlays: Overlays::default(),
            menu: menus::MenuState::default(),
            toasts: toasts::Toasts::default(),
            main: main_window::MainUi::default(),
            mini: mini::MiniUi::default(),
            chrome: Chrome::default(),
            commands: Vec::new(),
            applied: None,
            preview_texture: None,
            preview_ui: preview::PreviewUi::default(),
            audio_dirty_since: None,
        }
    }

    /// Текущая палитра с учётом «Как в системе».
    pub fn palette(&self) -> Palette {
        let dark = match self.settings.theme {
            ThemePref::Dark => true,
            ThemePref::Light => false,
            ThemePref::System => self.info.system_dark,
        };
        if dark { Palette::DARK } else { Palette::LIGHT }
    }

    pub fn chrome(&self) -> &Chrome {
        &self.chrome
    }

    pub fn take_commands(&mut self) -> Vec<WindowCommand> {
        std::mem::take(&mut self.commands)
    }

    pub fn send(&mut self, cmd: Command) {
        tracing::debug!("команда: {cmd:?}");
        self.engine.send(cmd);
    }

    /// Открыть окно настроек на вкладке.
    pub fn open_settings(&mut self, tab: SettingsTab) {
        self.overlays.settings = Some(settings::SettingsDialog::new(self, tab));
    }

    pub fn open_mixer(&mut self, focus: Option<usize>) {
        self.overlays.mixer = Some(mixer::MixerUi::new(focus));
    }

    /// Окно свойств источника.
    pub fn open_source_dialog(&mut self, src: Option<cs_core::state::SourceView>) {
        if let Some(d) = src.as_ref().and_then(source_dialog::SourceDialog::edit) {
            self.overlays.source = Some(d);
        }
    }

    /// Окно добавления источника заданного типа.
    pub fn open_new_source(&mut self, kind: source_dialog::NewKind) {
        self.overlays.source = Some(source_dialog::SourceDialog::new_source(self, kind));
    }

    /// Открыто модальное окно (настройки или свойства источника).
    pub fn modal_open(&self) -> bool {
        self.overlays.settings.is_some() || self.overlays.source.is_some()
    }

    pub fn open_stats(&mut self) {
        self.overlays.stats = Some(stats::StatsUi::default());
    }

    /// Переносит фейдеры/mute/дорожки из движка в профиль и сохраняет через 1 с после изменения.
    fn sync_audio_settings(&mut self) {
        if !self.engine.state().live_engine {
            return;
        }
        let audio = self.engine.state().audio.clone();
        let mut changed = false;
        for (cfg, ch) in self.profile.audio.channels.iter_mut().zip(audio.iter()) {
            let fader_changed =
                cfg.fader_db != ch.fader_db && !(cfg.fader_db.is_infinite() && ch.fader_db.is_infinite());
            if fader_changed || cfg.muted != ch.muted || cfg.tracks != ch.tracks {
                cfg.fader_db = ch.fader_db;
                cfg.muted = ch.muted;
                cfg.tracks = ch.tracks;
                changed = true;
            }
        }
        let now = std::time::Instant::now();
        if changed {
            self.audio_dirty_since = Some(now);
        }
        if self.audio_dirty_since.is_some_and(|t| now - t >= std::time::Duration::from_secs(1)) {
            self.audio_dirty_since = None;
            if let Err(e) = self.save_config() {
                tracing::warn!("сохранение настроек аудио: {e}");
            }
        }
    }

    /// Применяет изменения языка/темы к egui.
    fn sync_style(&mut self, ctx: &Context) {
        let p = self.palette();
        let key = (self.settings.language.clone(), p);
        if self.applied.as_ref() != Some(&key) {
            if self.i18n.lang() != self.settings.language {
                self.i18n = I18n::new(&self.settings.language);
            }
            theme::apply_style(ctx, &p);
            self.applied = Some(key);
        }
    }

    /// Сохраняет настройки и профиль на диск (атомарно).
    pub fn save_config(&mut self) -> Result<(), config::ConfigError> {
        if let Some(d) = &self.dirs {
            config::save(&d.settings_file(), &self.settings)?;
            config::save(&d.profile_file(&self.settings.last_profile), &self.profile)?;
        }
        Ok(())
    }

    pub fn recordings_dir(&self) -> PathBuf {
        if let Some(d) = &self.settings.recordings_dir {
            return PathBuf::from(d);
        }
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        home.join("Videos").join("Capture Studio")
    }

    fn handle_events(&mut self, events: Vec<Event>) {
        use crate::i18n::s;
        for e in events {
            tracing::info!("событие: {e:?}");
            let (text, kind) = match e {
                Event::StreamStarted => (self.i18n.t("toast-stream-started"), toasts::Kind::Info),
                Event::StreamStopped => (self.i18n.t("toast-stream-stopped"), toasts::Kind::Info),
                Event::StreamReconnecting { attempt, max } => {
                    (self.i18n.t_args("toast-reconnecting", &[("n", s(attempt)), ("max", s(max))]), toasts::Kind::Warn)
                }
                Event::RecordingStarted => (self.i18n.t("toast-record-started"), toasts::Kind::Info),
                Event::RecordingStopped { path } => {
                    (self.i18n.t_args("toast-record-stopped", &[("path", s(path))]), toasts::Kind::Info)
                }
                Event::DiskSpaceLow { free_mb } => (
                    self.i18n.t_args("toast-disk-low", &[("gb", s(format!("{:.1}", free_mb as f64 / 1024.0)))]),
                    toasts::Kind::Warn,
                ),
                Event::DiskSpaceStopped => (self.i18n.t("toast-disk-stopped"), toasts::Kind::Error),
                Event::RemuxFinished { path } => {
                    (self.i18n.t_args("toast-remux-done", &[("path", s(path))]), toasts::Kind::Info)
                }
                Event::Error(msg) => (self.i18n.t_args("toast-error", &[("msg", s(msg))]), toasts::Kind::Error),
                Event::Info(msg) => (msg, toasts::Kind::Info),
            };
            self.toasts.push(text, kind);
        }
    }

    pub fn toast(&mut self, text: String) {
        self.toasts.push(text, toasts::Kind::Info);
    }

    /// Один кадр UI.
    pub fn frame(&mut self, ctx: &Context, info: WindowInfo) {
        self.info = info;
        self.sync_style(ctx);
        let events = self.engine.poll();
        self.handle_events(events);
        self.sync_audio_settings();
        self.chrome = Chrome::default();

        let screen = ctx.content_rect();
        match self.mode {
            Mode::Main => {
                self.chrome.resizable = !info.maximized;
                ctx.root_ui(|ui| {
                    main_window::draw(self, ui, screen);
                });
                self.draw_overlays(ctx, screen);
                menus::draw(self, ctx);
                self.local_hotkeys(ctx);
            }
            Mode::Mini => {
                self.chrome.drag_everywhere = true;
                ctx.root_ui(|ui| mini::draw(self, ui, screen));
            }
        }
        toasts::draw(self, ctx, screen);

        // Индикаторы уровня обновляются 30 раз в секунду (§8.2); таймеры — раз в секунду.
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }

    fn draw_overlays(&mut self, ctx: &Context, screen: Rect) {
        if let Some(mut m) = self.overlays.mixer.take() {
            let keep = mixer::window(self, ctx, &mut m, screen);
            if keep {
                self.overlays.mixer = Some(m);
            }
        }
        if let Some(mut s) = self.overlays.stats.take() {
            let keep = stats::window(self, ctx, &mut s, screen);
            if keep {
                self.overlays.stats = Some(s);
            }
        }
        if let Some(mut d) = self.overlays.source.take() {
            let keep = source_dialog::modal(self, ctx, &mut d, screen);
            if keep && self.overlays.source.is_none() {
                self.overlays.source = Some(d);
            }
        }
        if let Some(mut d) = self.overlays.settings.take() {
            let keep = settings::modal(self, ctx, &mut d, screen);
            if keep && self.overlays.settings.is_none() {
                self.overlays.settings = Some(d);
            }
        }
    }

    /// Локальные горячие клавиши окна (глобальные — M3, §13).
    fn local_hotkeys(&mut self, ctx: &Context) {
        if ctx.egui_wants_keyboard_input() || self.modal_open() {
            return;
        }
        // Undo/Redo операций со сценами (§7.2).
        let (undo, redo) = ctx.input(|i| {
            let ctrl = i.modifiers.ctrl && !i.modifiers.alt;
            (
                ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
                ctrl && (i.key_pressed(egui::Key::Y) || i.modifiers.shift && i.key_pressed(egui::Key::Z)),
            )
        });
        if undo {
            self.send(Command::Undo);
        } else if redo {
            self.send(Command::Redo);
        }
        let keys = [egui::Key::F1, egui::Key::F2, egui::Key::F3, egui::Key::F4];
        let pressed: Vec<usize> = ctx.input(|i| {
            keys.iter()
                .enumerate()
                .filter(|(_, k)| i.key_pressed(**k) && i.modifiers.is_none())
                .map(|(n, _)| n)
                .collect()
        });
        for n in pressed {
            self.send(Command::SelectScene(n));
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.menu.is_open() {
                self.menu.close();
            } else if self.overlays.stats.is_some() {
                self.overlays.stats = None;
            } else {
                self.overlays.mixer = None;
            }
        }
    }

    pub fn push_command(&mut self, c: WindowCommand) {
        self.commands.push(c);
    }

    /// Регистрирует интерактивный элемент заголовка (не перетаскивает окно).
    pub fn chrome_interactive(&mut self, r: Rect) {
        self.chrome.interactive.push(r);
    }
}

/// Экран для скриншот-тестов (§17.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
    Settings,
    Mixer,
    Stats,
    Mini,
    /// Главное окно с открытым окном свойств текстового источника.
    Source,
}

impl Screen {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "1a" | "1b" | "main" => Self::Main,
            "1c" | "settings" => Self::Settings,
            "1d" | "mixer" => Self::Mixer,
            "1e" | "stats" => Self::Stats,
            "1f" | "mini" => Self::Mini,
            "source" => Self::Source,
            _ => return None,
        })
    }
}

impl App {
    /// Логический размер экрана для скриншота.
    pub fn screen_size(&mut self, ctx: &Context, screen: Screen) -> Vec2 {
        match screen {
            // Рамка 1 px снаружи сетки 1280×760, как в дизайн-макете.
            Screen::Main => Vec2::new(1282.0, 762.0),
            Screen::Settings => settings::panel_size(self, ctx),
            Screen::Mixer => mixer::panel_size(self, ctx),
            Screen::Stats => stats::panel_size(self, ctx),
            Screen::Mini => mini::SIZE,
            Screen::Source => Vec2::new(1282.0, 762.0),
        }
    }

    /// Рисует один экран во весь кадр (скриншот-режим).
    pub fn screenshot_frame(&mut self, ctx: &Context, screen: Screen) {
        self.sync_style(ctx);
        let rect = Rect::from_min_size(pos2(0.0, 0.0), ctx.content_rect().size());
        ctx.root_ui(|ui| match screen {
            Screen::Main => main_window::draw(self, ui, rect),
            Screen::Settings => {
                let mut d = settings::SettingsDialog::new(self, SettingsTab::Output);
                settings::draw_panel(self, ui, &mut d, rect);
            }
            Screen::Mixer => {
                let mut m = mixer::MixerUi::new(None);
                mixer::draw_panel(self, ui, &mut m, rect);
            }
            Screen::Stats => {
                let mut s = stats::StatsUi::default();
                stats::draw_panel(self, ui, &mut s, rect);
            }
            Screen::Mini => {
                self.mode = Mode::Mini;
                mini::draw(self, ui, rect);
            }
            Screen::Source => main_window::draw(self, ui, rect),
        });
        if screen == Screen::Source {
            if self.overlays.source.is_none() {
                self.open_new_source(source_dialog::NewKind::Text);
            }
            if let Some(mut d) = self.overlays.source.take() {
                source_dialog::modal(self, ctx, &mut d, rect);
                self.overlays.source = Some(d);
            }
        }
    }
}

/// Логический размер окна мини-режима.
pub fn mini_size() -> Vec2 {
    mini::SIZE
}

/// Центрирует прямоугольник размера `size` в `area`, округляя к пикселям.
pub fn centered(area: Rect, size: Vec2) -> Rect {
    let min = area.center() - size / 2.0;
    Rect::from_min_size(Pos2::new(min.x.round(), min.y.round()), size)
}
