//! Применение команд редактирования к коллекции сцен (без GPU — покрыто тестами).

use cs_core::Command;
use cs_core::geometry;
use cs_core::history::History;
use cs_core::model::{Crop, Scene, SceneCollection, SceneId, SceneItem, SourceConfig, SourceId, Transform};
use cs_core::state::ItemAction;

/// Что изменилось после команды.
#[derive(Debug, Default, PartialEq)]
pub struct Effect {
    /// Коллекция изменилась (нужно автосохранение и обновление снимка).
    pub changed: bool,
    /// Сменилась активная сцена: индекс предыдущей (для перехода).
    pub switched_from: Option<usize>,
    /// Источники, которые нужно (пере)создать.
    pub create: Vec<SourceId>,
    /// Новый элемент, который стоит вписать в холст, когда у источника появится размер.
    pub fit_when_ready: Option<SourceId>,
}

/// Состояние редактора: коллекция + история.
pub struct SceneEditor {
    pub collection: SceneCollection,
    pub history: History<SceneCollection>,
}

impl SceneEditor {
    pub fn new(collection: SceneCollection) -> Self {
        let mut collection = collection;
        if collection.scenes.is_empty() {
            collection = SceneCollection::default();
        }
        collection.active_scene = collection.active_scene.min(collection.scenes.len() - 1);
        Self { collection, history: History::default() }
    }

    fn record(&mut self) {
        self.history.record(&self.collection);
    }

    fn item_mut(&mut self, item: usize) -> Option<&mut SceneItem> {
        self.collection.active_mut().and_then(|s| s.items.get_mut(item))
    }

