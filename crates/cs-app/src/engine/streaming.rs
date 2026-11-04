//! Network output consumes encoded packets on a separate thread, never blocks rendering.
use super::recording::{MuxMsg, RecordStats};
use crossbeam_channel::{Receiver, Sender};
use cs_core::{Event, state::OutputStatus};
use cs_encode::avc;
use cs_output::{
    ReconnectPolicy, flv,
    rtmp::client::{Endpoint, Publisher},
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub struct Config {
    pub server: String,
    pub key: String,
    pub reconnect: bool,
    pub dynamic_bitrate: bool,
}

pub fn run(
    cfg: Config,
    rx: Receiver<MuxMsg>,
    stats: Arc<RecordStats>,
    stop: Arc<AtomicBool>,
    events: Sender<Event>,
    video: cs_encode::VideoSettings,
    audio_kbps: u32,
) {
    let result = run_inner(&cfg, &rx, &stats, &stop, &events, video, audio_kbps);
    if let Err(e) = result
        && set_status(&stats, &stop, OutputStatus::Error(e.to_string()))
    {
        let _ = events.send(Event::Error(format!("трансляция: {e}")));
    }
    stop.store(true, Ordering::Release);
}

fn run_inner(
    cfg: &Config,
    rx: &Receiver<MuxMsg>,
    stats: &RecordStats,
    stop: &AtomicBool,
    events: &Sender<Event>,
    video: cs_encode::VideoSettings,
    audio_kbps: u32,
) -> io::Result<()> {
    let endpoint = Endpoint::parse(&cfg.server)?;
    let policy = ReconnectPolicy::default();
    let mut attempt = 0;
    let mut ever_live = false;
    let mut codec = None;
    let mut bitrate = cs_output::bitrate::AdaptiveBitrate::new(video.bitrate_kbps);
    while !stop.load(Ordering::Acquire) {
        let result: io::Result<()> = (|| {
            let mut publisher = Publisher::connect(&endpoint, &cfg.key, stop)?;
            use cs_output::rtmp::amf::{self, Value, object, string};
            let metadata = amf::encode(&[
                string("@setDataFrame"),
                string("onMetaData"),
                object(&[
                    ("width", Value::Number(video.width as f64)),
                    ("height", Value::Number(video.height as f64)),
                    ("framerate", Value::Number(video.fps_num as f64 / video.fps_den.max(1) as f64)),
                    ("videocodecid", Value::Number(7.)),
                    ("videodatarate", Value::Number(video.bitrate_kbps as f64)),
                    ("audiocodecid", Value::Number(10.)),
                    ("audiodatarate", Value::Number(audio_kbps as f64)),
                    ("audiosamplerate", Value::Number(48000.)),
                    ("stereo", Value::Bool(true)),
                ]),
            ])?;
            publisher.media(18, 0, &metadata)?;
            // Data accumulated during a connection attempt is stale. Resume on a fresh IDR.
            for (s, p) in rx.try_iter().take(if ever_live || attempt > 0 { 256 } else { 0 }) {
                if s == 0 {
                    if let Some((sps, pps)) = avc::find_parameter_sets(&p.data) {
                        codec = Some(avc::avcc(&sps, &pps));
                    }
                    stats.dropped_network.fetch_add(1, Ordering::Relaxed);
                }
            }
            let mut base = None;
            let mut recovery = KeyframeRecovery::default();
            let mut headers_sent = false;
            let mut last_poll = Instant::now();
            let mut sample = (Instant::now(), publisher.sent_bytes);
            while !stop.load(Ordering::Acquire) {
                recovery.request(&stats.request_keyframe, Instant::now());
                if last_poll.elapsed() >= Duration::from_millis(100) {
                    publisher.poll(stop)?;
                    last_poll = Instant::now();
                    if let Some(rtt) = publisher.rtt {
                        stats.rtt_us.store(rtt.as_micros() as u64, Ordering::Relaxed);
                    }
                }
                let (stream, packet) = match rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(packet) => packet,
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                };
                if stream == 0
                    && let Some((sps, pps)) = avc::find_parameter_sets(&packet.data)
                {
                    codec = Some(avc::avcc(&sps, &pps));
                }
                if stats.queue_gap.swap(false, Ordering::AcqRel) {
                    adapt(cfg, stats, &mut bitrate, true, false);
                    recovery.restart();
                    if stream == 0 {
                        stats.dropped_network.fetch_add(1, Ordering::Relaxed);
                    }
                    // Discard everything queued before the discontinuity. Only a future IDR
                    // can establish an unbroken decode chain after the dropped packet.
                    for (s, p) in rx.try_iter().take(256) {
                        if s == 0 {
                            if let Some((sps, pps)) = avc::find_parameter_sets(&p.data) {
                                codec = Some(avc::avcc(&sps, &pps));
                            }
                            stats.dropped_network.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    continue;
                }
                let latest = stats.latest_dts.load(Ordering::Relaxed);
                let lag = latest.saturating_sub(packet.dts);
                if lag > 20_000_000 {
                    adapt(cfg, stats, &mut bitrate, true, false);
                }
                if latest.saturating_sub(packet.dts) > 20_000_000 && !recovery.waiting {
                    recovery.restart();
                }
                let idr = stream == 0 && avc::is_idr(&packet.data);
                if recovery.waiting {
                    if stream != 0 || !idr || codec.is_none() || latest.saturating_sub(packet.dts) > 20_000_000 {
                        if stream == 0 {
                            stats.dropped_network.fetch_add(1, Ordering::Relaxed);
                        }
                        continue;
                    }
                    recovery.waiting = false;
                    // All tracks share the same decoding epoch; preserve PTS-DTS for B-frames.
                    base.get_or_insert(packet.dts);
                    let timestamp = ((packet.dts - base.unwrap_or(0)).max(0) / 10_000) as u32;
                    publisher.media(
                        flv::VIDEO,
                        timestamp,
                        &flv::avc_sequence_header(codec.as_deref().expect("codec checked"))?,
                    )?;
                    if !headers_sent {
                        publisher.media(flv::AUDIO, timestamp, &flv::aac(&avc::aac_asc(48_000, 2), true)?)?;
                        headers_sent = true;
                    }
                }
                let epoch = base.expect("keyframe established epoch");
                if packet.dts < epoch {
                    continue;
                }
                let time = ((packet.dts - epoch) / 10_000) as u32;
                let body = if stream == 0 {
                    flv::avc_frame(
                        &avc::annexb_to_length_prefixed(&packet.data),
                        idr,
                        (packet.pts - packet.dts) / 10_000,
                    )?
                } else {
                    flv::aac(&packet.data, false)?
                };
                let sending = Instant::now();
                publisher.media(if stream == 0 { flv::VIDEO } else { flv::AUDIO }, time, &body)?;
                let send_time = sending.elapsed();
                adapt(
                    cfg,
                    stats,
                    &mut bitrate,
                    lag > 20_000_000 || send_time >= Duration::from_millis(250),
                    lag < 5_000_000 && send_time < Duration::from_millis(100),
                );
                stats.bytes.fetch_add(body.len() as u64, Ordering::Relaxed);
                if stream == 0 && set_status(stats, stop, OutputStatus::Live) {
                    if !ever_live {
                        *stats.live_since.lock() = Some(Instant::now());
                        let _ = events.send(Event::StreamStarted);
                        ever_live = true;
                    }
                    attempt = 0;
                }
                if sample.0.elapsed() >= Duration::from_secs(1) {
                    let kbps = (publisher.sent_bytes - sample.1) as f64 * 8. / sample.0.elapsed().as_secs_f64() / 1000.;
                    stats.bitrate_kbps.store(kbps as u64, Ordering::Relaxed);
                    sample = (Instant::now(), publisher.sent_bytes);
                }
            }
            let _ = publisher.close();
            Ok(())
        })();
        match result {
            Ok(()) => return Ok(()),
            Err(_) if stop.load(Ordering::Acquire) => return Ok(()),
            Err(e)
                if !cfg.reconnect || e.kind() == io::ErrorKind::PermissionDenied || attempt >= policy.max_attempts =>
            {
                return Err(e);
            }
            Err(_) => {
                // Time disconnected is not evidence of healthy bandwidth.
                adapt(cfg, stats, &mut bitrate, false, false);
                attempt += 1;
                stats.bitrate_kbps.store(0, Ordering::Relaxed);
                if !set_status(stats, stop, OutputStatus::Reconnecting { attempt, max: policy.max_attempts }) {
                    return Ok(());
                }
                let _ = events.send(Event::StreamReconnecting { attempt, max: policy.max_attempts });
                let deadline = Instant::now() + Duration::from_secs(u64::from(policy.delay_secs(attempt)));
                while Instant::now() < deadline && !stop.load(Ordering::Acquire) {
                    // Drop rather than buffer media across reconnections.
                    while let Ok((s, p)) = rx.try_recv() {
                        if s == 0 {
                            if let Some((sps, pps)) = avc::find_parameter_sets(&p.data) {
                                codec = Some(avc::avcc(&sps, &pps));
                            }
                            stats.dropped_network.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
    Ok(())
}

fn adapt(
    cfg: &Config,
    stats: &RecordStats,
    policy: &mut cs_output::bitrate::AdaptiveBitrate,
    congested: bool,
    healthy: bool,
) {
    if cfg.dynamic_bitrate
        && !stats.dynamic_bitrate_failed.load(Ordering::Acquire)
        && let Some(kbps) = policy.observe(congested, healthy, Instant::now())
    {
        stats.requested_bitrate_kbps.store(u64::from(kbps), Ordering::Release);
    }
}

/// Late worker results cannot revive Stop.
fn set_status(stats: &RecordStats, stop: &AtomicBool, next: OutputStatus) -> bool {
    let mut status = stats.stream_status.lock();
    if stop.load(Ordering::Acquire) || *status == OutputStatus::Stopping || *status == next {
        return false;
    }
    *status = next;
    true
}

/// Repeated losses coalesce into one request. Retry slowly while waiting, never once per packet.
struct KeyframeRecovery {
    waiting: bool,
    last_request: Option<Instant>,
}
impl Default for KeyframeRecovery {
    fn default() -> Self {
        Self { waiting: true, last_request: None }
    }
}
impl KeyframeRecovery {
    fn restart(&mut self) {
        self.waiting = true;
    }
    fn request(&mut self, flag: &AtomicBool, now: Instant) {
        if self.waiting && self.last_request.is_none_or(|last| now.duration_since(last) >= Duration::from_secs(1)) {
            flag.store(true, Ordering::Release);
            self.last_request = Some(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_requests_idr_without_flooding_encoder() {
        let mut recovery = KeyframeRecovery::default();
        let flag = AtomicBool::new(false);
        let now = Instant::now();
        recovery.request(&flag, now);
        assert!(flag.swap(false, Ordering::AcqRel));
        for ms in 1..1000 {
            recovery.restart();
            recovery.request(&flag, now + Duration::from_millis(ms));
        }
        assert!(!flag.load(Ordering::Acquire));
        recovery.request(&flag, now + Duration::from_secs(1));
        assert!(flag.swap(false, Ordering::AcqRel));
        recovery.waiting = false;
        recovery.request(&flag, now + Duration::from_secs(10));
        assert!(!flag.load(Ordering::Acquire));
        recovery.restart();
        recovery.request(&flag, now + Duration::from_secs(11));
        assert!(flag.load(Ordering::Acquire));
    }
    #[test]
    fn late_connection_results_cannot_revive_cancelled_stream() {
        let stats = RecordStats::default();
        let stop = AtomicBool::new(false);
        assert!(set_status(&stats, &stop, OutputStatus::Starting));
        stop.store(true, Ordering::Release);
        *stats.stream_status.lock() = OutputStatus::Stopping;
        for late in [
            OutputStatus::Live,
            OutputStatus::Reconnecting { attempt: 1, max: 25 },
            OutputStatus::Error("late error".into()),
        ] {
            assert!(!set_status(&stats, &stop, late));
            assert_eq!(*stats.stream_status.lock(), OutputStatus::Stopping);
        }
    }
}

#[cfg(test)]
#[path = "streaming_network_tests.rs"]
mod network_tests;
