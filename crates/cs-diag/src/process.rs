//! Метрики процесса: загрузка CPU (%) и рабочий набор (МБ) — для статус-бара (§11.2) и бенчмарков (§14.4).

/// Снимок метрик процесса.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ProcessSample {
    /// Суммарная загрузка CPU процессом в % от всех логических ядер.
    pub cpu_percent: f32,
    /// Рабочий набор, МБ.
    pub working_set_mb: f32,
    /// Частный объём памяти (commit), МБ.
    pub private_mb: f32,
}

/// Считает загрузку CPU между последовательными вызовами [`ProcessMonitor::sample`].
pub struct ProcessMonitor {
    #[cfg(windows)]
    last: Option<(u64, u64)>,
    #[cfg(windows)]
    cpus: u32,
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl ProcessMonitor {
    pub fn new() -> Self {
        use windows_sys::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
        // SAFETY: SYSTEM_INFO — POD-структура, GetSystemInfo заполняет её целиком.
        let cpus = unsafe {
            let mut si: SYSTEM_INFO = std::mem::zeroed();
            GetSystemInfo(&mut si);
            si.dwNumberOfProcessors.max(1)
        };
        Self { last: None, cpus }
    }

    /// Новый замер. Первый вызов возвращает `cpu_percent = 0`.
    pub fn sample(&mut self) -> ProcessSample {
        use windows_sys::Win32::Foundation::FILETIME;
        use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
        use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

        let ft = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
        let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut c, mut e, mut k, mut u, mut now) = (zero, zero, zero, zero, zero);
        // SAFETY: псевдодескриптор текущего процесса всегда валиден; все
        // указатели — на локальные переменные.
        let ok = unsafe {
            GetSystemTimeAsFileTime(&mut now);
            GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u)
        };
        let mut out = ProcessSample::default();
        if ok != 0 {
            let busy = ft(k) + ft(u);
            let wall = ft(now);
            if let Some((pb, pw)) = self.last
                && wall > pw
            {
                out.cpu_percent = ((busy - pb) as f64 / (wall - pw) as f64 * 100.0 / f64::from(self.cpus)) as f32;
            }
            self.last = Some((busy, wall));
        }

        // SAFETY: структура инициализирована нулями, `cb` задан корректно;
        // PROCESS_MEMORY_COUNTERS_EX совместима по префиксу с PROCESS_MEMORY_COUNTERS.
        unsafe {
            let mut pmc: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
            let cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
            pmc.cb = cb;
            if GetProcessMemoryInfo(GetCurrentProcess(), (&raw mut pmc).cast(), cb) != 0 {
                out.working_set_mb = pmc.WorkingSetSize as f32 / 1_048_576.0;
                out.private_mb = pmc.PrivateUsage as f32 / 1_048_576.0;
            }
        }
        out
    }
}

#[cfg(not(windows))]
impl ProcessMonitor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn sample(&mut self) -> ProcessSample {
        ProcessSample::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_is_sane() {
        let mut m = ProcessMonitor::new();
        let _ = m.sample();
        let s = m.sample();
        assert!(s.cpu_percent >= 0.0 && s.cpu_percent <= 100.0);
        #[cfg(windows)]
        assert!(s.working_set_mb > 0.0);
    }
}
