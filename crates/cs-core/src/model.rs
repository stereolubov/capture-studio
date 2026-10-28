//! Модель данных сцен (§7.1).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Идентификатор источника (общий экземпляр захвата на все сцены).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceId(pub Uuid);

/// Идентификатор сцены.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SceneId(pub Uuid);

impl SourceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SourceId {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SceneId {
    fn default() -> Self {
        Self::new()
    }
}

/// Коллекция сцен — единица хранения в `scenes/<имя>.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneCollection {
    pub id: Uuid,
    pub name: String,
    pub scenes: Vec<Scene>,
    pub sources: BTreeMap<SourceId, SourceConfig>,
    /// Активная сцена (индекс в `scenes`).
    #[serde(default)]
    pub active_scene: usize,
    #[serde(default)]
    pub transition: Transition,
}

/// Переход между сценами (§7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    pub kind: TransitionKind,
    /// Длительность затухания, 100–2000 мс.
    pub duration_ms: u32,
}

impl Default for Transition {
    fn default() -> Self {
        Self { kind: TransitionKind::Fade, duration_ms: 300 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    Cut,
    #[default]
    Fade,
}

impl SceneCollection {
    /// Новая коллекция с одной пустой сценой.
    pub fn new(name: &str, first_scene: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_owned(),
            scenes: vec![Scene {
                id: SceneId::new(),
                name: first_scene.to_owned(),
                hotkey: Some("F1".into()),
                items: vec![],
            }],
            sources: BTreeMap::new(),
            active_scene: 0,
            transition: Transition::default(),
        }
    }

    pub fn active(&self) -> Option<&Scene> {
        self.scenes.get(self.active_scene)
    }

    pub fn active_mut(&mut self) -> Option<&mut Scene> {
        self.scenes.get_mut(self.active_scene)
    }

    /// Удаляет источники, которые не входят ни в одну сцену.
    pub fn prune_sources(&mut self) {
        let used: std::collections::BTreeSet<SourceId> =
            self.scenes.iter().flat_map(|s| s.items.iter().map(|i| i.source)).collect();
        self.sources.retain(|id, _| used.contains(id));
    }
}

impl crate::config::Versioned for SceneCollection {
    const SCHEMA_VERSION: u32 = 1;
    fn migrations() -> &'static [crate::config::migrate::Migration] {
        &[|_| {}]
    }
}

impl Default for SceneCollection {
    fn default() -> Self {
        Self::new("Основная", "Сцена 1")
    }
}

/// Сцена. `items` упорядочены снизу вверх.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub id: SceneId,
    pub name: String,
    #[serde(default)]
    pub hotkey: Option<String>,
    #[serde(default)]
    pub items: Vec<SceneItem>,
}

/// Вхождение источника в сцену.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneItem {
    pub source: SourceId,
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default)]
    pub crop: Crop,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

/// Поворот с шагом 90°.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Rot90 {
    #[default]
    R0,
    R90,
    R180,
    R270,
}

/// Режим вписывания источника в рамку.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundsMode {
    /// Вписать с сохранением пропорций.
    Fit,
    /// Растянуть без сохранения пропорций.
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub pos: Vec2,
    pub scale: Vec2,
    pub rotation: Rot90,
    pub bounds: Option<BoundsMode>,
}

impl Default for Transform {
    fn default() -> Self {
        Self { pos: Vec2::default(), scale: Vec2 { x: 1.0, y: 1.0 }, rotation: Rot90::R0, bounds: None }
    }
}

/// Обрезка в пикселях источника.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Crop {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

/// Метод захвата игры (§6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GameCaptureMethod {
    #[default]
    Auto,
    HookOnly,
    WgcOnly,
}

/// Метод захвата монитора (§6.1, методы C и D).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorMethod {
    /// WGC, если поддерживается, иначе DXGI Desktop Duplication.
    #[default]
    Auto,
    Wgc,
    Dxgi,
}

/// Конфигурация источника.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceConfig {
    pub name: String,
    pub kind: SourceKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SourceKind {
    GameCapture {
        /// `None` — режим «Авто» (приложение в фокусе в полноэкранном режиме).
        target_exe: Option<String>,
        window_class: Option<String>,
        method: GameCaptureMethod,
        capture_audio: bool,
        capture_cursor: bool,
    },
    WindowCapture {
        target_exe: String,
        window_class: String,
        capture_cursor: bool,
    },
    MonitorCapture {
        monitor_index: u32,
        capture_cursor: bool,
        #[serde(default)]
        method: MonitorMethod,
    },
    Camera {
        device_id: String,
        width: u32,
        height: u32,
        fps: u32,
    },
    Image {
        path: String,
    },
    Color {
        rgba: [u8; 4],
        width: u32,
        height: u32,
    },
    Text {
        text: String,
        font: String,
        size: f32,
        rgba: [u8; 4],
        #[serde(default)]
        outline_rgba: [u8; 4],
        /// Толщина обводки в пикселях (0 — без обводки).
        #[serde(default)]
        outline_width: f32,
    },
    /// v1.x, опциональный модуль (§7.4).
    Browser {
        url: String,
        width: u32,
        height: u32,
    },
}
