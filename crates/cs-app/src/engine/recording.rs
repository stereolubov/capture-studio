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
                    if block.pts < start_ticks {
                        continue;
                    }
                    let pts = to_100ns(block.pts, start_ticks);
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
        let tex = self.converter.convert(gpu, canvas, canvas_size);
        let pts = to_100ns(at, self.start_ticks);
        if let Some(tx) = &self.video_tx
            && tx.try_send((tex, pts)).is_err()
        {
            self.stats.dropped_encoder.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Остановка с ожиданием финализации (выход из приложения).
    pub fn stop_blocking(mut self) {
        self.stop.store(true, Ordering::Release);
        self.video_tx = None;
        for t in std::mem::take(&mut self.threads) {
            let _ = t.join();
        }
        tracing::info!("запись завершена при выходе: {}", self.path.display());
    }

    /// Асинхронная остановка: событие `RecordingStopped` придёт после финализации файла.
    pub fn stop(mut self, events: Sender<Event>) {
        self.stop.store(true, Ordering::Release);
        self.video_tx = None;
        let path = self.path.clone();
        let threads = std::mem::take(&mut self.threads);
        let _ = std::thread::Builder::new().name("recording-finish".into()).spawn(move || {
            for t in threads {
                let _ = t.join();
            }
            tracing::info!("запись завершена: {}", path.display());
            let _ = events.send(Event::RecordingStopped { path: path.display().to_string() });
        });
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
