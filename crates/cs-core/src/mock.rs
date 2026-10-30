//! Мок-движок для `--mock-ui` и скриншот-тестов (§17.6).
//!
//! Начальное состояние повторяет значения из дизайн-проекта (сцены «Игра/Камера/
//! Перерыв/Финал», источники, уровни, статистика). В анимированном режиме уровни,
//! битрейт и таймеры «живут», а команды старта/остановки проходят через
//! переходные состояния, как у настоящего движка.

use crate::clock::Ticks;
use crate::model::{Crop, GameCaptureMethod, SceneId, SourceId, Transform, Transition};
use crate::state::{
    AppState, AudioChannel, Command, Engine, Event, MonitorMode, OutputStatus, OutputSummary, SceneView, SourceMeta,
    SourceView, Stats,
};

/// Длительность переходов Starting/Stopping в моке.
const TRANSITION_SECS: f64 = 0.8;
/// Время удержания пика (§8.2).
const PEAK_HOLD_SECS: f64 = 1.5;

/// Мок-движок.
pub struct MockEngine {
    state: AppState,
    animate: bool,
    last_tick: Ticks,
    stream_transition: Option<Ticks>,
    record_transition: Option<Ticks>,
    stream_secs: f64,
    record_secs: f64,
    rng: u64,
    peak_hold_age: Vec<[f64; 2]>,
    bitrate_bucket: (f64, u32),
    events: Vec<Event>,
}

/// Уровни из дизайна (ширина заливки индикатора, доли шкалы −60…0 dBFS).
fn level_from_fraction(f: f32) -> f32 {
    -60.0 + 60.0 * f
}

/// Высоты столбцов графика битрейта из дизайна (в долях от 6000 kbps·1.28).
/// Итоги пробы для дизайна: машина с GPU NVIDIA.
fn design_encoders() -> Vec<crate::state::EncoderStatus> {
    use crate::config::profile::EncoderKind;
    let st = |kind, why: Option<&str>| crate::state::EncoderStatus { kind, unavailable: why.map(Into::into) };
    vec![
        st(EncoderKind::Nvenc, None),
        st(EncoderKind::Amf, Some("нет GPU AMD")),
        st(EncoderKind::Qsv, Some("нет GPU Intel")),
        st(EncoderKind::MediaFoundation, None),
        st(EncoderKind::Software, Some("OpenH264 не установлен")),
    ]
}

fn design_bitrate_history() -> Vec<f32> {
    (0..60)
        .map(|i| {
            let i = i as f32;
            let pct = 78.0 + ((i * 0.7).sin() * 8.0 + (i * 1.9).cos() * 5.0).round();
            // В дизайне 100% высоты ≈ 7700 kbps: 78% ≈ 6000 kbps.
            pct / 100.0 * 7700.0
        })
        .collect()
}

