//! Упорядочивание пакетов нескольких потоков по времени перед мультиплексированием.
//!
//! Пакет выдаётся, когда во всех потоках есть данные (берётся самый ранний), либо когда
//! поток отстал больше чем на `max_lag` (например, аудио ещё не началось) — чтобы запись
//! не зависала, если один энкодер молчит.

use std::collections::VecDeque;

pub struct Interleaver<T> {
    queues: Vec<VecDeque<(i64, T)>>,
    max_lag: i64,
    newest: i64,
}

impl<T> Interleaver<T> {
    pub fn new(streams: usize, max_lag: i64) -> Self {
        Self { queues: (0..streams).map(|_| VecDeque::new()).collect(), max_lag, newest: i64::MIN }
    }

    pub fn push(&mut self, stream: usize, ts: i64, item: T) {
        self.newest = self.newest.max(ts);
        if let Some(q) = self.queues.get_mut(stream) {
            q.push_back((ts, item));
        }
    }

    fn earliest(&self) -> Option<usize> {
        self.queues
            .iter()
            .enumerate()
            .filter_map(|(i, q)| q.front().map(|(t, _)| (i, *t)))
            .min_by_key(|(_, t)| *t)
            .map(|(i, _)| i)
    }

    /// Следующий готовый пакет: (поток, время, данные).
    pub fn pop(&mut self) -> Option<(usize, i64, T)> {
        let i = self.earliest()?;
        let all_ready = self.queues.iter().all(|q| !q.is_empty());
        let ts = self.queues[i].front().map(|(t, _)| *t)?;
        if all_ready || self.newest - ts > self.max_lag {
            self.queues[i].pop_front().map(|(t, item)| (i, t, item))
        } else {
            None
        }
    }

    /// Всё оставшееся в порядке времени (при завершении записи).
    pub fn drain(&mut self) -> Vec<(usize, i64, T)> {
        let mut out = Vec::new();
        while let Some(i) = self.earliest() {
            if let Some((t, item)) = self.queues[i].pop_front() {
                out.push((i, t, item));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_by_time() {
        let mut il = Interleaver::new(2, 1000);
        il.push(0, 0, "v0");
        il.push(0, 33, "v1");
        assert!(il.pop().is_none(), "ждём аудио");
        il.push(1, 10, "a0");
        il.push(1, 31, "a1");
        let order: Vec<_> = std::iter::from_fn(|| il.pop()).map(|(_, t, _)| t).collect();
        assert_eq!(order, vec![0, 10, 31]);
        let rest: Vec<_> = il.drain().into_iter().map(|(_, t, _)| t).collect();
        assert_eq!(rest, vec![33]);
    }

    #[test]
    fn lagging_stream_does_not_block() {
        let mut il = Interleaver::new(2, 500);
        for t in (0..2000).step_by(100) {
            il.push(0, t, ());
        }
        // Аудио нет совсем: выдаются пакеты старше 500 мс от новейшего (1900): 0..=1300.
        let n = std::iter::from_fn(|| il.pop()).count();
        assert_eq!(n, 14);
    }
}
