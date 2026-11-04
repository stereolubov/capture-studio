//! Adaptive CBR: reduce during congestion; restore only after sustained healthy output.
use std::time::{Duration, Instant};
pub struct AdaptiveBitrate {
    base: u32,
    current: u32,
    last_change: Option<Instant>,
    healthy_since: Option<Instant>,
}
impl AdaptiveBitrate {
    pub fn new(base_kbps: u32) -> Self {
        let base = base_kbps.clamp(100, 1_000_000);
        Self { base, current: base, last_change: None, healthy_since: None }
    }
    pub fn observe(&mut self, congested: bool, healthy: bool, now: Instant) -> Option<u32> {
        let next = if congested {
            self.healthy_since = None;
            if self.last_change.is_some_and(|t| now.duration_since(t) < Duration::from_secs(2)) {
                return None;
            }
            (self.current * 4 / 5).max(self.base.div_ceil(2).max(100))
        } else if healthy {
            let since = *self.healthy_since.get_or_insert(now);
            if now.duration_since(since) < Duration::from_secs(5)
                || self.last_change.is_some_and(|t| now.duration_since(t) < Duration::from_secs(5))
            {
                return None;
            }
            self.current.saturating_add((self.base / 20).max(1)).min(self.base)
        } else {
            self.healthy_since = None;
            return None;
        };
        if next == self.current {
            return None;
        }
        self.current = next;
        self.last_change = Some(now);
        Some(next)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn congestion_is_rate_limited_and_bounded() {
        let mut p = AdaptiveBitrate::new(6000);
        let now = Instant::now();
        assert_eq!(p.observe(true, false, now), Some(4800));
        assert_eq!(p.observe(true, false, now + Duration::from_millis(500)), None);
        for sec in [2, 4, 6] {
            p.observe(true, false, now + Duration::from_secs(sec));
        }
        assert_eq!(p.current, 3000);
        assert_eq!(p.observe(true, false, now + Duration::from_secs(20)), None);
    }
    #[test]
    fn recovery_requires_continuous_health_and_stays_below_target() {
        let now = Instant::now();
        let mut p = AdaptiveBitrate::new(6000);
        p.observe(true, false, now);
        p.observe(false, true, now + Duration::from_secs(1));
        p.observe(false, false, now + Duration::from_secs(5));
        assert_eq!(p.observe(false, true, now + Duration::from_secs(6)), None);
        assert_eq!(p.observe(false, true, now + Duration::from_secs(10)), None);
        assert_eq!(p.observe(false, true, now + Duration::from_secs(11)), Some(5100));
        for sec in (16..=60).step_by(5) {
            p.observe(false, true, now + Duration::from_secs(sec));
        }
        assert_eq!(p.current, 6000);
    }
}
