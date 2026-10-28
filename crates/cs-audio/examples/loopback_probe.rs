//! Диагностика захвата: уровни loopback устройства вывода по умолчанию в течение нескольких секунд.

fn main() {
    use cs_audio::wasapi::{self, Direction, SourceShared};
    use std::sync::Arc;
    println!("устройство вывода: {:?}", wasapi::default_device(Direction::Render));
    for d in wasapi::devices(Direction::Render) {
        println!("  вывод: {} [{}]", d.name, d.id);
    }
    let shared = Arc::new(SourceShared::default());
    // Необязательный аргумент — id устройства вывода (иначе устройство по умолчанию).
    let device = std::env::args().nth(1);
    let t = wasapi::spawn(Direction::Render, device, shared.clone()).expect("spawn");
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let mut q = shared.queue.lock();
        let n = q.len();
        let peak = q.iter().fold(0f32, |m, v| m.max(v.abs()));
        q.clear();
        println!("сэмплов {n:6}  пик {peak:.4}  устройство {:?}", shared.device_name.lock());
    }
    shared.stop.store(true, std::sync::atomic::Ordering::Release);
    let _ = t.join();
}
