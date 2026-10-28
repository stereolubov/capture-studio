//! Рендер-поток (§5.4): тик по высокоточному таймеру с частотой холста, команды, источники,
//! компоновка, метрики, публикация снимка, автосохранение коллекции через 1 с после изменения.

use super::scene_ops::{Effect, SceneEditor};
use super::{EngineConfig, Shared};
use crossbeam_channel::{Receiver, Sender};
use cs_audio::engine::{AudioEngine, ChannelConfig};
use cs_audio::mixer::ChannelControl;
use cs_audio::wasapi::Direction;
use cs_capture::{CaptureContext, Content, SourceState, VideoSource, enumerate};
use cs_core::config::profile::AudioChannelKind;
use cs_core::config::{self, Profile};
use cs_core::geometry;
use cs_core::model::{Scene, SceneCollection, SourceId, SourceKind};
use cs_core::state::{AudioChannel, MonitorMode};
use cs_core::state::{OutputStatus, OutputSummary, PreviewInfo, SceneView, SourceMeta, SourceView, Stats};
use cs_core::{AppState, Command, Event};
use cs_diag::process::ProcessMonitor;
use cs_gpu::{Compositor, DrawItem, DrawSource, GpuDevice, TextRenderer};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const AUTOSAVE_DELAY: Duration = Duration::from_secs(1);

/// Размер холста: из профиля или по основному монитору (§7.3).
fn canvas_size(profile: &Profile) -> (u32, u32) {
    profile.video.canvas.unwrap_or_else(|| {
        enumerate::monitors().into_iter().find(|m| m.primary).map_or((1920, 1080), |m| (m.width, m.height))
    })
}

fn fps_of(profile: &Profile) -> f64 {
    let (n, d) = profile.video.fps;
    (f64::from(n) / f64::from(d.max(1))).clamp(1.0, 240.0)
}

fn output_summary(profile: &Profile, canvas: (u32, u32)) -> OutputSummary {
    OutputSummary {
        canvas,
        output: profile.video.output.unwrap_or(canvas),
        fps: fps_of(profile) as f32,
        encoder: profile.output.encoder.label().into(),
        bitrate_kbps: profile.output.bitrate_kbps,
        rate_control: profile.output.rate_control.label().into(),
    }
}

/// Снимок до запуска потока (чтобы UI сразу показал сцены).
pub(super) fn initial_state(cfg: &EngineConfig) -> AppState {
    let canvas = canvas_size(&cfg.profile);
    build_state(&cfg.collection, &HashMap::new(), &cfg.profile, canvas, &Stats::default(), None, false, false)
}

fn image_format(path: &str) -> String {
    path.rsplit('.').next().map(str::to_ascii_uppercase).unwrap_or_default()
}

/// Размер содержимого с учётом поворота монитора (DXGI).
fn effective_size(content: &Content) -> ((u32, u32), u8) {
    match content {
        Content::Texture { width, height, rotation, .. } => {
            if rotation % 2 == 1 {
                ((*height, *width), *rotation)
            } else {
                ((*width, *height), *rotation)
            }
        }
        Content::Color { width, height, .. } => ((*width, *height), 0),
    }
}

