//! Запись (§10.1): холст → NV12 → аппаратный H.264, микс дорожек → AAC, мультиплексор MKV.
//!
//! Потоки: видеоэнкодер, аудиоэнкодер, мультиплексор. Рендер-поток только конвертирует кадр
//! на GPU и отдаёт текстуру в ограниченную очередь (переполнение — «пропущено (энкодер)»).
//! Остановка асинхронная: финализацию (дренаж энкодеров, Cues, размер сегмента) выполняет
//! отдельный поток, после чего приходит событие `RecordingStopped`.

use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use cs_audio::engine::TimedBlock;
use cs_audio::mixer::{SAMPLE_RATE, TRACKS};
use cs_core::Event;
use cs_core::clock::Ticks;
use cs_encode::mf_aac::MfAacEncoder;
use cs_encode::mf_h264::MfH264Encoder;
use cs_encode::{Packet, VideoSettings, avc};
use cs_gpu::{ColorConverter, ColorParams, GpuDevice};
use cs_output::interleave::Interleaver;
use cs_output::mkv::{AudioTrack, MkvWriter, VideoTrack};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use windows::Win32::Graphics::Direct3D11::{ID3D11ShaderResourceView, ID3D11Texture2D};

/// Сообщение мультиплексору: поток (0 — видео, 1.. — дорожки аудио) и пакет.
type MuxMsg = (usize, Packet);

/// Параметры записи.
pub struct RecordingConfig {
    pub path: PathBuf,
    pub video: VideoSettings,
    pub color: ColorParams,
    pub audio_kbps: u32,
    /// Маска дорожек аудио (бит 0 — дорожка 1).
    pub tracks: u8,
    /// После остановки перепаковать MKV в MP4.
    pub remux: bool,
    /// Удалить MKV после успешного ремукса.
    pub delete_source: bool,
}

/// Пауза записи: суммарное время пауз вычитается из меток видео и аудио.
#[derive(Default)]
pub struct PauseState {
    pub paused: AtomicBool,
    /// Суммарная длительность пауз, 100 нс.
    pub total: std::sync::atomic::AtomicI64,
}

/// Счётчики записи для статистики.
#[derive(Default)]
pub struct RecordStats {
    pub bytes: AtomicU64,
    pub dropped_encoder: AtomicU64,
}

/// Идущая запись.
pub struct Recording {
    pub path: PathBuf,
    pub started: Instant,
    start_ticks: Ticks,
    converter: ColorConverter,
    video_tx: Option<Sender<(ID3D11Texture2D, i64)>>,
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
    pub stats: Arc<RecordStats>,
    pub pause: Arc<PauseState>,
    pause_started: Option<Ticks>,
    remux: bool,
    delete_source: bool,
    /// Длительность кадра, 100 нс; метки видео ставятся на эту сетку.
    frame_duration: i64,
    last_frame_index: Option<i64>,
}

/// Номер кадра на сетке частоты: дрожание времени тика (±пара мс) не попадает в метки,
/// а настоящие пропуски кадров остаются пропусками.
fn grid_index(pts: i64, frame_duration: i64) -> i64 {
    (pts + frame_duration / 2).div_euclid(frame_duration.max(1))
}

fn to_100ns(t: Ticks, start: Ticks) -> i64 {
    t.to_timebase(start, 10_000_000)
}

