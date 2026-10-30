//! Сторож рендер-потока: цикл не продвигается дольше 3 с — в лог пишется этап, на котором он
//! стоит; при восстановлении — сколько длилась остановка. Для разбора зависаний по логу.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Этап цикла рендер-потока.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Stage {
    Commands = 1,
    Tick,
    Publish,
    Housekeeping,
    Sleep,
}

impl Stage {
    fn name(v: u8) -> &'static str {
        match v {
            1 => "команды",
            2 => "компоновка",
            3 => "публикация состояния",
            4 => "диск и автосохранение",
            5 => "ожидание кадра",
            _ => "запуск",
        }
    }
}

/// Пульс цикла: каждый этап отмечается, сторож следит за продвижением.
#[derive(Default)]
pub struct Heartbeat {
    beat: AtomicU64,
    stage: AtomicU8,
    stop: AtomicBool,
}

impl Heartbeat {
    pub fn mark(&self, stage: Stage) {
        self.stage.store(stage as u8, Ordering::Relaxed);
        self.beat.fetch_add(1, Ordering::Relaxed);
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
    }
}

/// Порог остановки цикла для записи в лог.
const STALL: Duration = Duration::from_secs(3);

/// Поток сторожа; завершается после [`Heartbeat::stop`].
pub fn spawn(hb: Arc<Heartbeat>) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name("render-watchdog".into()).spawn(move || {
        let mut last = hb.beat.load(Ordering::Relaxed);
        let mut since = Instant::now();
        let mut warned = false;
        while !hb.stop.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(500));
            let beat = hb.beat.load(Ordering::Relaxed);
            if beat != last {
                if warned {
                    tracing::info!(
                        "рендер-поток снова работает после остановки {:.1} с",
                        since.elapsed().as_secs_f64()
                    );
                    warned = false;
                }
                last = beat;
                since = Instant::now();
            } else if !warned && since.elapsed() >= STALL {
                warned = true;
                tracing::warn!(
                    "рендер-поток не продвигается {:.1} с, этап: {}",
                    since.elapsed().as_secs_f64(),
                    Stage::name(hb.stage.load(Ordering::Relaxed))
                );
            }
        }
    })
}