#[expect(clippy::too_many_arguments, reason = "внутренняя функция сборки снимка")]
fn build_state(
    c: &SceneCollection,
    sources: &HashMap<SourceId, Box<dyn VideoSource>>,
    profile: &Profile,
    canvas: (u32, u32),
    stats: &Stats,
    preview: Option<PreviewInfo>,
    can_undo: bool,
    can_redo: bool,
) -> AppState {
    let scenes =
        c.scenes.iter().map(|s| SceneView { id: s.id, name: s.name.clone(), hotkey: s.hotkey.clone() }).collect();
    let mut views = Vec::new();
    if let Some(scene) = c.active() {
        for (idx, item) in scene.items.iter().enumerate().rev() {
            let Some(cfg) = c.sources.get(&item.source) else { continue };
            let src = sources.get(&item.source);
            let size = src.and_then(|s| s.content()).map_or((0, 0), |ct| effective_size(&ct).0);
            let state = src.map_or(SourceState::Waiting, |s| s.state());
            let method = src.map(|s| s.method().label()).filter(|m| !m.is_empty());
            let meta = match &cfg.kind {
                SourceKind::MonitorCapture { monitor_index, .. } => {
                    SourceMeta::Monitor { index: *monitor_index, width: size.0, height: size.1, method }
                }
                SourceKind::WindowCapture { target_exe, .. } => {
                    SourceMeta::Window { exe: target_exe.clone(), width: size.0, height: size.1, method }
                }
                SourceKind::Image { path } => {
                    SourceMeta::Image { format: image_format(path), width: size.0, height: size.1 }
                }
                SourceKind::Color { width, height, .. } => SourceMeta::Color { width: *width, height: *height },
                SourceKind::Text { .. } => SourceMeta::Text,
                SourceKind::GameCapture { target_exe, method: mode, .. } => SourceMeta::Game {
                    exe: target_exe.clone().unwrap_or_default(),
                    width: size.0,
                    height: size.1,
                    method,
                    mode: *mode,
                },
                SourceKind::Camera { device_id, height, fps, .. } => {
                    SourceMeta::Camera { device: device_id.clone(), mode: format!("{height}p{fps}") }
                }
                SourceKind::Browser { width, height, .. } => SourceMeta::Browser { width: *width, height: *height },
            };
            views.push(SourceView {
                id: item.source,
                name: cfg.name.clone(),
                meta,
                visible: item.visible,
                locked: item.locked,
                error: match &state {
                    SourceState::Error(e) => Some(e.clone()),
                    _ => None,
                },
                waiting: state == SourceState::Waiting,
                minimized: state == SourceState::Minimized,
                item: idx,
                transform: item.transform,
                crop: item.crop,
                size,
                kind: Some(cfg.kind.clone()),
            });
        }
    }
    AppState {
        scenes,
        active_scene: c.active_scene,
        sources: views,
        audio: Vec::new(),
        stream: OutputStatus::Idle,
        stream_elapsed_secs: 0,
        record: OutputStatus::Idle,
        record_elapsed_secs: 0,
        record_paused: false,
        stats: stats.clone(),
        output: output_summary(profile, canvas),
        preview_enabled: true,
        preview,
        transition: c.transition,
        can_undo,
        can_redo,
        live_engine: true,
    }
}

/// Элементы сцены для компоновщика (снизу вверх, только видимые и с кадром).
fn draw_items(scene: &Scene, sources: &HashMap<SourceId, Box<dyn VideoSource>>) -> Vec<DrawItem> {
    let mut out = Vec::with_capacity(scene.items.len());
    for item in scene.items.iter().filter(|i| i.visible) {
        let Some(content) = sources.get(&item.source).and_then(|s| s.content()) else { continue };
        let (size, extra_rot) = effective_size(&content);
        let r = geometry::item_rect(&item.transform, &item.crop, size);
        let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let cr = &item.crop;
        let uv = [cr.left as f32 / w, cr.top as f32 / h, 1.0 - cr.right as f32 / w, 1.0 - cr.bottom as f32 / h];
        let rotation = (item.transform.rotation as u8 + extra_rot) % 4;
        let source = match content {
            Content::Texture { srv, .. } => DrawSource::Texture(srv),
            Content::Color { rgba, .. } => DrawSource::Color(rgba),
        };
        out.push(DrawItem { source, rect: [r.x, r.y, r.w, r.h], uv, rotation, opacity: 1.0 });
    }
    out
}

struct Worker {
    gpu: Arc<GpuDevice>,
    capture: CaptureContext,
    editor: SceneEditor,
    profile: Profile,
    sources: HashMap<SourceId, Box<dyn VideoSource>>,
    compositor: Option<Compositor>,
    canvas: (u32, u32),
    /// Переход: предыдущая сцена и момент начала.
    transition: Option<(usize, Instant)>,
    pending_fit: Vec<SourceId>,
    dirty_since: Option<Instant>,
    path: Option<PathBuf>,
    events: Sender<Event>,
    shared: Arc<Shared>,
    stats: Stats,
    frames_in_second: u32,
    second_start: Instant,
    seconds: u64,
    process: ProcessMonitor,
    audio: Option<AudioEngine>,
    /// Режим мониторинга по каналам (воспроизведение — позже в M2).
    monitor: Vec<MonitorMode>,
}

/// Запуск аудиодвижка по каналам профиля.
fn start_audio(profile: &Profile) -> Option<AudioEngine> {
    let channels = profile
        .audio
        .channels
        .iter()
        .map(|c| ChannelConfig {
            name: c.name.clone(),
            direction: match c.kind {
                AudioChannelKind::Desktop => Direction::Render,
                AudioChannelKind::Mic => Direction::Capture,
            },
            device: match c.kind {
                AudioChannelKind::Desktop => profile.audio.desktop_device.clone(),
                AudioChannelKind::Mic => profile.audio.mic_device.clone(),
            },
            control: ChannelControl { fader_db: c.fader_db, muted: c.muted, tracks: c.tracks },
        })
        .collect();
    match AudioEngine::start(channels) {
        Ok(a) => Some(a),
        Err(e) => {
            tracing::error!("аудио не запущено: {e}");
            None
        }
    }
}

