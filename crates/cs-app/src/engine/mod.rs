//! Движок (M1): рендер-поток компоновщика, источники, коллекция сцен (ADR-0009).
//!
//! UI общается с движком только через [`cs_core::Command`] и снимок [`AppState`] (§5.2 п.6):
//! команды уходят в ограниченную очередь, снимок публикуется рендер-потоком после каждого тика.
//! Превью — текстура холста на том же D3D11-устройстве (без копий), передаётся окну через
//! [`EngineHandle::preview_srv`].

mod recording;
mod scene_ops;
mod thread;

use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use cs_core::config::Profile;
use cs_core::model::SceneCollection;
use cs_core::state::Engine;
use cs_core::{AppState, Command, Event};
use cs_gpu::GpuDevice;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView;

/// Параметры запуска движка.
pub struct EngineConfig {
    pub gpu: Arc<GpuDevice>,
    pub collection: SceneCollection,
    /// Куда автосохранять коллекцию (`None` — не сохранять).
    pub collection_path: Option<PathBuf>,
    pub profile: Profile,
}

/// Общие между потоками данные.
struct Shared {
    snapshot: Mutex<Arc<AppState>>,
    /// Текущая текстура холста: (поколение, SRV).
    preview: Mutex<Option<(u64, ID3D11ShaderResourceView)>>,
    shutdown: AtomicBool,
}

/// Ручка движка для UI-потока.
pub struct EngineHandle {
    tx: Sender<Command>,
    events: Receiver<Event>,
    shared: Arc<Shared>,
    cached: Arc<AppState>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl EngineHandle {
    /// Запускает рендер-поток. Первый снимок готов сразу (до первого тика).
    pub fn start(cfg: EngineConfig) -> anyhow::Result<Self> {
        let (tx, rx) = bounded::<Command>(256);
        let (etx, events) = unbounded::<Event>();
        let initial = Arc::new(thread::initial_state(&cfg));
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(initial.clone()),
            preview: Mutex::new(None),
            shutdown: AtomicBool::new(false),
        });
        let sh = shared.clone();
        let thread = std::thread::Builder::new().name("render".into()).spawn(move || thread::run(cfg, rx, etx, sh))?;
        Ok(Self { tx, events, shared, cached: initial, thread: Some(thread) })
    }

    /// Текстура холста для превью (поколение меняется при смене разрешения).
    pub fn preview_srv(&self) -> Option<(u64, ID3D11ShaderResourceView)> {
        self.shared.preview.lock().clone()
    }
}

impl Engine for EngineHandle {
    fn send(&mut self, cmd: Command) {
        if self.tx.try_send(cmd).is_err() {
            tracing::warn!("очередь команд движка переполнена");
        }
    }

    fn poll(&mut self) -> Vec<Event> {
        self.cached = self.shared.snapshot.lock().clone();
        self.events.try_iter().collect()
    }

    fn state(&self) -> &AppState {
        &self.cached
    }

    fn preview_handle(&self) -> Option<(u64, Box<dyn std::any::Any>)> {
        self.preview_srv().map(|(g, srv)| (g, Box::new(srv) as Box<dyn std::any::Any>))
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
