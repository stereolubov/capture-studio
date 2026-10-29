//! Аудиодвижок: потоки захвата + поток микшера (MMCSS «Pro Audio», блоки 10 мс по мастер-часам).
//!
//! Метка времени блока = момент старта микшера + `index × 10 мс` (тики QPC, §5.2 п.4).
//! Подписчики (энкодеры) получают каждый блок; аудио не отбрасывается никогда (§5.2 п.5).

use crate::filters::FilterChain;
use crate::mixer::{BLOCK_FRAMES, ChannelControl, Meter, MixBlock, Mixer, SAMPLE_RATE};
use crate::wasapi::{self, Direction, SourceShared};
use crossbeam_channel::{Receiver, Sender, unbounded};
use cs_core::clock::Ticks;
use cs_core::config::profile::AudioFilter;
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
    pub filters: Vec<AudioFilter>,
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
    pub filters: Vec<AudioFilter>,
    pub meter: Meter,
    /// Имя устройства; `None` — устройство недоступно.
    pub device: Option<String>,
}

/// Передача цепочки фильтров в поток микшера и обратно (старая уничтожается вне аудиопотока).
#[derive(Default)]
struct ChainSlot {
    incoming: Option<FilterChain>,
    outgoing: Option<FilterChain>,
}

struct Shared {
    controls: Mutex<Vec<ChannelControl>>,
    filters: Mutex<Vec<Vec<AudioFilter>>>,
    chains: Mutex<Vec<ChainSlot>>,
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
            filters: Mutex::new(channels.iter().map(|c| c.filters.clone()).collect()),
            chains: Mutex::new(
                channels
                    .iter()
                    .map(|c| ChainSlot { incoming: Some(FilterChain::new(&c.filters)), outgoing: None })
                    .collect(),
            ),
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

    /// Новая цепочка фильтров канала: собирается здесь, микшер подменяет её на следующем блоке.
    pub fn set_filters(&self, channel: usize, filters: Vec<AudioFilter>) {
        let chain = FilterChain::new(&filters);
        if let Some(f) = self.shared.filters.lock().get_mut(channel) {
            *f = filters;
        }
        if let Some(slot) = self.shared.chains.lock().get_mut(channel) {
            slot.outgoing = None;
            slot.incoming = Some(chain);
        }
    }

    pub fn status(&self) -> Vec<ChannelStatus> {
        let controls = self.shared.controls.lock().clone();
        let filters = self.shared.filters.lock().clone();
        let meters = self.shared.meters.lock().clone();
        self.names
            .iter()
            .enumerate()
            .map(|(i, name)| ChannelStatus {
                name: name.clone(),
                control: controls.get(i).copied().unwrap_or_default(),
                filters: filters.get(i).cloned().unwrap_or_default(),
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

/// Начальная задержка шкалы микшера (40 мс): очередь WASAPI + опрос + компенсация вывода.
const INITIAL_DELAY_100NS: i64 = 400_000;
const MAX_DELAY_100NS: i64 = 5_000_000;
const DELAY_STEP_100NS: i64 = 50_000;
const BLOCK_100NS: i64 = BLOCK_FRAMES as i64 * 10_000_000 / SAMPLE_RATE as i64;

fn mixer_loop(sources: Vec<Arc<SourceShared>>, shared: Arc<Shared>) {
    mmcss_pro_audio();
    let mut mixer = Mixer::new(sources.len());
    let start = Instant::now();
    let start_ticks = Ticks::now();
    let block = Duration::from_secs_f64(BLOCK_FRAMES as f64 / f64::from(SAMPLE_RATE));
    let freq = Ticks::frequency();
    let to_100ns = |t: i64| (i128::from(t) * 10_000_000 / i128::from(freq)) as i64;
    // Задержка шкалы микшера относительно часов, 100 нс: в блок с меткой T попадает звук,
    // захваченный в момент T (§5.2 п.4). Растёт, если какому-то источнику её не хватает.
    let mut delay: i64 = INITIAL_DELAY_100NS;
    // Конец очереди источника на прошлом блоке: нехватка учитывается, только пока данные идут.
    let mut prev_tail: Vec<Option<u64>> = vec![None; sources.len()];
    // Очередь каналов накапливается перед первым блоком: 20 мс запаса против рывков опроса.
    std::thread::sleep(Duration::from_millis(20));
    while !shared.stop.load(Ordering::Acquire) {
        {
            let controls = shared.controls.lock();
            for (ch, c) in mixer.channels.iter_mut().zip(controls.iter()) {
                ch.control = *c;
            }
        }
        // Новые цепочки фильтров: без ожидания, если движок как раз пишет в слоты.
        if let Some(mut slots) = shared.chains.try_lock() {
            for (ch, slot) in mixer.channels.iter_mut().zip(slots.iter_mut()) {
                if let Some(mut chain) = slot.incoming.take() {
                    chain.adopt(&mut ch.filters);
                    slot.outgoing = Some(std::mem::replace(&mut ch.filters, chain));
                }
            }
        }
        let scheduled =
            Ticks(start_ticks.0 + (mixer.index() as i64) * freq * BLOCK_FRAMES as i64 / i64::from(SAMPLE_RATE));
        let now = to_100ns(scheduled.0);
        let mut guards: Vec<_> = sources.iter().map(|s| s.queue.lock()).collect();
        // Хватает ли задержки: у активного источника к моменту блока должен быть звук до конца
        // блока (с учётом смещения канала). Не хватает — шкала сдвигается позже (разовый скачок).
        for (i, (q, ch)) in guards.iter().zip(mixer.channels.iter()).enumerate() {
            let tail = q.tail_time();
            let streaming = tail.is_some() && prev_tail[i].is_some_and(|p| tail > Some(p));
            prev_tail[i] = tail;
            if let Some(tail) = tail.filter(|_| streaming) {
                let need = now - tail as i64 + BLOCK_100NS - i64::from(ch.control.sync_offset_ms.min(0)) * 10_000;
                if need > delay && need < MAX_DELAY_100NS {
                    delay = (need + DELAY_STEP_100NS - 1) / DELAY_STEP_100NS * DELAY_STEP_100NS;
                    tracing::info!("аудио: задержка шкалы микшера увеличена до {} мс (канал {i})", delay / 10_000);
                }
            }
        }
        let target = (now - delay) as u64;
        let mut queues: Vec<&mut crate::mixer::AudioQueue> = guards.iter_mut().map(|g| &mut **g).collect();
        let mixed = mixer.mix_aligned(&mut queues, Some(target));
        drop(queues);
        drop(guards);
        *shared.meters.lock() = mixer.channels.iter().map(|c| c.meter).collect();
        // Метка блока — время захвата его звука.
        let pts = Ticks(scheduled.0 - (i128::from(delay) * i128::from(freq) / 10_000_000) as i64);
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
