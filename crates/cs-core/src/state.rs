//! Снимок состояния приложения, команды и события (§5.2 п.6).
//!
//! UI получает `Arc<AppState>` (дешёвый снимок), команды отправляет через [`Command`],
//! а значимые происшествия (для тостов и лога) приходят как [`Event`].

use crate::model::{Crop, GameCaptureMethod, SceneId, SourceId, SourceKind, Transform, Transition};

/// Состояние вывода — конечный автомат §10.3.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputStatus {
    Idle,
    Starting,
    Live,
    Reconnecting { attempt: u32, max: u32 },
    Stopping,
    Error(String),
}

impl OutputStatus {
    /// Вывод активен (идёт эфир/запись или переподключение).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Live | Self::Reconnecting { .. })
    }

    /// Переходное состояние: кнопка управления должна быть неактивна.
    pub fn is_transitional(&self) -> bool {
        matches!(self, Self::Starting | Self::Stopping)
    }
}

/// Сцена в списке.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneView {
    pub id: SceneId,
    pub name: String,
    pub hotkey: Option<String>,
}

/// Данные для мета-строки источника (§11.2). Текст собирает UI с учётом локали.
#[derive(Debug, Clone, PartialEq)]
pub enum SourceMeta {
    Game {
        exe: String,
        width: u32,
        height: u32,
        /// Фактический метод захвата, если уже выбран (`хук D3D12`, `WGC`).
        method: Option<String>,
        mode: GameCaptureMethod,
    },
    Window {
        exe: String,
        width: u32,
        height: u32,
        method: Option<String>,
    },
    Monitor {
        index: u32,
        width: u32,
        height: u32,
        method: Option<String>,
    },
    Camera {
        device: String,
        mode: String,
    },
    Browser {
        width: u32,
        height: u32,
    },
    Image {
        format: String,
        width: u32,
        height: u32,
    },
    Color {
        width: u32,
        height: u32,
    },
    Text,
}

/// Источник активной сцены.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceView {
    pub id: SourceId,
    pub name: String,
    pub meta: SourceMeta,
    pub visible: bool,
    pub locked: bool,
    /// Ошибка источника — показывается в мета-строке цветом `warn`.
    pub error: Option<String>,
    /// Источник ждёт окно/устройство.
    pub waiting: bool,
    /// Окно источника свёрнуто (показывается последний кадр).
    pub minimized: bool,
    /// Индекс элемента в сцене (`Scene::items`, снизу вверх). Список `AppState::sources`
    /// упорядочен сверху вниз, как в интерфейсе.
    pub item: usize,
    pub transform: Transform,
    pub crop: Crop,
    /// Собственный размер источника (0×0 — ещё нет кадра).
    pub size: (u32, u32),
    /// Конфигурация источника (для окна свойств).
    pub kind: Option<SourceKind>,
}

/// Превью сцены: текстура холста на GPU. `generation` меняется при пересоздании текстуры.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewInfo {
    pub generation: u64,
    pub width: u32,
    pub height: u32,
}

/// Режим мониторинга канала (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MonitorMode {
    #[default]
    Off,
    MonitorOnly,
    MonitorAndOutput,
}

/// Аудиоканал микшера.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioChannel {
    pub name: String,
    /// Положение фейдера, дБ (−∞ = `f32::NEG_INFINITY`).
    pub fader_db: f32,
    pub muted: bool,
    pub monitor: MonitorMode,
    /// Пиковые уровни L/R, dBFS.
    pub peak: [f32; 2],
    /// RMS-уровни L/R, dBFS.
    pub rms: [f32; 2],
    /// Удерживаемый пик (1.5 с), dBFS.
    pub peak_hold: [f32; 2],
    /// Устройство канала недоступно (отключено) — показывается цветом `warn`.
    pub device_missing: bool,
    /// Битовая маска дорожек 1–6.
    pub tracks: u8,
    /// Смещение синхронизации, мс.
    pub sync_offset_ms: i32,
    /// Цепочка фильтров (§8.3).
    pub filters: Vec<crate::config::profile::AudioFilter>,
}

/// Метрики для статус-бара и окна статистики (§14.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stats {
    pub cpu_percent: f32,
    pub gpu_percent: f32,
    pub ram_mb: f32,
    pub fps: f32,
    pub target_fps: f32,
    pub bitrate_kbps: f32,
    pub dropped_frames: u64,
    pub total_frames: u64,
    pub render_ms: f32,
    pub rtt_ms: f32,
    pub transferred_bytes: u64,
    /// Пропуски по причинам: рендер / энкодер / сеть.
    pub dropped_render: u64,
    pub dropped_encoder: u64,
    pub dropped_network: u64,
    pub encoder_load_percent: f32,
    /// Средний битрейт по 10-секундным интервалам, последние 10 минут (≤ 60 значений).
    pub bitrate_history: Vec<f32>,
}

impl Stats {
    /// Доля потерянных кадров, %.
    pub fn dropped_percent(&self) -> f32 {
        if self.total_frames == 0 { 0.0 } else { self.dropped_frames as f32 * 100.0 / self.total_frames as f32 }
    }
}

/// Итог пробного кодирования энкодера (§9.1) — для списка энкодеров в настройках.
#[derive(Debug, Clone, PartialEq)]
pub struct EncoderStatus {
    pub kind: crate::config::profile::EncoderKind,
    /// `None` — энкодер работает; иначе причина недоступности.
    pub unavailable: Option<String>,
}

