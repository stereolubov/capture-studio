//! Undo/Redo операций со сценами (§7.2): снимки состояния, глубина 50.

/// Максимальная глубина истории.
pub const DEPTH: usize = 50;

/// История снимков значения `T`.
#[derive(Debug, Clone)]
pub struct History<T: Clone> {
    undo: Vec<T>,
    redo: Vec<T>,
    depth: usize,
}

impl<T: Clone> Default for History<T> {
    fn default() -> Self {
        Self::new(DEPTH)
    }
}

impl<T: Clone> History<T> {
    pub fn new(depth: usize) -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), depth: depth.max(1) }
    }

    /// Запоминает состояние *до* изменения. Новая правка сбрасывает ветку redo.
    pub fn record(&mut self, before: &T) {
        self.undo.push(before.clone());
        if self.undo.len() > self.depth {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Отмена: возвращает предыдущее состояние, текущее уходит в redo.
    pub fn undo(&mut self, current: &T) -> Option<T> {
        let prev = self.undo.pop()?;
        self.redo.push(current.clone());
        Some(prev)
    }

    /// Повтор отменённого.
    pub fn redo(&mut self, current: &T) -> Option<T> {
        let next = self.redo.pop()?;
        self.undo.push(current.clone());
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_cycle() {
        let mut h = History::default();
        let mut v = 0;
        for next in 1..=3 {
            h.record(&v);
            v = next;
        }
        v = h.undo(&v).expect("undo");
        assert_eq!(v, 2);
        v = h.undo(&v).expect("undo");
        assert_eq!(v, 1);
        v = h.redo(&v).expect("redo");
        assert_eq!(v, 2);
        // Новая правка сбрасывает redo.
        h.record(&v);
        v = 10;
        assert!(!h.can_redo());
        assert_eq!(h.undo(&v), Some(2));
    }

    #[test]
    fn depth_limit() {
        let mut h = History::new(DEPTH);
        for i in 0..100 {
            h.record(&i);
        }
        let mut v = 100;
        let mut n = 0;
        while let Some(p) = h.undo(&v) {
            v = p;
            n += 1;
        }
        assert_eq!(n, DEPTH);
        assert_eq!(v, 50);
    }
}
