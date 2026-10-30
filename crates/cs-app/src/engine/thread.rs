//! Рендер-поток (§5.4): тик по высокоточному таймеру с частотой холста, команды, источники,
//! компоновка, метрики, публикация снимка, автосохранение коллекции через 1 с после изменения.

use super::recording::{self, Recording, RecordingConfig};
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
const WARN_FREE_MB: u64 = 5 * 1024;
const STOP_FREE_MB: u64 = 500;

/// Свободное место на диске папки (ближайший существующий родитель), МБ.
fn free_mb(dir: &std::path::Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut p = dir.to_path_buf();
    while !p.exists() {
        p = p.parent()?.to_path_buf();
    }
    let wide: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free = 0u64;
    // SAFETY: нуль-терминированная строка пути; выходные указатели — локальные.
    unsafe { GetDiskFreeSpaceExW(windows::core::PCWSTR(wide.as_ptr()), Some(&mut free), None, None).ok()? };
    Some(free / (1024 * 1024))
}

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
        remux_progress: None,
    }
}

/// Задержка «захват кадра → тик компоновщика» по самому свежему кадру видимого источника
/// захвата (WGC знает время композиции кадра), если кадр получен не позже 25 мс назад (§5.2 п.4).
fn frame_capture_lag(
    sources: &HashMap<SourceId, Box<dyn VideoSource>>,
    c: &SceneCollection,
    now: cs_core::clock::Ticks,
) -> Option<f64> {
    let max_age = cs_core::clock::Ticks::frequency() / 40;
    c.active()
        .into_iter()
        .flat_map(|s| s.items.iter().filter(|i| i.visible))
        .filter_map(|i| sources.get(&i.source).and_then(|s| s.frame_time()))
        .filter(|t| t.0 <= now.0 && now.0 - t.0 <= max_age)
        .max()
        .map(|t| (now.0 - t.0) as f64)
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
    /// Сглаженная задержка «захват кадра → тик», тики QPC.
    video_lag: f64,
    process: ProcessMonitor,
    audio: Option<AudioEngine>,
    recording: Option<Recording>,
    recording_starting: Option<crossbeam_channel::Receiver<anyhow::Result<Recording>>>,
    cancel_start: bool,
    recordings_dir: PathBuf,
    remux_progress: Arc<parking_lot::Mutex<Option<f32>>>,
    disk_checked: Option<Instant>,
    disk_warned: bool,
    /// Режим мониторинга по каналам (воспроизведение — позже в M2).
    monitor: Vec<MonitorMode>,
    /// Каналы работающего аудиодвижка (тип и приложение) — по ним переносятся настройки
    /// при перезапуске движка.
    audio_keys: Vec<AudioKey>,
    /// Состав каналов или устройства изменились во время записи: перезапуск после остановки.
    audio_restart_pending: bool,
}

/// Канал движка для сопоставления при перезапуске.
type AudioKey = (AudioChannelKind, Option<String>);