/// Краткая сводка настроек вывода для строки под превью.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputSummary {
    pub canvas: (u32, u32),
    pub output: (u32, u32),
    pub fps: f32,
    pub encoder: String,
    pub bitrate_kbps: u32,
    pub rate_control: String,
}

/// Полный снимок состояния для UI.
#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub scenes: Vec<SceneView>,
    pub active_scene: usize,
    pub sources: Vec<SourceView>,
    pub audio: Vec<AudioChannel>,
    pub stream: OutputStatus,
    pub stream_elapsed_secs: u64,
    pub record: OutputStatus,
    pub record_elapsed_secs: u64,
    pub record_paused: bool,
    pub stats: Stats,
    pub output: OutputSummary,
    pub preview_enabled: bool,
    /// `None` — превью недоступно (мок-режим, нет GPU).
    pub preview: Option<PreviewInfo>,
    pub transition: Transition,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Движок настоящий (не мок): старт трансляции/записи появится в M2–M3.
    pub live_engine: bool,
    /// Прогресс ремукса MKV → MP4 (0…1), пока он идёт.
    pub remux_progress: Option<f32>,
    /// Итоги пробного кодирования по порядку автовыбора (пусто, пока проба идёт).
    pub encoders: Vec<EncoderStatus>,
    /// Энкодер, который выбирает «Авто» (первый рабочий по итогам пробы).
    pub auto_encoder: Option<crate::config::profile::EncoderKind>,
}

/// Команды от UI (и горячих клавиш) к движку.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    SelectScene(usize),
    AddScene(String),
    RemoveScene(usize),
    DuplicateScene(usize),
    RenameScene(usize, String),
    /// Видимость элемента активной сцены (индекс `SceneItem`).
    ToggleSourceVisible(usize),
    ToggleItemLocked(usize),
    /// Добавить новый источник поверх активной сцены.
    AddSource {
        name: String,
        kind: SourceKind,
    },
    /// Изменить свойства источника (во всех сценах).
    UpdateSource {
        id: SourceId,
        name: String,
        kind: SourceKind,
    },
    RemoveItem(usize),
    /// Перенести элемент (индексы `SceneItem`, снизу вверх).
    MoveItem {
        from: usize,
        to: usize,
    },
    /// Трансформация элемента. `record = false` — промежуточный шаг перетаскивания
    /// (в историю попадает [`Command::Checkpoint`], отправленный в начале жеста).
    SetItemTransform {
        item: usize,
        transform: Transform,
        crop: Crop,
        record: bool,
    },
    ItemAction {
        item: usize,
        action: ItemAction,
    },
    /// Запомнить текущее состояние сцен в истории (перед жестом мышью).
    Checkpoint,
    Undo,
    Redo,
    SetTransition(Transition),
    /// Папка записей (передаётся UI при старте и после изменения настроек).
    SetRecordingsDir(std::path::PathBuf),
    /// Пауза / продолжение записи (MKV).
    TogglePauseRecording,
    StartStream,
    StopStream,
    StartRecording,
    StopRecording,
    SetFader {
        channel: usize,
        db: f32,
    },
    ToggleMute(usize),
    SetMonitor {
        channel: usize,
        mode: MonitorMode,
    },
    SetTracks {
        channel: usize,
        tracks: u8,
    },
    /// Смещение синхронизации канала, мс (−950…+2000).
    SetSyncOffset {
        channel: usize,
        ms: i32,
    },
    /// Цепочка фильтров канала целиком (добавление, удаление, порядок, параметры).
    SetFilters {
        channel: usize,
        filters: Vec<crate::config::profile::AudioFilter>,
    },
    SetPreviewEnabled(bool),
    /// Применить профиль (вывод, кодирование, видео). Движок перезапускает затронутые части.
    ApplyProfile(Box<crate::config::Profile>),
}

/// Действия контекстного меню элемента (§7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAction {
    Fit,
    Stretch,
    Center,
    Reset,
    RotateCw,
}

/// Важные события движка: показываются тостами и пишутся в лог.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    StreamStarted,
    StreamStopped,
    StreamReconnecting {
        attempt: u32,
        max: u32,
    },
    RecordingStarted,
    RecordingStopped {
        path: String,
    },
    /// Мало места на диске записей (предупреждение при < 5 ГБ).
    DiskSpaceLow {
        free_mb: u64,
    },
    /// Запись остановлена: свободно меньше 500 МБ (файл сохранён).
    DiskSpaceStopped,
    RemuxFinished {
        path: String,
    },
    Error(String),
    /// Информационное сообщение (тост).
    Info(String),
}

/// Интерфейс движка, которым пользуется UI. Реальный движок (M1+) и мок
/// (`--mock-ui`) реализуют его одинаково.
pub trait Engine {
    fn send(&mut self, cmd: Command);
    /// Продвигает внутреннее время (для мока) и возвращает новые события.
    fn poll(&mut self) -> Vec<Event>;
    fn state(&self) -> &AppState;
    /// Нативная текстура превью для платформенного слоя: (поколение, платформенный объект).
    /// На Windows — `ID3D11ShaderResourceView`. Мок превью не имеет.
    fn preview_handle(&self) -> Option<(u64, Box<dyn std::any::Any>)> {
        None
    }
}