impl MockEngine {
    /// Создаёт мок. `animate = false` — статичный снимок дизайна (для скриншот-тестов).
    pub fn new(animate: bool) -> Self {
        let scenes = ["Игра", "Камера", "Перерыв", "Финал"]
            .iter()
            .enumerate()
            .map(|(i, n)| SceneView { id: SceneId::new(), name: (*n).to_owned(), hotkey: Some(format!("F{}", i + 1)) })
            .collect();

        let sources = vec![
            SourceView {
                id: SourceId::new(),
                name: "Захват игры".into(),
                meta: SourceMeta::Game {
                    exe: "cyberpunk.exe".into(),
                    width: 1920,
                    height: 1080,
                    method: None,
                    mode: GameCaptureMethod::Auto,
                },
                visible: true,
                locked: false,
                error: None,
                waiting: false,
                minimized: false,
                item: 3,
                transform: Transform::default(),
                crop: Crop::default(),
                size: (0, 0),
                kind: None,
            },
            SourceView {
                id: SourceId::new(),
                name: "Веб-камера".into(),
                meta: SourceMeta::Camera { device: "Logitech C920".into(), mode: "720p30".into() },
                visible: true,
                locked: false,
                error: None,
                waiting: false,
                minimized: false,
                item: 2,
                transform: Transform::default(),
                crop: Crop::default(),
                size: (0, 0),
                kind: None,
            },
            SourceView {
                id: SourceId::new(),
                name: "Оверлей чата".into(),
                meta: SourceMeta::Browser { width: 400, height: 600 },
                visible: false,
                locked: false,
                error: None,
                waiting: false,
                minimized: false,
                item: 1,
                transform: Transform::default(),
                crop: Crop::default(),
                size: (0, 0),
                kind: None,
            },
            SourceView {
                id: SourceId::new(),
                name: "Логотип".into(),
                meta: SourceMeta::Image { format: "PNG".into(), width: 256, height: 256 },
                visible: true,
                locked: false,
                error: None,
                waiting: false,
                minimized: false,
                item: 0,
                transform: Transform::default(),
                crop: Crop::default(),
                size: (0, 0),
                kind: None,
            },
        ];

        let chan = |name: &str, db: f32, l: f32, r: f32| {
            let l = level_from_fraction(l);
            let r = level_from_fraction(r);
            AudioChannel {
                name: name.into(),
                fader_db: db,
                muted: false,
                monitor: MonitorMode::Off,
                peak: [l, r],
                rms: [l - 6.0, r - 6.0],
                peak_hold: [l, r],
                device_missing: false,
                tracks: 0b1,
                sync_offset_ms: 0,
                filters: Vec::new(),
            }
        };
        let audio = vec![
            chan("Рабочий стол", -12.4, 0.62, 0.58),
            chan("Микрофон", -6.1, 0.81, 0.77),
            chan("Музыка", -24.0, 0.34, 0.30),
        ];

        let stats = Stats {
            cpu_percent: 14.0,
            gpu_percent: 31.0,
            ram_mb: 52.0,
            fps: 60.0,
            target_fps: 60.0,
            bitrate_kbps: 6012.0,
            dropped_frames: 0,
            total_frames: 302_820,
            render_ms: 1.8,
            rtt_ms: 38.0,
            transferred_bytes: 3_700_000_000,
            dropped_render: 0,
            dropped_encoder: 0,
            dropped_network: 0,
            encoder_load_percent: 9.0,
            bitrate_history: design_bitrate_history(),
        };

        let audio_len = audio.len();
        Self {
            state: AppState {
                scenes,
                active_scene: 0,
                sources,
                audio,
                stream: OutputStatus::Live,
                stream_elapsed_secs: 5047,
                record: OutputStatus::Idle,
                record_elapsed_secs: 0,
                record_paused: false,
                stats,
                output: OutputSummary {
                    canvas: (1920, 1080),
                    output: (1920, 1080),
                    fps: 60.0,
                    encoder: "NVENC H.264".into(),
                    bitrate_kbps: 6000,
                    rate_control: "CBR".into(),
                },
                preview_enabled: true,
                preview: None,
                transition: Transition::default(),
                can_undo: false,
                can_redo: false,
                live_engine: false,
                remux_progress: None,
                encoders: design_encoders(),
                auto_encoder: Some(crate::config::profile::EncoderKind::Nvenc),
            },
            animate,
            last_tick: Ticks::now(),
            stream_transition: None,
            record_transition: None,
            stream_secs: 5047.0,
            record_secs: 0.0,
            rng: 0x9E37_79B9_7F4A_7C15,
            peak_hold_age: vec![[0.0; 2]; audio_len],
            bitrate_bucket: (0.0, 0),
            events: Vec::new(),
        }
    }

    /// Мутабельный доступ к состоянию (для подготовки сценариев скриншот-тестов).
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    fn rand(&mut self) -> f32 {
        // xorshift64*: детерминированный и без зависимостей.
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let v = self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (v >> 40) as f32 / (1u64 << 24) as f32
    }

