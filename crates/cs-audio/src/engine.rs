//! Аудиодвижок: потоки захвата + поток микшера (MMCSS «Pro Audio», блоки 10 мс по мастер-часам).
//!
//! Метка времени блока = момент старта микшера + `index × 10 мс` (тики QPC, §5.2 п.4).
//! Подписчики (энкодеры) получают каждый блок; аудио не отбрасывается никогда (§5.2 п.5).

use crate::mixer::{BLOCK_FRAMES, ChannelControl, Meter, MixBlock, Mixer, SAMPLE_RATE};
use crate::wasapi::{self, Direction, SourceShared};
use crossbeam_channel::{Receiver, Sender, unbounded};
use cs_core::clock::Ticks;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Описание канала микшера.
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub name: String,
    pub direction: Direction,
    /// `None` — устройство по умолчанию (со следованием за его сменой).
    pub device: Option<String>,
    pub control: ChannelControl,
}

/// Блок микса с меткой времени мастер-часов.
#[derive(Debug, Clone)]
pub struct TimedBlock {
    pub pts: Ticks,
    pub block: MixBlock,
}

/// Состояние канала для UI.
#[derive(Debug, Clone)]
pub struct ChannelStatus {
    pub name: String,
    pub control: ChannelControl,
    pub meter: Meter,
    /// Имя устройства; `None` — устройство недоступно.
    pub device: Option<String>,
}

struct Shared {
    controls: Mutex<Vec<ChannelControl>>,
    meters: Mutex<Vec<Meter>>,
    subscribers: Mutex<Vec<Sender<Arc<TimedBlock>>>>,
    stop: AtomicBool,
}

/// Работающий аудиодвижок.
pub struct AudioEngine {
    names: Vec<String>,
    sources: Vec<Arc<SourceShared>>,
    threads: Vec<std::thread::JoinHandle<()>>,
    shared: Arc<Shared>,
}

impl AudioEngine {
    pub fn start(channels: Vec<ChannelConfig>) -> std::io::Result<Self> {
        let mut sources = Vec::new();
        let mut threads = Vec::new();
        for c in &channels {
            let s = Arc::new(SourceShared::default());
            threads.push(wasapi::spawn(c.direction, c.device.clone(), s.clone())?);
            sources.push(s);
        }
        let shared = Arc::new(Shared {
            controls: Mutex::new(channels.iter().map(|c| c.control).collect()),
            meters: Mutex::new(vec![Meter::default(); channels.len()]),
            subscribers: Mutex::new(Vec::new()),
            stop: AtomicBool::new(false),
        });
        let sh = shared.clone();
        let srcs = sources.clone();
        threads.push(std::thread::Builder::new().name("audio-mixer".into()).spawn(move || mixer_loop(srcs, sh))?);
        Ok(Self { names: channels.into_iter().map(|c| c.name).collect(), sources, threads, shared })
    }

    /// Подписка на блоки микса (для энкодеров и записи).
    pub fn subscribe(&self) -> Receiver<Arc<TimedBlock>> {
        let (tx, rx) = unbounded();
        self.shared.subscribers.lock().push(tx);
        rx
    }

    pub fn set_control(&self, channel: usize, control: ChannelControl) {
        if let Some(c) = self.shared.controls.lock().get_mut(channel) {
            *c = control;
        }
    }

    pub fn status(&self) -> Vec<ChannelStatus> {
        let controls = self.shared.controls.lock().clone();
        let meters = self.shared.meters.lock().clone();
        self.names
            .iter()
            .enumerate()
            .map(|(i, name)| ChannelStatus {
                name: name.clone(),
                control: controls.get(i).copied().unwrap_or_default(),
                meter: meters.get(i).copied().unwrap_or_default(),
                device: self.sources.get(i).and_then(|s| s.device_name.lock().clone()),
            })
            .collect()
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        for s in &self.sources {
            s.stop.store(true, Ordering::Release);
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

fn mmcss_pro_audio() {
    use windows::Win32::System::Threading::AvSetMmThreadCharacteristicsW;
    let mut task = 0u32;
    // SAFETY: регистрация текущего потока в MMCSS; дескриптор живёт до конца потока.
    unsafe {
        if AvSetMmThreadCharacteristicsW(windows::core::w!("Pro Audio"), &mut task).is_err() {
            tracing::debug!("MMCSS «Pro Audio» недоступен");
        }
    }
}

fn mixer_loop(sources: Vec<Arc<SourceShared>>, shared: Arc<Shared>) {
    mmcss_pro_audio();
    let mut mixer = Mixer::new(sources.len());
    let start = Instant::now();
    let start_ticks = Ticks::now();
    let block = Duration::from_secs_f64(BLOCK_FRAMES as f64 / f64::from(SAMPLE_RATE));
    let freq = Ticks::frequency();
    // Очередь каналов накапливается перед первым блоком: 20 мс запаса против рывков опроса.
    std::thread::sleep(Duration::from_millis(20));
    while !shared.stop.load(Ordering::Acquire) {
        {
            let controls = shared.controls.lock();
            for (ch, c) in mixer.channels.iter_mut().zip(controls.iter()) {
                ch.control = *c;
            }
        }
        let mut guards: Vec<_> = sources.iter().map(|s| s.queue.lock()).collect();
        let mut queues: Vec<&mut std::collections::VecDeque<f32>> = guards.iter_mut().map(|g| &mut **g).collect();
        let mixed = mixer.mix(&mut queues);
        drop(queues);
        drop(guards);
        *shared.meters.lock() = mixer.channels.iter().map(|c| c.meter).collect();
        let pts = Ticks(start_ticks.0 + (mixed.index as i64) * freq * BLOCK_FRAMES as i64 / i64::from(SAMPLE_RATE));
        let timed = Arc::new(TimedBlock { pts, block: mixed });
        shared.subscribers.lock().retain(|tx| tx.send(timed.clone()).is_ok());

        // Следующий блок — по расписанию от старта (без накопления ошибки).
        let next = start + block * (timed.block.index as u32 + 1);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        }
    }
}