fn audio_channels(audio: Option<&AudioEngine>, monitor: &[MonitorMode]) -> Vec<AudioChannel> {
    audio.map_or_else(Vec::new, |a| {
        a.status()
            .into_iter()
            .enumerate()
            .map(|(i, s)| AudioChannel {
                name: s.name,
                fader_db: s.control.fader_db,
                muted: s.control.muted,
                monitor: monitor.get(i).copied().unwrap_or_default(),
                peak: s.meter.peak,
                rms: s.meter.rms,
                peak_hold: s.meter.peak_hold,
                device_missing: s.device.is_none(),
                tracks: s.control.tracks,
            })
            .collect()
    })
}

impl Worker {
    fn size_of(&self, id: SourceId) -> Option<(u32, u32)> {
        self.sources.get(&id).and_then(|s| s.content()).map(|c| effective_size(&c).0)
    }

    fn create_source(&mut self, id: SourceId) {
        if let Some(cfg) = self.editor.collection.sources.get(&id) {
            self.sources.insert(id, cs_capture::create(&cfg.kind, &self.capture));
        }
    }

    fn handle(&mut self, cmd: Command) {
        tracing::debug!("движок: {cmd:?}");
        match cmd {
            Command::ApplyProfile(p) => {
                self.profile = *p;
                let canvas = canvas_size(&self.profile);
                if canvas != self.canvas {
                    self.canvas = canvas;
                    if let Some(c) = self.compositor.as_mut() {
                        let _g = self.gpu.lock();
                        if let Err(e) = c.resize(&self.gpu, canvas.0, canvas.1) {
                            tracing::error!("смена разрешения холста: {e}");
                        }
                    }
                }
                return;
            }
            Command::SetFader { channel, db } => {
                self.update_control(channel, |c| c.fader_db = db);
                return;
            }
            Command::ToggleMute(channel) => {
                self.update_control(channel, |c| c.muted = !c.muted);
                return;
            }
            Command::SetTracks { channel, tracks } => {
                self.update_control(channel, |c| c.tracks = tracks);
                return;
            }
            Command::SetMonitor { channel, mode } => {
                if let Some(m) = self.monitor.get_mut(channel) {
                    *m = mode;
                }
                return;
            }
            Command::StartStream | Command::StopStream | Command::StartRecording | Command::StopRecording => {
                // Вывод — этапы M2/M3; UI сообщает об этом сам.
                return;
            }
            _ => {}
        }
        let canvas = self.canvas;
        let sizes: HashMap<SourceId, (u32, u32)> =
            self.sources.keys().filter_map(|id| self.size_of(*id).map(|s| (*id, s))).collect();
        let fx: Effect = self.editor.apply(cmd, &|id| sizes.get(&id).copied(), canvas);
        for id in fx.create {
            self.create_source(id);
        }
        // Удалённые из коллекции источники освобождаем сразу.
        let alive = &self.editor.collection.sources;
        self.sources.retain(|id, _| alive.contains_key(id));
        if let Some(id) = fx.fit_when_ready {
            self.pending_fit.push(id);
        }
        if let Some(prev) = fx.switched_from {
            let t = self.editor.collection.transition;
            self.transition = (t.kind == cs_core::model::TransitionKind::Fade).then(|| (prev, Instant::now()));
        }
        if fx.changed {
            self.dirty_since = Some(Instant::now());
        }
    }

    fn update_control(&mut self, channel: usize, f: impl FnOnce(&mut ChannelControl)) {
        if let Some(a) = &self.audio
            && let Some(s) = a.status().get(channel)
        {
            let mut c = s.control;
            f(&mut c);
            a.set_control(channel, c);
        }
    }

    /// Новые источники крупнее холста вписываются в него, как только известен их размер.
    fn apply_pending_fit(&mut self) {
        if self.pending_fit.is_empty() {
            return;
        }
        let canvas = self.canvas;
        let ready: Vec<(SourceId, (u32, u32))> =
            self.pending_fit.iter().filter_map(|id| self.size_of(*id).filter(|s| s.0 > 1).map(|s| (*id, s))).collect();
        for (id, size) in ready {
            self.pending_fit.retain(|x| *x != id);
            if let Some(scene) = self.editor.collection.active_mut()
                && let Some(item) = scene.items.iter_mut().find(|i| i.source == id)
                && (size.0 > canvas.0 || size.1 > canvas.1)
            {
                item.transform = geometry::fit(&item.transform, &item.crop, size, canvas);
                self.dirty_since = Some(Instant::now());
            }
        }
    }