    fn advance(&mut self, dt: f64) {
        let now = Ticks::now();

        // Переходы автомата вывода.
        if let Some(t) = self.stream_transition
            && now.seconds_since(t) >= TRANSITION_SECS
        {
            self.stream_transition = None;
            match self.state.stream {
                OutputStatus::Starting => {
                    self.state.stream = OutputStatus::Live;
                    self.stream_secs = 0.0;
                    self.state.stats.dropped_frames = 0;
                    self.state.stats.total_frames = 0;
                    self.state.stats.transferred_bytes = 0;
                    self.events.push(Event::StreamStarted);
                }
                OutputStatus::Stopping => {
                    self.state.stream = OutputStatus::Idle;
                    self.events.push(Event::StreamStopped);
                }
                _ => {}
            }
        }
        if let Some(t) = self.record_transition
            && now.seconds_since(t) >= TRANSITION_SECS / 2.0
        {
            self.record_transition = None;
            match self.state.record {
                OutputStatus::Starting => {
                    self.state.record = OutputStatus::Live;
                    self.record_secs = 0.0;
                    self.events.push(Event::RecordingStarted);
                }
                OutputStatus::Stopping => {
                    self.state.record = OutputStatus::Idle;
                    self.events.push(Event::RecordingStopped {
                        path: "Видео\\Capture Studio\\2026-10-28 17-00-00.mkv".into(),
                    });
                }
                _ => {}
            }
        }

        let streaming = self.state.stream.is_active();
        if streaming {
            self.stream_secs += dt;
            self.state.stream_elapsed_secs = self.stream_secs as u64;
        }
        if self.state.record.is_active() {
            self.record_secs += dt;
            self.state.record_elapsed_secs = self.record_secs as u64;
        }

        // Уровни: случайное блуждание вокруг базового уровня канала.
        for i in 0..self.state.audio.len() {
            let base = match i {
                0 => -22.0,
                1 => -12.0,
                _ => -38.0,
            };
            for c in 0..2 {
                let target = base + (self.rand() - 0.5) * 14.0;
                let ch = &mut self.state.audio[i];
                let cur = ch.peak[c];
                // Быстрая атака, медленный спад — как у реального индикатора.
                let k = if target > cur { 0.6 } else { 0.15 };
                let v = if ch.muted { -60.0 } else { cur + (target - cur) * k };
                ch.peak[c] = v.clamp(-60.0, 0.0);
                ch.rms[c] = (ch.peak[c] - 6.0).max(-60.0);
                self.peak_hold_age[i][c] += dt;
                if ch.peak[c] >= ch.peak_hold[c] || self.peak_hold_age[i][c] > PEAK_HOLD_SECS {
                    ch.peak_hold[c] = ch.peak[c];
                    self.peak_hold_age[i][c] = 0.0;
                }
            }
        }

        // Статистика.
        let r: [f32; 6] = std::array::from_fn(|_| self.rand());
        let s = &mut self.state.stats;
        s.cpu_percent = (s.cpu_percent + (r[0] - 0.5) * 2.0).clamp(9.0, 19.0);
        s.gpu_percent = (s.gpu_percent + (r[1] - 0.5) * 3.0).clamp(24.0, 38.0);
        s.fps = 60.0 - if r[2] > 0.97 { 0.1 } else { 0.0 };
        if streaming {
            s.bitrate_kbps = (6000.0 + (r[3] - 0.5) * 120.0).round();
            s.total_frames += (dt * 60.0) as u64;
            s.transferred_bytes += (f64::from(s.bitrate_kbps) * 1000.0 / 8.0 * dt) as u64;
            s.rtt_ms = (s.rtt_ms + (r[4] - 0.5) * 2.0).clamp(30.0, 48.0);
            self.bitrate_bucket.0 += f64::from(s.bitrate_kbps);
            self.bitrate_bucket.1 += 1;
        } else {
            s.bitrate_kbps = 0.0;
        }
        let r = r[5];
        s.render_ms = (1.8 + (r - 0.5) * 0.2).max(0.5);
    }

    /// Закрывает 10-секундный интервал графика битрейта.
    fn push_bitrate_bucket(&mut self) {
        let (sum, n) = self.bitrate_bucket;
        if n > 0 {
            let h = &mut self.state.stats.bitrate_history;
            h.push((sum / f64::from(n)) as f32);
            if h.len() > 60 {
                h.remove(0);
            }
        }
        self.bitrate_bucket = (0.0, 0);
    }
}