    /// Применяет команду. `size_of` — текущий размер источника, `canvas` — размер холста.
    pub fn apply(
        &mut self,
        cmd: Command,
        size_of: &dyn Fn(SourceId) -> Option<(u32, u32)>,
        canvas: (u32, u32),
    ) -> Effect {
        let mut fx = Effect::default();
        let c = &mut self.collection;
        match cmd {
            Command::SelectScene(i) if i < c.scenes.len() && i != c.active_scene => {
                fx.switched_from = Some(c.active_scene);
                c.active_scene = i;
                fx.changed = true;
            }
            Command::AddScene(name) => {
                self.record();
                let c = &mut self.collection;
                c.scenes.push(Scene { id: SceneId::new(), name, hotkey: None, items: vec![] });
                fx.switched_from = Some(c.active_scene);
                c.active_scene = c.scenes.len() - 1;
                fx.changed = true;
            }
            Command::RemoveScene(i) if i < c.scenes.len() && c.scenes.len() > 1 => {
                self.record();
                let c = &mut self.collection;
                c.scenes.remove(i);
                c.active_scene = c.active_scene.min(c.scenes.len() - 1);
                c.prune_sources();
                fx.changed = true;
            }
            Command::DuplicateScene(i) if i < c.scenes.len() => {
                self.record();
                let c = &mut self.collection;
                let mut s = c.scenes[i].clone();
                s.id = SceneId::new();
                s.name = format!("{} 2", s.name);
                s.hotkey = None;
                c.scenes.insert(i + 1, s);
                fx.changed = true;
            }
            Command::RenameScene(i, name) if i < c.scenes.len() => {
                self.record();
                self.collection.scenes[i].name = name;
                fx.changed = true;
            }
            Command::ToggleSourceVisible(item) => {
                if self.collection.active().is_some_and(|s| item < s.items.len()) {
                    self.record();
                    if let Some(it) = self.item_mut(item) {
                        it.visible = !it.visible;
                    }
                    fx.changed = true;
                }
            }
            Command::ToggleItemLocked(item) => {
                if self.collection.active().is_some_and(|s| item < s.items.len()) {
                    self.record();
                    if let Some(it) = self.item_mut(item) {
                        it.locked = !it.locked;
                    }
                    fx.changed = true;
                }
            }
            Command::AddSource { name, kind } => {
                self.record();
                let id = SourceId::new();
                let c = &mut self.collection;
                c.sources.insert(id, SourceConfig { name, kind });
                if let Some(scene) = c.active_mut() {
                    scene.items.push(SceneItem {
                        source: id,
                        visible: true,
                        locked: false,
                        transform: Transform::default(),
                        crop: Crop::default(),
                    });
                }
                fx.create.push(id);
                fx.fit_when_ready = Some(id);
                fx.changed = true;
            }
            Command::UpdateSource { id, name, kind } => {
                if self.collection.sources.contains_key(&id) {
                    self.record();
                    self.collection.sources.insert(id, SourceConfig { name, kind });
                    fx.create.push(id);
                    fx.changed = true;
                }
            }
            Command::RemoveItem(item) => {
                if self.collection.active().is_some_and(|s| item < s.items.len()) {
                    self.record();
                    let c = &mut self.collection;
                    if let Some(s) = c.active_mut() {
                        s.items.remove(item);
                    }
                    c.prune_sources();
                    fx.changed = true;
                }
            }
            Command::MoveItem { from, to } => {
                let len = self.collection.active().map_or(0, |s| s.items.len());
                if from < len && to < len && from != to {
                    self.record();
                    if let Some(s) = self.collection.active_mut() {
                        let it = s.items.remove(from);
                        s.items.insert(to, it);
                    }
                    fx.changed = true;
                }
            }
            Command::SetItemTransform { item, transform, crop, record } => {
                if self.collection.active().is_some_and(|s| item < s.items.len()) {
                    if record {
                        self.record();
                    }
                    if let Some(it) = self.item_mut(item) {
                        it.transform = transform;
                        it.crop = crop;
                    }
                    fx.changed = true;
                }
            }
            Command::ItemAction { item, action } => {
                let Some(it) = self.collection.active().and_then(|s| s.items.get(item)).cloned() else {
                    return fx;
                };
                let Some(size) = size_of(it.source) else { return fx };
                self.record();
                let t = match action {
                    ItemAction::Fit => geometry::fit(&it.transform, &it.crop, size, canvas),
                    ItemAction::Stretch => geometry::stretch(&it.transform, &it.crop, size, canvas),
                    ItemAction::Center => geometry::center(&it.transform, &it.crop, size, canvas),
                    ItemAction::RotateCw => geometry::rotate_cw(&it.transform, &it.crop, size),
                    ItemAction::Reset => Transform::default(),
                };
                if let Some(it) = self.item_mut(item) {
                    it.transform = t;
                    if action == ItemAction::Reset {
                        it.crop = Crop::default();
                    }
                }
                fx.changed = true;
            }
            Command::Checkpoint => self.record(),
            Command::Undo | Command::Redo => {
                let prev_active = self.collection.active_scene;
                let restored = if cmd == Command::Undo {
                    self.history.undo(&self.collection)
                } else {
                    self.history.redo(&self.collection)
                };
                if let Some(c) = restored {
                    // Источники, которых не было в текущей версии (восстановлены отменой), нужно создать.
                    fx.create =
                        c.sources.keys().filter(|id| !self.collection.sources.contains_key(id)).copied().collect();
                    // Изменённые свойства — тоже пересоздать.
                    fx.create.extend(
                        c.sources
                            .iter()
                            .filter(|(id, cfg)| self.collection.sources.get(id).is_some_and(|old| old != *cfg))
                            .map(|(id, _)| *id),
                    );
                    self.collection = c;
                    if self.collection.active_scene != prev_active {
                        fx.switched_from = Some(prev_active);
                    }
                    fx.changed = true;
                }
            }
            Command::SetTransition(t) => {
                let mut t = t;
                t.duration_ms = t.duration_ms.clamp(100, 2000);
                self.collection.transition = t;
                fx.changed = true;
            }
            _ => {}
        }
        fx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::model::SourceKind;

    fn color() -> SourceKind {
        SourceKind::Color { rgba: [255, 0, 0, 255], width: 200, height: 100 }
    }

    fn editor_with_source() -> (SceneEditor, SourceId) {
        let mut e = SceneEditor::new(SceneCollection::default());
        let fx = e.apply(Command::AddSource { name: "Цвет".into(), kind: color() }, &|_| None, (1920, 1080));
        (e, fx.create[0])
    }

    #[test]
    fn add_move_remove_with_undo() {
        let (mut e, a) = editor_with_source();
        let fx = e.apply(Command::AddSource { name: "Цвет 2".into(), kind: color() }, &|_| None, (1920, 1080));
        let b = fx.create[0];
        let items = |e: &SceneEditor| {
            e.collection.active().map(|s| s.items.iter().map(|i| i.source).collect::<Vec<_>>()).unwrap_or_default()
        };
        assert_eq!(items(&e), vec![a, b]);
        e.apply(Command::MoveItem { from: 1, to: 0 }, &|_| None, (1920, 1080));
        assert_eq!(items(&e), vec![b, a]);
        e.apply(Command::RemoveItem(0), &|_| None, (1920, 1080));
        assert_eq!(items(&e), vec![a]);
        assert!(!e.collection.sources.contains_key(&b), "неиспользуемый источник удалён");
        // Отмена удаления восстанавливает источник и просит создать его заново.
        let fx = e.apply(Command::Undo, &|_| None, (1920, 1080));
        assert_eq!(items(&e), vec![b, a]);
        assert_eq!(fx.create, vec![b]);
        e.apply(Command::Redo, &|_| None, (1920, 1080));
        assert_eq!(items(&e), vec![a]);
    }

    #[test]
    fn drag_gesture_is_one_undo_step() {
        let (mut e, _) = editor_with_source();
        e.apply(Command::Checkpoint, &|_| None, (1920, 1080));
        for x in 1..=10 {
            let mut t = Transform::default();
            t.pos.x = x as f32 * 10.0;
            e.apply(
                Command::SetItemTransform { item: 0, transform: t, crop: Crop::default(), record: false },
                &|_| None,
                (1920, 1080),
            );
        }
        let pos = |e: &SceneEditor| e.collection.active().map_or(0.0, |s| s.items[0].transform.pos.x);
        assert_eq!(pos(&e), 100.0);
        e.apply(Command::Undo, &|_| None, (1920, 1080));
        assert_eq!(pos(&e), 0.0);
    }

    #[test]
    fn fit_action_uses_source_size() {
        let (mut e, a) = editor_with_source();
        e.apply(
            Command::ItemAction { item: 0, action: ItemAction::Fit },
            &|id| (id == a).then_some((200, 100)),
            (1920, 1080),
        );
        let t = e.collection.active().map(|s| s.items[0].transform).unwrap_or_default();
        assert_eq!(t.scale.x, 9.6);
        // 200×100 × 9.6 = 1920×960 → по вертикали центрируется: (1080 − 960) / 2 = 60.
        assert_eq!((t.pos.x, t.pos.y), (0.0, 60.0));
    }

    #[test]
    fn scene_switch_reports_previous() {
        let mut e = SceneEditor::new(SceneCollection::default());
        e.apply(Command::AddScene("Б".into()), &|_| None, (1920, 1080));
        assert_eq!(e.collection.active_scene, 1);
        let fx = e.apply(Command::SelectScene(0), &|_| None, (1920, 1080));
        assert_eq!(fx.switched_from, Some(1));
        let fx = e.apply(Command::SelectScene(0), &|_| None, (1920, 1080));
        assert_eq!(fx.switched_from, None);
    }
}