    fn tick(&mut self) {
        let gpu = self.gpu.clone();
        let _g = gpu.lock();
        for s in self.sources.values_mut() {
            s.update(&gpu);
        }
        let Some(compositor) = self.compositor.as_mut() else { return };
        let c = &self.editor.collection;
        let current = c.active().map(|s| draw_items(s, &self.sources)).unwrap_or_default();
        let mut previous = None;
        if let Some((prev, start)) = self.transition {
            let t = start.elapsed().as_secs_f32() / (c.transition.duration_ms.max(1) as f32 / 1000.0);
            if t >= 1.0 {
                self.transition = None;
            } else if let Some(scene) = c.scenes.get(prev) {
                previous = Some((draw_items(scene, &self.sources), t));
            }
        }
        if let Err(e) = compositor.compose(&gpu, &current, previous.as_ref().map(|(v, t)| (v.as_slice(), *t))) {
            tracing::error!("компоновка: {e}");
        }
        if let Some(ms) = compositor.poll_gpu_time(&gpu) {
            // Скользящее среднее: метрика «Рендер» в статистике (§11.5).
            self.stats.render_ms = if self.stats.render_ms == 0.0 { ms } else { self.stats.render_ms * 0.9 + ms * 0.1 };
        }
    }

    fn publish(&mut self) {
        let preview = self.compositor.as_ref().map(|c| PreviewInfo {
            generation: c.generation(),
            width: c.size().0,
            height: c.size().1,
        });
        if let Some(c) = &self.compositor {
            let mut p = self.shared.preview.lock();
            if p.as_ref().is_none_or(|(g, _)| *g != c.generation()) {
                *p = Some((c.generation(), c.canvas().srv.clone()));
            }
        }
        let mut state = build_state(
            &self.editor.collection,
            &self.sources,
            &self.profile,
            self.canvas,
            &self.stats,
            preview,
            self.editor.history.can_undo(),
            self.editor.history.can_redo(),
        );
        state.audio = audio_channels(self.audio.as_ref(), &self.monitor);
        *self.shared.snapshot.lock() = Arc::new(state);
    }

    fn save_now(&mut self, background: bool) {
        self.dirty_since = None;
        let Some(path) = self.path.clone() else { return };
        let collection = self.editor.collection.clone();
        let save = move || {
            if let Err(e) = config::save(&path, &collection) {
                tracing::warn!("автосохранение сцен: {e}");
            }
        };
        if background {
            let _ = std::thread::Builder::new().name("autosave".into()).spawn(save);
        } else {
            save();
        }
    }

    fn update_stats(&mut self, frame_interval: Duration) {
        self.frames_in_second += 1;
        let elapsed = self.second_start.elapsed();
        if elapsed >= Duration::from_secs(1) {
            self.stats.fps = self.frames_in_second as f32 / elapsed.as_secs_f32();
            self.frames_in_second = 0;
            self.second_start = Instant::now();
            let p = self.process.sample();
            self.stats.cpu_percent = p.cpu_percent;
            self.stats.ram_mb = p.working_set_mb;
            self.seconds += 1;
            if self.seconds.is_multiple_of(60) {
                tracing::info!(
                    "рендер: {:.1} fps, компоновка {:.2} мс GPU, пропущено {} кадров, CPU {:.1}%, RAM {:.0} МБ",
                    self.stats.fps,
                    self.stats.render_ms,
                    self.stats.dropped_render,
                    p.cpu_percent,
                    p.working_set_mb
                );
            }
        }
        self.stats.target_fps = (1.0 / frame_interval.as_secs_f64()) as f32;
        self.stats.total_frames += 1;
    }
}

/// Высокоточный таймер ожидания (Windows 10 1803+), иначе — обычный sleep.
struct FrameTimer(Option<windows::Win32::Foundation::HANDLE>);

impl FrameTimer {
    fn new() -> Self {
        use windows::Win32::System::Threading::{
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, TIMER_ALL_ACCESS,
        };
        // SAFETY: создание безымянного таймера; дескриптор закрывается в Drop.
        let h =
            unsafe { CreateWaitableTimerExW(None, None, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS.0) };
        Self(h.ok())
    }