impl Engine for MockEngine {
    fn send(&mut self, cmd: Command) {
        let st = &mut self.state;
        match cmd {
            Command::SelectScene(i) if i < st.scenes.len() => st.active_scene = i,
            Command::SelectScene(_) => {}
            Command::AddScene(name) => st.scenes.push(SceneView { id: SceneId::new(), name, hotkey: None }),
            Command::RemoveScene(i) if i < st.scenes.len() && st.scenes.len() > 1 => {
                st.scenes.remove(i);
                st.active_scene = st.active_scene.min(st.scenes.len() - 1);
            }
            Command::RemoveScene(_) => {}
            Command::DuplicateScene(i) => {
                if let Some(s) = st.scenes.get(i).cloned() {
                    st.scenes
                        .insert(i + 1, SceneView { id: SceneId::new(), name: format!("{} 2", s.name), hotkey: None });
                }
            }
            Command::RenameScene(i, name) => {
                if let Some(s) = st.scenes.get_mut(i) {
                    s.name = name;
                }
            }
            Command::ToggleSourceVisible(item) => {
                if let Some(s) = st.sources.iter_mut().find(|s| s.item == item) {
                    s.visible = !s.visible;
                }
            }
            Command::ToggleItemLocked(item) => {
                if let Some(s) = st.sources.iter_mut().find(|s| s.item == item) {
                    s.locked = !s.locked;
                }
            }
            Command::SetTransition(t) => st.transition = t,
            Command::SetRecordingsDir(_) => {}
            Command::TogglePauseRecording => st.record_paused = !st.record_paused,
            // Редактирование сцен в моке не поддерживается: только визуальная оболочка.
            Command::AddSource { .. }
            | Command::UpdateSource { .. }
            | Command::RemoveItem(_)
            | Command::MoveItem { .. }
            | Command::SetItemTransform { .. }
            | Command::ItemAction { .. }
            | Command::Checkpoint
            | Command::Undo
            | Command::Redo => {}
            Command::StartStream if st.stream == OutputStatus::Idle => {
                st.stream = OutputStatus::Starting;
                self.stream_transition = Some(Ticks::now());
            }
            Command::StopStream if st.stream.is_active() => {
                st.stream = OutputStatus::Stopping;
                self.stream_transition = Some(Ticks::now());
            }
            Command::StartRecording if st.record == OutputStatus::Idle => {
                st.record = OutputStatus::Starting;
                self.record_transition = Some(Ticks::now());
            }
            Command::StopRecording if st.record.is_active() => {
                st.record = OutputStatus::Stopping;
                self.record_transition = Some(Ticks::now());
            }
            Command::StartStream | Command::StopStream | Command::StartRecording | Command::StopRecording => {}
            Command::SetFader { channel, db } => {
                if let Some(c) = st.audio.get_mut(channel) {
                    c.fader_db = db;
                }
            }
            Command::ToggleMute(i) => {
                if let Some(c) = st.audio.get_mut(i) {
                    c.muted = !c.muted;
                }
            }
            Command::SetMonitor { channel, mode } => {
                if let Some(c) = st.audio.get_mut(channel) {
                    c.monitor = mode;
                }
            }
            Command::SetPreviewEnabled(v) => st.preview_enabled = v,
            Command::SetSyncOffset { channel, ms } => {
                if let Some(c) = st.audio.get_mut(channel) {
                    c.sync_offset_ms = ms;
                }
            }
            Command::SetFilters { channel, filters } => {
                if let Some(c) = st.audio.get_mut(channel) {
                    c.filters = filters;
                }
            }
            Command::SetTracks { channel, tracks } => {
                if let Some(c) = st.audio.get_mut(channel) {
                    c.tracks = tracks;
                }
            }
            Command::ApplyProfile(p) => {
                use crate::config::profile::EncoderKind;
                let o = &mut st.output;
                o.bitrate_kbps = p.output.bitrate_kbps;
                o.rate_control = p.output.rate_control.label().into();
                // В моке «Авто» показывает NVENC, как в дизайне.
                o.encoder = if p.output.encoder == EncoderKind::Auto {
                    "NVENC H.264".into()
                } else {
                    p.output.encoder.label().into()
                };
                if let Some(c) = p.video.canvas {
                    o.canvas = c;
                }
                o.output = p.video.output.unwrap_or(o.canvas);
                o.fps = p.video.fps.0 as f32 / p.video.fps.1.max(1) as f32;
                st.stats.target_fps = o.fps;
            }
        }
    }

    fn poll(&mut self) -> Vec<Event> {
        let now = Ticks::now();
        let dt = now.seconds_since(self.last_tick);
        if self.animate && dt > 0.0 {
            self.last_tick = now;
            let before = self.stream_secs;
            self.advance(dt.min(0.5));
            if (self.stream_secs / 10.0).floor() > (before / 10.0).floor() {
                self.push_bitrate_bucket();
            }
        }
        std::mem::take(&mut self.events)
    }

    fn state(&self) -> &AppState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_snapshot_values() {
        let m = MockEngine::new(false);
        let s = m.state();
        assert_eq!(s.scenes.len(), 4);
        assert_eq!(s.scenes[0].hotkey.as_deref(), Some("F1"));
        assert_eq!(s.sources.len(), 4);
        assert!(!s.sources[2].visible);
        assert_eq!(s.stream, OutputStatus::Live);
        assert_eq!(crate::clock::format_hms(s.stream_elapsed_secs), "01:24:07");
        assert_eq!(s.stats.bitrate_history.len(), 60);
    }

    #[test]
    fn stream_state_machine() {
        let mut m = MockEngine::new(false);
        m.send(Command::StopStream);
        assert_eq!(m.state().stream, OutputStatus::Stopping);
        // Повторная остановка в переходном состоянии игнорируется.
        m.send(Command::StopStream);
        assert_eq!(m.state().stream, OutputStatus::Stopping);
        m.send(Command::StartStream);
        assert_eq!(m.state().stream, OutputStatus::Stopping);
    }

    #[test]
    fn scene_commands() {
        let mut m = MockEngine::new(false);
        m.send(Command::SelectScene(2));
        assert_eq!(m.state().active_scene, 2);
        m.send(Command::SelectScene(99));
        assert_eq!(m.state().active_scene, 2);
        m.send(Command::RemoveScene(3));
        assert_eq!(m.state().scenes.len(), 3);
        assert_eq!(m.state().active_scene, 2);
        // Оверлей чата — третий сверху, элемент №1 снизу.
        m.send(Command::ToggleSourceVisible(1));
        assert!(m.state().sources[2].visible);
    }
}