/// Всё, что требует перезапуска аудиодвижка: устройства и состав каналов.
fn audio_topology(p: &Profile) -> (Option<String>, Option<String>, Vec<(AudioKey, String)>) {
    let a = &p.audio;
    let channels = a.channels.iter().map(|c| ((c.kind, c.app.clone()), c.name.clone())).collect();
    (a.desktop_device.clone(), a.mic_device.clone(), channels)
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
                AudioChannelKind::App => Direction::App,
            },
            device: match c.kind {
                AudioChannelKind::Desktop => profile.audio.desktop_device.clone(),
                AudioChannelKind::Mic => profile.audio.mic_device.clone(),
                AudioChannelKind::App => c.app.clone(),
            },
            control: ChannelControl {
                fader_db: c.fader_db,
                muted: c.muted,
                tracks: c.tracks,
                sync_offset_ms: c.sync_offset_ms,
            },
            filters: c.filters.clone(),
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
                sync_offset_ms: s.control.sync_offset_ms,
                filters: s.filters,
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
                let topology_changed = audio_topology(&self.profile) != audio_topology(&p);
                self.profile = *p;
                if topology_changed {
                    self.request_audio_restart();
                }
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
            Command::SetFilters { channel, filters } => {
                if let Some(a) = &self.audio {
                    a.set_filters(channel, filters);
                }
                return;
            }
            Command::SetSyncOffset { channel, ms } => {
                self.update_control(channel, |c| c.sync_offset_ms = ms.clamp(-950, 2000));
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
            Command::SetRecordingsDir(dir) => {
                self.recordings_dir = dir;
                return;
            }
            Command::StartRecording => {
                self.start_recording();
                return;
            }
            Command::StopRecording if self.recording_starting.is_some() => {
                self.cancel_start = true;
                return;
            }
            Command::StopRecording => {
                tracing::info!("запись: пропущено рендером {} кадров (за сеанс)", self.stats.dropped_render);
                if let Some(r) = self.recording.take() {
                    r.stop(self.events.clone(), self.remux_progress.clone());
                }
                return;
            }
            Command::TogglePauseRecording => {
                if let Some(r) = self.recording.as_mut() {
                    let paused = r.toggle_pause();
                    tracing::info!("запись: {}", if paused { "пауза" } else { "продолжена" });
                }
                return;
            }
            Command::StartStream | Command::StopStream => {
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

    fn start_recording(&mut self) {
        if self.recording.is_some() || self.recording_starting.is_some() {
            return;
        }
        use cs_core::config::profile::{
            ColorRange, ColorSpace, EncoderKind, RateControl as Rc, RecordFormat, ScaleFilter,
        };
        use cs_encode::{EncoderOptions, H264Profile};
        if let Some(free) = free_mb(&self.recordings_dir)
            && free < STOP_FREE_MB
        {
            let _ = self.events.send(Event::DiskSpaceStopped);
            return;
        }
        self.disk_warned = false;
        let p = &self.profile;
        let (ow, oh) = p.video.output.unwrap_or(self.canvas);
        let video = cs_encode::VideoSettings {
            width: ow & !1,
            height: oh & !1,
            fps_num: p.video.fps.0,
            fps_den: p.video.fps.1.max(1),
            bitrate_kbps: p.output.bitrate_kbps,
            keyframe_interval_s: p.output.keyframe_interval_s,
            rate_control: match p.output.rate_control {
                Rc::Cbr => cs_encode::RateControl::Cbr,
                Rc::Vbr => cs_encode::RateControl::Vbr,
                Rc::Cqp => cs_encode::RateControl::Cqp(p.output.cq_level),
            },
        };
        let cfg = RecordingConfig {
            path: self.recordings_dir.join(recording::file_name(
                if p.output.record_format == RecordFormat::FragmentedMp4 { "mp4" } else { "mkv" },
            )),
            video,
            color: cs_gpu::ColorParams {
                bt601: p.video.color_space == ColorSpace::Bt601,
                full_range: p.video.color_range == ColorRange::Full,
            },
            scale: match p.video.scale_filter {
                ScaleFilter::Bilinear => cs_gpu::ScaleKernel::Bilinear,
                ScaleFilter::Bicubic => cs_gpu::ScaleKernel::Bicubic,
                ScaleFilter::Lanczos => cs_gpu::ScaleKernel::Lanczos,
            },
            encoder: match p.output.encoder {
                EncoderKind::Auto => cs_encode::EncoderChoice::Auto,
                EncoderKind::Nvenc => cs_encode::EncoderChoice::Nvenc,
                EncoderKind::Amf => cs_encode::EncoderChoice::Amf,
                EncoderKind::Qsv => cs_encode::EncoderChoice::Qsv,
                EncoderKind::MediaFoundation => cs_encode::EncoderChoice::MediaFoundation,
                EncoderKind::Software => cs_encode::EncoderChoice::Software,
            },
            // Запись — тюнинг «качество».
            nvenc: EncoderOptions {
                preset: p.output.preset,
                profile: match p.output.h264_profile.as_str() {
                    "baseline" => H264Profile::Baseline,
                    "main" => H264Profile::Main,
                    _ => H264Profile::High,
                },
                low_latency: false,
                lookahead: p.output.lookahead,
                b_frames: p.output.b_frames,
            },
            audio_kbps: p.output.audio_bitrate_kbps,
            tracks: p.output.record_tracks,
            remux: matches!(p.output.record_format, RecordFormat::MkvRemuxMp4 | RecordFormat::Mp4),
            // «MP4» — то же, что MKV с ремуксом, но без исходника.
            delete_source: p.output.record_format != RecordFormat::MkvRemuxMp4 || p.output.delete_mkv_after_remux,
            // Фрагментированный MP4 пишется сразу и не перепаковывается (безопасен при сбое).
            fragmented: p.output.record_format == RecordFormat::FragmentedMp4,
        };
        let audio = self.audio.as_ref().map(|a| a.subscribe());
        // Создание аппаратного энкодера занимает ~80 мс: выполняется вне рендер-потока, чтобы
        // не пропускать кадры сцены (состояние «Запуск…» до готовности).
        let (tx, rx) = crossbeam_channel::bounded(1);
        let (gpu, events) = (self.gpu.clone(), self.events.clone());
        let spawned = std::thread::Builder::new().name("recording-start".into()).spawn(move || {
            let _ = tx.send(Recording::start(&gpu, cfg, audio, events));
        });
        match spawned {
            Ok(_) => {
                self.recording_starting = Some(rx);
                self.cancel_start = false;
            }
            Err(e) => {
                let _ = self.events.send(Event::Error(format!("запись не начата: {e}")));
            }
        }
    }

    /// Готовность запускаемой записи (опрос каждый тик).
    /// Перезапуск аудиодвижка после смены устройств или состава каналов. Во время записи
    /// откладывается до её остановки: иначе оборвалась бы звуковая дорожка.
    fn request_audio_restart(&mut self) {
        if self.recording.is_some() || self.recording_starting.is_some() {
            tracing::info!("аудио: изменения каналов применятся после остановки записи");
            self.audio_restart_pending = true;
        } else {
            self.restart_audio();
        }
    }

    fn apply_pending_audio_restart(&mut self) {
        if self.audio_restart_pending && self.recording.is_none() && self.recording_starting.is_none() {
            self.audio_restart_pending = false;
            self.restart_audio();
        }
    }

    /// Новый аудиодвижок по профилю; у сохранившихся каналов — текущие фейдер, mute, дорожки,
    /// смещение и фильтры (в профиле диалога настроек они могли устареть).
    fn restart_audio(&mut self) {
        if let Some(old) = self.audio.take() {
            let status = old.status();
            for c in &mut self.profile.audio.channels {
                let key = (c.kind, c.app.clone());
                if let Some(s) = self.audio_keys.iter().position(|k| *k == key).and_then(|i| status.get(i)) {
                    c.fader_db = s.control.fader_db;
                    c.muted = s.control.muted;
                    c.tracks = s.control.tracks;
                    c.sync_offset_ms = s.control.sync_offset_ms;
                    c.filters = s.filters.clone();
                }
            }
        }
        let old_keys = std::mem::take(&mut self.audio_keys);
        let old_monitor = std::mem::take(&mut self.monitor);
        self.audio = start_audio(&self.profile);
        self.audio_keys = self.profile.audio.channels.iter().map(|c| (c.kind, c.app.clone())).collect();
        self.monitor = self
            .audio_keys
            .iter()
            .map(|k| old_keys.iter().position(|o| o == k).and_then(|i| old_monitor.get(i).copied()).unwrap_or_default())
            .collect();
        tracing::info!("аудио: движок перезапущен, каналов {}", self.audio_keys.len());
    }

    fn poll_recording_start(&mut self) {
        let Some(rx) = &self.recording_starting else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.recording_starting = None;
        match result {
            Ok(r) if self.cancel_start => r.stop(self.events.clone(), self.remux_progress.clone()),
            Ok(r) => {
                tracing::info!("запись: {}", r.path.display());
                self.recording = Some(r);
                let _ = self.events.send(Event::RecordingStarted);
            }
            Err(e) => {
                tracing::error!("запись не начата: {e:#}");
                let _ = self.events.send(Event::Error(format!("запись не начата: {e:#}")));
            }
        }
    }

    /// Свободное место во время записи (раз в 5 с): < 5 ГБ — предупреждение, < 500 МБ — остановка.
    fn check_disk(&mut self) {
        if self.recording.is_none() || self.disk_checked.is_some_and(|t| t.elapsed() < Duration::from_secs(5)) {
            return;
        }
        self.disk_checked = Some(Instant::now());
        let Some(free) = free_mb(&self.recordings_dir) else { return };
        if free < STOP_FREE_MB {
            tracing::warn!("мало места ({free} МБ): запись остановлена");
            if let Some(r) = self.recording.take() {
                r.stop(self.events.clone(), self.remux_progress.clone());
            }
            let _ = self.events.send(Event::DiskSpaceStopped);
        } else if free < WARN_FREE_MB && !self.disk_warned {
            self.disk_warned = true;
            let _ = self.events.send(Event::DiskSpaceLow { free_mb: free });
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

    /// `scheduled` — плановое время тика по расписанию холста: опоздание пробуждения потока
    /// и ожидание устройства не меняют номер кадра в записи.
    fn tick(&mut self, scheduled: cs_core::clock::Ticks) {
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
        let now = cs_core::clock::Ticks::now();
        if let Some(lag) = frame_capture_lag(&self.sources, &self.editor.collection, now) {
            // Средняя задержка захвата: меряется и вне записи, чтобы к старту записи она устоялась.
            self.video_lag = if self.video_lag == 0.0 { lag } else { self.video_lag * 0.95 + lag * 0.05 };
        }
        if let Some(rec) = self.recording.as_mut() {
            let canvas = compositor.canvas();
            // Задержка захвата в метке меняется ступенькой, только когда живое значение ушло больше
            // чем на полкадра. Живое значение медленно ползёт в пределах кадра (таймер рендера и
            // развёртка монитора идут с чуть разной частотой) и дрожит: прямо в метке оно сталкивало
            // бы кадры между слотами сетки, а замороженное за час уводит синхронизацию на кадр.
            let ms = 1000.0 / cs_core::clock::Ticks::frequency() as f64;
            let half_frame = cs_core::clock::Ticks::frequency() as f64 / fps_of(&self.profile) / 2.0;
            let lag = match rec.capture_lag {
                Some(lag) if (self.video_lag - lag as f64).abs() <= half_frame => lag,
                prev => {
                    match prev {
                        None => tracing::info!("запись: задержка захвата видео {:.1} мс", self.video_lag * ms),
                        Some(old) => tracing::info!(
                            "запись: задержка захвата видео {:.1} → {:.1} мс",
                            old as f64 * ms,
                            self.video_lag * ms
                        ),
                    }
                    rec.capture_lag = Some(self.video_lag as i64);
                    self.video_lag as i64
                }
            };
            let at = cs_core::clock::Ticks(scheduled.0 - lag);
            rec.push_frame(&gpu, &canvas.srv, (canvas.width, canvas.height), at);
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
        state.remux_progress = *self.remux_progress.lock();
        if self.recording_starting.is_some() {
            state.record = OutputStatus::Starting;
        }
        if let Some(r) = &self.recording {
            state.record = OutputStatus::Live;
            state.record_paused = r.pause.paused.load(std::sync::atomic::Ordering::Acquire);
            state.record_elapsed_secs = r.elapsed_secs();
            state.stats.dropped_encoder = r.stats.dropped_encoder.load(std::sync::atomic::Ordering::Relaxed);
        }
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
                    "рендер: {:.1} fps, компоновка {:.2} мс GPU, пропущено {} кадров, захват {:.1} мс, CPU {:.1}%, RAM {:.0} МБ",
                    self.stats.fps,
                    self.stats.render_ms,
                    self.stats.dropped_render,
                    self.video_lag * 1000.0 / cs_core::clock::Ticks::frequency() as f64,
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
        video_lag: 0.0,
        process: ProcessMonitor::new(),
        audio: None,
        recording: None,
        recording_starting: None,
        cancel_start: false,
        remux_progress: Arc::new(parking_lot::Mutex::new(None)),
        disk_checked: None,
        disk_warned: false,
        recordings_dir: std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("Videos")
            .join("Capture Studio"),
        monitor: Vec::new(),
        audio_keys: Vec::new(),
        audio_restart_pending: false,
    };
    w.audio = start_audio(&w.profile);
    w.monitor = vec![MonitorMode::Off; w.profile.audio.channels.len()];
    w.audio_keys = w.profile.audio.channels.iter().map(|c| (c.kind, c.app.clone())).collect();
    let ids: Vec<SourceId> = w.editor.collection.sources.keys().copied().collect();
    for id in ids {
        w.create_source(id);
    }
    tracing::info!("рендер-поток: холст {}×{} @ {:.2} fps", canvas.0, canvas.1, fps_of(&w.profile));

    // Прогрев: старт захвата, рендер текста и загрузка изображений до начала отсчёта темпа,
    // чтобы инициализация не считалась пропущенными кадрами.
    for _ in 0..3 {
        w.tick(cs_core::clock::Ticks::now());
    }
    let timer = FrameTimer::new();
    let loop_start = Instant::now();
    let mut next = Instant::now();
    let heartbeat = Arc::new(super::watchdog::Heartbeat::default());
    let watchdog = super::watchdog::spawn(heartbeat.clone()).ok();
    use super::watchdog::Stage;
    while !w.shared.shutdown.load(Ordering::Acquire) {
        let interval = Duration::from_secs_f64(1.0 / fps_of(&w.profile));
        heartbeat.mark(Stage::Commands);
        let t0 = Instant::now();
        for cmd in rx.try_iter().take(64) {
            w.handle(cmd);
        }
        w.apply_pending_fit();
        w.poll_recording_start();
        w.apply_pending_audio_restart();
        let t1 = Instant::now();
        // Плановое время этого тика: сейчас минус опоздание относительно расписания. Иначе
        // опоздание больше полукадра сдвигает кадр в следующий слот сетки записи, а его слот
        // пустеет (такие пропуски не видны в счётчике).
        heartbeat.mark(Stage::Tick);
        let late = t1.saturating_duration_since(next).as_secs_f64();
        let now_ticks = cs_core::clock::Ticks::now();
        w.tick(cs_core::clock::Ticks(now_ticks.0 - (late * cs_core::clock::Ticks::frequency() as f64) as i64));
        let t2 = Instant::now();
        heartbeat.mark(Stage::Publish);
        w.update_stats(interval);
        w.publish();
        let t3 = Instant::now();
        heartbeat.mark(Stage::Housekeeping);
        w.check_disk();
        if w.dirty_since.is_some_and(|t| t.elapsed() >= AUTOSAVE_DELAY) {
            w.save_now(true);
        }

        // Темп холста: если тик опоздал больше чем на кадр — кадры считаются пропущенными
        // (метрика «пропущено (рендер)», §7.3), расписание выравнивается.
        heartbeat.mark(Stage::Sleep);
        next += interval;
        let now = Instant::now();
        if now > next + interval {
            let missed = ((now - next).as_secs_f64() / interval.as_secs_f64()) as u64;
            w.stats.dropped_render += missed;
            // Диагностика пропусков: где ушло время (команды / компоновка / снимок / ожидание).
            tracing::info!(
                "рендер: пропущено {missed} кадр(ов) на {:.2} с; команды {:.1} мс, тик {:.1} мс, снимок {:.1} мс, запись {}",
                (now - loop_start).as_secs_f64(),
                (t1 - t0).as_secs_f64() * 1000.0,
                (t2 - t1).as_secs_f64() * 1000.0,
                (t3 - t2).as_secs_f64() * 1000.0,
                if w.recording.is_some() { "идёт" } else { "нет" }
            );
            w.stats.dropped_frames = w.stats.dropped_render + w.stats.dropped_encoder + w.stats.dropped_network;
            next = now;
        } else if next > now {
            timer.sleep(next - now);
        }
    }
    heartbeat.stop();
    if let Some(t) = watchdog {
        let _ = t.join();
    }
    if let Some(r) = w.recording.take() {
        // Выход во время записи: файл финализируется до завершения процесса (§15).
        r.stop_blocking();
    }
    if w.dirty_since.is_some() {
        w.save_now(false);
    }
    let _ = &w.events;
    tracing::info!("рендер-поток остановлен");
}
