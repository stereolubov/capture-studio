//! Выводы (§10): мультиплексоры MKV/MP4/fMP4/FLV, RTMP(S), ремукс, менеджер выводов.
//! M2: MKV-мультиплексор, интерливер потоков; MP4/fMP4 и ремукс — M2c; RTMP — M3.

pub mod interleave;
pub mod mkv;

/// Политика переподключения (§10.2): до 25 попыток, задержка 2, 4, 8… максимум 30 с.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPolicy {
    pub max_attempts: u32,
    pub max_delay_secs: u32,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self { max_attempts: 25, max_delay_secs: 30 }
    }
}

impl ReconnectPolicy {
    /// Задержка перед попыткой `attempt` (с 1), секунды.
    pub fn delay_secs(&self, attempt: u32) -> u32 {
        2u32.saturating_pow(attempt.clamp(1, 16)).min(self.max_delay_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff() {
        let p = ReconnectPolicy::default();
        assert_eq!(p.delay_secs(1), 2);
        assert_eq!(p.delay_secs(2), 4);
        assert_eq!(p.delay_secs(3), 8);
        assert_eq!(p.delay_secs(5), 30);
        assert_eq!(p.delay_secs(25), 30);
    }
}