    fn sleep(&self, d: Duration) {
        use windows::Win32::System::Threading::{INFINITE, SetWaitableTimer, WaitForSingleObject};
        if d.is_zero() {
            return;
        }
        if let Some(h) = self.0 {
            let due = -((d.as_nanos() / 100) as i64).max(1);
            // SAFETY: дескриптор таймера валиден; относительное время в единицах 100 нс.
            unsafe {
                if SetWaitableTimer(h, &due, 0, None, None, false).is_ok() {
                    WaitForSingleObject(h, INFINITE);
                    return;
                }
            }
        }
        std::thread::sleep(d);
    }
}

impl Drop for FrameTimer {
    fn drop(&mut self) {
        if let Some(h) = self.0 {
            // SAFETY: закрытие собственного дескриптора.
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(h);
            }
        }
    }
}

pub(super) fn run(cfg: EngineConfig, rx: Receiver<Command>, events: Sender<Event>, shared: Arc<Shared>) {
    use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_HIGHEST};
    cs_capture::init_thread();
    // SAFETY: приоритет текущего потока (§5.4: рендер — HIGHEST).
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
    }

    let canvas = canvas_size(&cfg.profile);
    let text = match TextRenderer::new() {
        Ok(t) => Arc::new(t),
        Err(e) => {
            tracing::error!("DirectWrite недоступен: {e}");
            let _ = events.send(Event::Error(format!("DirectWrite: {e}")));
            return;
        }
    };
    let compositor = {
        let _g = cfg.gpu.lock();
        match Compositor::new(&cfg.gpu, canvas.0, canvas.1) {
            Ok(c) => Some(c),
            Err(e) => {
                tracing::error!("компоновщик: {e}");
                let _ = events.send(Event::Error(format!("компоновщик: {e}")));
                None
            }
        }
    };
    let mut w = Worker {
        capture: CaptureContext { gpu: cfg.gpu.clone(), text },
        gpu: cfg.gpu,
        editor: SceneEditor::new(cfg.collection),
        profile: cfg.profile,
        sources: HashMap::new(),
        compositor,
        canvas,
        transition: None,
        pending_fit: Vec::new(),
        dirty_since: None,
        path: cfg.collection_path,
        events,
        shared,
        // Загрузка GPU по процессу (PDH) — позже; NaN показывается как «—».
        stats: Stats { gpu_percent: f32::NAN, ..Stats::default() },
        frames_in_second: 0,
        second_start: Instant::now(),
        seconds: 0,
        process: ProcessMonitor::new(),
        audio: None,
        monitor: Vec::new(),
    };
    w.audio = start_audio(&w.profile);
    w.monitor = vec![MonitorMode::Off; w.profile.audio.channels.len()];
    let ids: Vec<SourceId> = w.editor.collection.sources.keys().copied().collect();
    for id in ids {
        w.create_source(id);
    }
    tracing::info!("рендер-поток: холст {}×{} @ {:.2} fps", canvas.0, canvas.1, fps_of(&w.profile));

    // Прогрев: старт захвата, рендер текста и загрузка изображений до начала отсчёта темпа,
    // чтобы инициализация не считалась пропущенными кадрами.
    for _ in 0..3 {
        w.tick();
    }
    let timer = FrameTimer::new();
    let mut next = Instant::now();
    while !w.shared.shutdown.load(Ordering::Acquire) {
        let interval = Duration::from_secs_f64(1.0 / fps_of(&w.profile));
        for cmd in rx.try_iter().take(64) {
            w.handle(cmd);
        }
        w.apply_pending_fit();
        w.tick();
        w.update_stats(interval);
        w.publish();
        if w.dirty_since.is_some_and(|t| t.elapsed() >= AUTOSAVE_DELAY) {
            w.save_now(true);
        }

        // Темп холста: если тик опоздал больше чем на кадр — кадры считаются пропущенными
        // (метрика «пропущено (рендер)», §7.3), расписание выравнивается.
        next += interval;
        let now = Instant::now();
        if now > next + interval {
            let missed = ((now - next).as_secs_f64() / interval.as_secs_f64()) as u64;
            w.stats.dropped_render += missed;
            w.stats.dropped_frames = w.stats.dropped_render + w.stats.dropped_encoder + w.stats.dropped_network;
            next = now;
        } else if next > now {
            timer.sleep(next - now);
        }
    }
    if w.dirty_since.is_some() {
        w.save_now(false);
    }
    let _ = &w.events;
    tracing::info!("рендер-поток остановлен");
}
