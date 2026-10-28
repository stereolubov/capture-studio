//! Capture Studio — точка входа.
//!
//! Режимы:
//! * обычный запуск (в M0 — то же, что `--mock-ui`: движок захвата появится в M1+);
//! * `--mock-ui` — UI с мок-состоянием без захвата (§17.6);
//! * `--screenshot <экран> <файл.png> [--theme light|dark] [--scale 1.5] [--lang en]` —
//!   отрисовка экрана 1a–1f во внеэкранную текстуру (скриншот-тесты);
//! * `--verbose` — уровень лога debug.

#![cfg_attr(not(test), windows_subsystem = "windows")]

mod args;
#[cfg(windows)]
mod engine;
mod i18n;
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod screenshot;
mod theme;
mod ui;

use anyhow::Context as _;
use cs_core::config::{self, DataDirs, Profile, Settings};
use cs_core::mock::MockEngine;
use cs_core::model::SceneCollection;

fn main() {
    let args = match args::Args::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\n{}", args::USAGE);
            std::process::exit(2);
        }
    };
    if let Err(e) = run(args) {
        tracing::error!("фатальная ошибка: {e:#}");
        eprintln!("Capture Studio: {e:#}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn run(args: args::Args) -> anyhow::Result<()> {
    use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};

    if let Some(shot) = args.screenshot.clone() {
        let _log = cs_diag::init_logging(None, args.verbose);
        return screenshot::run(&shot);
    }

    // SAFETY: вызывается до создания окон.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let _instance = match single_instance() {
        Some(m) => m,
        None => {
            platform::window::activate_existing();
            return Ok(());
        }
    };

    let dirs = match &args.data_dir {
        Some(d) => DataDirs::at(d),
        None => DataDirs::detect()?,
    };
    dirs.ensure().context("создание папки данных")?;
    let settings: Settings = config::load(&dirs.settings_file()).unwrap_or_else(|e| {
        eprintln!("settings.toml: {e}");
        Settings::default()
    });
    let _log = cs_diag::init_logging(Some(&dirs.logs_dir()), args.verbose || settings.verbose_log);
    log_header(&dirs);
    let profile: Profile = config::load(&dirs.profile_file(&settings.last_profile)).unwrap_or_else(|e| {
        tracing::warn!("профиль: {e}; используются значения по умолчанию");
        Profile::default()
    });
    set_process_priority(settings.process_priority);

    let gpu = std::sync::Arc::new(cs_gpu::GpuDevice::new(cs_gpu::DriverKind::Hardware)?);
    let engine: Box<dyn cs_core::state::Engine> = if args.mock_ui {
        let mut engine = MockEngine::new(true);
        engine.state_mut().preview_enabled = settings.preview_enabled;
        Box::new(engine)
    } else {
        let path = dirs.scenes_dir().join(format!("{}.toml", settings.last_scene_collection));
        let collection: SceneCollection = config::load(&path).unwrap_or_else(|e| {
            tracing::warn!("коллекция сцен: {e}; создаётся новая");
            SceneCollection::default()
        });
        Box::new(engine::EngineHandle::start(engine::EngineConfig {
            gpu: gpu.clone(),
            collection,
            collection_path: Some(path),
            profile: profile.clone(),
        })?)
    };
    let mut app = ui::App::new(engine, settings, profile, Some(dirs));
    if args.mock_ui {
        app.send(cs_core::Command::ApplyProfile(Box::new(app.profile.clone())));
    }
    platform::window::run(app, gpu)?;
    tracing::info!("выход");
    Ok(())
}

#[cfg(not(windows))]
fn run(_args: args::Args) -> anyhow::Result<()> {
    anyhow::bail!("Capture Studio v1 поддерживает только Windows")
}

/// Именованный мьютекс единственного экземпляра. `None` — приложение уже запущено.
#[cfg(windows)]
fn single_instance() -> Option<windows::Win32::Foundation::HANDLE> {
    use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows::Win32::System::Threading::CreateMutexW;
    // SAFETY: создание именованного объекта; дескриптор живёт до выхода из процесса.
    unsafe {
        let h = CreateMutexW(None, false, windows::core::w!("Local\\CaptureStudio.SingleInstance")).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS { None } else { Some(h) }
    }
}

#[cfg(windows)]
fn set_process_priority(p: cs_core::config::settings::ProcessPriority) {
    use cs_core::config::settings::ProcessPriority;
    use windows::Win32::System::Threading::{
        ABOVE_NORMAL_PRIORITY_CLASS, GetCurrentProcess, HIGH_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, SetPriorityClass,
    };
    let class = match p {
        ProcessPriority::Normal => NORMAL_PRIORITY_CLASS,
        ProcessPriority::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
        ProcessPriority::High => HIGH_PRIORITY_CLASS,
    };
    // SAFETY: псевдодескриптор текущего процесса.
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), class);
    }
}

/// Заголовок лога (§14.2): версия, ОС, режим данных. GPU пишет `cs-gpu` при создании устройства.
#[cfg(windows)]
fn log_header(dirs: &DataDirs) {
    tracing::info!(
        "Capture Studio {} · Windows build {} · данные: {}{}",
        env!("CARGO_PKG_VERSION"),
        platform::system::os_build(),
        dirs.root.display(),
        if dirs.portable { " (портативный режим)" } else { "" }
    );
}