impl Recording {
    pub fn start(
        gpu: &Arc<GpuDevice>,
        cfg: RecordingConfig,
        audio: Option<Receiver<Arc<TimedBlock>>>,
        events: Sender<Event>,
    ) -> anyhow::Result<Self> {
        if let Some(dir) = cfg.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let converter = {
            let _g = gpu.lock();
            ColorConverter::new(gpu, cfg.video.width, cfg.video.height, cfg.color)?
        };
        let start_ticks = Ticks::now();
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(RecordStats::default());
        let pause = Arc::new(PauseState::default());
        let (video_tx, video_rx) = bounded::<(ID3D11Texture2D, i64)>(4);
        let (mux_tx, mux_rx) = unbounded::<MuxMsg>();
        let tracks: Vec<usize> = (0..TRACKS).filter(|t| cfg.tracks & (1 << t) != 0).collect();
        let tracks = if tracks.is_empty() { vec![0] } else { tracks };
        let mut threads = Vec::new();

        // Видеоэнкодер: MF-объекты создаются и живут в этом потоке.
        let device = gpu.device.clone();
        let vs = cfg.video;
        let (ready_tx, ready_rx) = bounded::<Result<String, String>>(1);
        let vtx = mux_tx.clone();
        let vstop = stop.clone();
        threads.push(std::thread::Builder::new().name("encoder-video".into()).spawn(move || {
            let mut enc = match MfH264Encoder::new(&device, &vs) {
                Ok(e) => {
                    let _ = ready_tx.send(Ok(e.name.clone()));
                    e
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
            };
            let mut out = Vec::new();
            loop {
                match video_rx.recv_timeout(std::time::Duration::from_millis(2)) {
                    Ok((tex, pts)) => enc.push(tex, pts),
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                }
                if let Err(e) = enc.pump(&mut out) {
                    tracing::error!("видеоэнкодер: {e}");
                    break;
                }
                for p in out.drain(..) {
                    let _ = vtx.send((0, p));
                }
                if vstop.load(Ordering::Acquire) && video_rx.is_empty() {
                    break;
                }
            }
            if let Err(e) = enc.drain(&mut out) {
                tracing::warn!("дренаж видеоэнкодера: {e}");
            }
            for p in out.drain(..) {
                let _ = vtx.send((0, p));
            }
        })?);
        match ready_rx.recv() {
            Ok(Ok(name)) => tracing::info!("запись: энкодер {name}"),
            Ok(Err(e)) => anyhow::bail!("видеоэнкодер: {e}"),
            Err(_) => anyhow::bail!("видеоэнкодер не запустился"),
        }

        // Аудиоэнкодеры: по одному AAC на дорожку.
        if let Some(rx) = audio {
            let atx = mux_tx.clone();
            let astop = stop.clone();
            let tr = tracks.clone();
            let kbps = cfg.audio_kbps;
            let apause = pause.clone();
            threads.push(std::thread::Builder::new().name("encoder-audio".into()).spawn(move || {
                let mut encs: Vec<MfAacEncoder> = Vec::new();
                for _ in &tr {
                    match MfAacEncoder::new(SAMPLE_RATE, kbps) {
                        Ok(e) => encs.push(e),
                        Err(e) => {
                            tracing::error!("AAC: {e}");
                            return;
                        }
                    }
                }
                let mut out = Vec::new();
                while !astop.load(Ordering::Acquire) {
                    let Ok(block) = rx.recv_timeout(std::time::Duration::from_millis(50)) else { continue };
                    if block.pts < start_ticks || apause.paused.load(Ordering::Acquire) {
                        continue;
                    }
                    let pts = to_100ns(block.pts, start_ticks) - apause.total.load(Ordering::Acquire);
                    for (k, (enc, t)) in encs.iter_mut().zip(&tr).enumerate() {
                        if let Err(e) = enc.encode(&block.block.tracks[*t], pts, &mut out) {
                            tracing::warn!("AAC: {e}");
                        }
                        for p in out.drain(..) {
                            let _ = atx.send((k + 1, p));
                        }
                    }
                }
                for (k, enc) in encs.iter_mut().enumerate() {
                    let _ = enc.drain(&mut out);
                    for p in out.drain(..) {
                        let _ = atx.send((k + 1, p));
                    }
                }
            })?);
        }
        drop(mux_tx);

        // Мультиплексор.
        let path = cfg.path.clone();
        let mstats = stats.clone();
        let (w, h) = (vs.width, vs.height);
        let n_audio = tracks.len();
        threads.push(std::thread::Builder::new().name("muxer".into()).spawn(move || {
            if let Err(e) = mux_loop(&path, mux_rx, w, h, n_audio, &mstats) {
                tracing::error!("запись {}: {e}", path.display());
                let _ = events.send(Event::Error(format!("запись: {e}")));
            }
        })?);

        Ok(Self {
            path: cfg.path,
            started: Instant::now(),
            start_ticks,
            converter,
            video_tx: Some(video_tx),
            stop,
            threads,
            stats,
            frame_duration: vs.frame_duration(),
            last_frame_index: None,
            pause,
            pause_started: None,
            remux: cfg.remux,
            delete_source: cfg.delete_source,
        })
    }

    /// Кадр холста в запись (рендер-поток, под блокировкой устройства).
    pub fn push_frame(
        &mut self,
        gpu: &GpuDevice,
        canvas: &ID3D11ShaderResourceView,
        canvas_size: (u32, u32),
        at: Ticks,
    ) {
        if self.pause.paused.load(Ordering::Acquire) {
            return;
        }
        let elapsed = to_100ns(at, self.start_ticks) - self.pause.total.load(Ordering::Acquire);
        let index = grid_index(elapsed, self.frame_duration);
        if self.last_frame_index.is_some_and(|last| index <= last) {
            return; // два тика попали в один слот сетки — второй не нужен
        }
        self.last_frame_index = Some(index);
        let pts = index * self.frame_duration;
        let tex = self.converter.convert(gpu, canvas, canvas_size);
        if let Some(tx) = &self.video_tx
            && tx.try_send((tex, pts)).is_err()
        {
            self.stats.dropped_encoder.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Пауза / продолжение. Возвращает новое состояние (`true` — на паузе).
    pub fn toggle_pause(&mut self) -> bool {
        let now = Ticks::now();
        match self.pause_started.take() {
            Some(start) => {
                self.pause.total.fetch_add(to_100ns(now, start), Ordering::AcqRel);
                self.pause.paused.store(false, Ordering::Release);
                false
            }
            None => {
                self.pause_started = Some(now);
                self.pause.paused.store(true, Ordering::Release);
                true
            }
        }
    }

    /// Время записи без пауз, с.
    pub fn elapsed_secs(&self) -> u64 {
        let paused = self.pause.total.load(Ordering::Acquire) / 10_000_000
            + self.pause_started.map_or(0, |s| (Ticks::now().seconds_since(s)) as i64);
        (self.started.elapsed().as_secs() as i64 - paused).max(0) as u64
    }

    /// Остановка с ожиданием финализации (выход из приложения).
    pub fn stop_blocking(mut self) {
        self.stop.store(true, Ordering::Release);
        self.video_tx = None;
        for t in std::mem::take(&mut self.threads) {
            let _ = t.join();
        }
        tracing::info!("запись завершена при выходе: {}", self.path.display());
        // Ремукс при выходе не выполняется (может занять время): MKV остаётся, его можно перепаковать позже.
    }

    /// Асинхронная остановка: событие `RecordingStopped` придёт после финализации файла.
    pub fn stop(mut self, events: Sender<Event>, progress: Arc<parking_lot::Mutex<Option<f32>>>) {
        self.stop.store(true, Ordering::Release);
        self.video_tx = None;
        let path = self.path.clone();
        let threads = std::mem::take(&mut self.threads);
        let (remux, delete_source) = (self.remux, self.delete_source);
        let _ = std::thread::Builder::new().name("recording-finish".into()).spawn(move || {
            for t in threads {
                let _ = t.join();
            }
            tracing::info!("запись завершена: {}", path.display());
            let _ = events.send(Event::RecordingStopped { path: path.display().to_string() });
            if remux {
                remux_file(&path, delete_source, &events, &progress);
            }
        });
    }
}

/// Авто-ремукс MKV → MP4 (§10.1): исходник удаляется только при успехе и включённой опции.
fn remux_file(
    mkv: &std::path::Path,
    delete_source: bool,
    events: &Sender<Event>,
    progress: &parking_lot::Mutex<Option<f32>>,
) {
    let mp4 = mkv.with_extension("mp4");
    *progress.lock() = Some(0.0);
    let t = Instant::now();
    let result = cs_output::mp4::remux(mkv, &mp4, |p| *progress.lock() = Some(p));
    *progress.lock() = None;
    match result {
        Ok(()) => {
            tracing::info!("ремукс за {:.1} с: {}", t.elapsed().as_secs_f32(), mp4.display());
            if delete_source && let Err(e) = std::fs::remove_file(mkv) {
                tracing::warn!("не удалось удалить {}: {e}", mkv.display());
            }
            let _ = events.send(Event::RemuxFinished { path: mp4.display().to_string() });
        }
        Err(e) => {
            tracing::error!("ремукс {}: {e}", mkv.display());
            let _ = std::fs::remove_file(mp4.with_extension("mp4.part"));
            let _ = events.send(Event::Error(format!("ремукс: {e} (MKV сохранён)")));
        }
    }
}

fn mux_loop(
    path: &std::path::Path,
    rx: Receiver<MuxMsg>,
    w: u32,
    h: u32,
    n_audio: usize,
    stats: &RecordStats,
) -> std::io::Result<()> {
    let mut il: Interleaver<Packet> = Interleaver::new(1 + n_audio, 1_000);
    let mut writer: Option<MkvWriter> = None;
    // Аудио до первого ключевого кадра видео копится, затем отбрасывается всё, что раньше него.
    let mut pending: Vec<MuxMsg> = Vec::new();
    let mut first_video_ms: Option<i64> = None;
    let mut last_ms = 0i64;
    let ms = |p: &Packet| p.pts / 10_000;
    let write =
        |w: &mut MkvWriter, stream: usize, t: i64, p: &Packet, base: i64, last: &mut i64| -> std::io::Result<()> {
            let rel = t - base;
            *last = (*last).max(rel);
            let data = if stream == 0 { avc::annexb_to_length_prefixed(&p.data) } else { p.data.clone() };
            stats.bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
            w.write((stream + 1) as u64, &data, rel, p.keyframe)
        };
    for (stream, p) in rx.iter() {
        if writer.is_none() {
            if stream == 0
                && p.keyframe
                && let Some((sps, pps)) = avc::find_parameter_sets(&p.data)
            {
                let audio: Vec<AudioTrack> = (0..n_audio)
                    .map(|i| AudioTrack {
                        sample_rate: SAMPLE_RATE,
                        channels: 2,
                        codec_private: avc::aac_asc(SAMPLE_RATE, 2).to_vec(),
                        name: format!("Track {}", i + 1),
                    })
                    .collect();
                writer = Some(MkvWriter::create(
                    path,
                    &VideoTrack { width: w, height: h, codec_private: avc::avcc(&sps, &pps) },
                    &audio,
                )?);
                first_video_ms = Some(ms(&p));
                for (s, q) in pending.drain(..) {
                    il.push(s, ms(&q), q);
                }
            } else {
                pending.push((stream, p));
                continue;
            }
        }
        il.push(stream, ms(&p), p);
        let base = first_video_ms.unwrap_or(0);
        while let Some((s, t, p)) = il.pop() {
            if t < base {
                continue;
            }
            if let Some(w) = writer.as_mut() {
                write(w, s, t, &p, base, &mut last_ms)?;
            }
        }
    }
    let base = first_video_ms.unwrap_or(0);
    if let Some(mut w) = writer {
        for (s, t, p) in il.drain() {
            if t >= base {
                write(&mut w, s, t, &p, base, &mut last_ms)?;
            }
        }
        w.finish(last_ms)?;
    }
    Ok(())
}

/// Имя файла записи по шаблону `%Y-%m-%d %H-%M-%S` (§10.1).
pub fn file_name(ext: &str) -> String {
    use windows::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: чтение локального времени.
    let t = unsafe { GetLocalTime() };
    format!("{:04}-{:02}-{:02} {:02}-{:02}-{:02}.{ext}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

#[cfg(test)]
mod tests {
    use super::grid_index;

    #[test]
    fn jitter_snaps_to_frame_grid() {
        let fd = 166_667; // 60 fps
        assert_eq!(grid_index(0, fd), 0);
        assert_eq!(grid_index(166_667 + 20_000, fd), 1); // +2 мс дрожания
        assert_eq!(grid_index(2 * 166_667 - 30_000, fd), 2);
        assert_eq!(grid_index(5 * 166_667, fd), 5);
    }
}
