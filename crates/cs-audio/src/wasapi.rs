//! WASAPI (§8.1): звук рабочего стола (loopback устройства вывода), микрофон (shared mode)
//! и звук приложения (process loopback с дочерними процессами, Windows 10 2004+).
//!
//! Поток захвата на источник: опрос каждые 5 мс (loopback не присылает событий, когда ничего
//! не играет), преобразование в float32 стерео 48 кГц, запись в очередь канала.
//! Устройство по умолчанию отслеживается раз в 2 с; потеря устройства — переподключение.
//! Приложение ищется по имени exe; закрылось — канал ждёт его нового запуска.

use crate::mixer::AudioQueue;
use crate::resample::Resampler;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE};
use windows::Win32::Media::Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ,
};
use windows::core::Interface;

/// Направление устройства.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Устройство вывода (для loopback — «звук рабочего стола»).
    Render,
    /// Устройство ввода (микрофон).
    Capture,
    /// Звук приложения: «устройство» канала — имя exe.
    App,
}

/// Аудиоустройство.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

fn flow(d: Direction) -> EDataFlow {
    match d {
        Direction::Render | Direction::App => eRender,
        Direction::Capture => eCapture,
    }
}

fn com_init() {
    // SAFETY: инициализация COM текущего потока (MTA); повторный вызов безопасен.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn enumerator() -> windows::core::Result<IMMDeviceEnumerator> {
    // SAFETY: создание системного COM-объекта.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

fn device_id(dev: &IMMDevice) -> String {
    // SAFETY: строка выделена системой и освобождается CoTaskMemFree.
    unsafe {
        match dev.GetId() {
            Ok(p) => {
                let s = p.to_string().unwrap_or_default();
                CoTaskMemFree(Some(p.0 as *const _));
                s
            }
            Err(_) => String::new(),
        }
    }
}

fn device_name(dev: &IMMDevice) -> String {
    // SAFETY: чтение свойства устройства; PROPVARIANT освобождается своим Drop.
    unsafe {
        dev.OpenPropertyStore(STGM_READ)
            .and_then(|store| store.GetValue(&PKEY_Device_FriendlyName))
            .map(|v| v.to_string())
            .unwrap_or_default()
    }
}

/// Активные устройства заданного направления.
pub fn devices(direction: Direction) -> Vec<DeviceInfo> {
    com_init();
    let Ok(en) = enumerator() else { return vec![] };
    // SAFETY: перечисление через COM; индексы в пределах GetCount.
    unsafe {
        let Ok(coll) = en.EnumAudioEndpoints(flow(direction), DEVICE_STATE_ACTIVE) else { return vec![] };
        let n = coll.GetCount().unwrap_or(0);
        (0..n)
            .filter_map(|i| coll.Item(i).ok())
            .map(|d| DeviceInfo { id: device_id(&d), name: device_name(&d) })
            .collect()
    }
}

/// Идентификатор устройства по умолчанию.
pub fn default_device(direction: Direction) -> Option<DeviceInfo> {
    com_init();
    let en = enumerator().ok()?;
    // SAFETY: запрос устройства по умолчанию.
    let d = unsafe { en.GetDefaultAudioEndpoint(flow(direction), eConsole) }.ok()?;
    Some(DeviceInfo { id: device_id(&d), name: device_name(&d) })
}

/// Формат данных устройства.
#[derive(Clone, Copy)]
enum SampleFormat {
    F32,
    I16,
    I32,
}

struct Stream {
    _client: IAudioClient,
    capture: IAudioCaptureClient,
    format: SampleFormat,
    channels: usize,
    resampler: Resampler,
    device_id: String,
}

fn open(direction: Direction, device: Option<&str>) -> windows::core::Result<(Stream, String)> {
    if direction == Direction::App {
        return open_app(device.unwrap_or_default());
    }
    let en = enumerator()?;
    // SAFETY: COM-вызовы WASAPI; указатель формата освобождается CoTaskMemFree после копирования полей.
    unsafe {
        let dev = match device {
            Some(id) => en.GetDevice(&windows::core::HSTRING::from(id))?,
            None => en.GetDefaultAudioEndpoint(flow(direction), eConsole)?,
        };
        let name = device_name(&dev);
        let client: IAudioClient = dev.Activate(CLSCTX_ALL, None)?;
        let fmt_ptr = client.GetMixFormat()?;
        let fmt = *fmt_ptr;
        let format = if fmt.wFormatTag == WAVE_FORMAT_IEEE_FLOAT as u16 {
            SampleFormat::F32
        } else if fmt.wFormatTag == WAVE_FORMAT_EXTENSIBLE as u16 {
            let ext = *(fmt_ptr as *const WAVEFORMATEXTENSIBLE);
            let sub = ext.SubFormat;
            if sub == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT {
                SampleFormat::F32
            } else if sub == KSDATAFORMAT_SUBTYPE_PCM && fmt.wBitsPerSample == 32 {
                SampleFormat::I32
            } else {
                SampleFormat::I16
            }
        } else {
            SampleFormat::I16
        };
        let flags = if direction == Direction::Render { AUDCLNT_STREAMFLAGS_LOOPBACK } else { 0 };
        // Буфер 100 мс (в единицах 100 нс): запас на опрос раз в 5 мс.
        let r = client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 1_000_000, 0, fmt_ptr, None);
        CoTaskMemFree(Some(fmt_ptr as *const _));
        r?;
        let capture: IAudioCaptureClient = client.GetService()?;
        // Метки loopback уже идут по часам позиции устройства вывода (момент воспроизведения
        // по данным WASAPI), поэтому поправка на задержку вывода не нужна — проверено syncgen.
        client.Start()?;
        let stream = Stream {
            capture,
            format,
            channels: usize::from(fmt.nChannels.max(1)),
            resampler: Resampler::new(fmt.nSamplesPerSec, crate::mixer::SAMPLE_RATE),
            device_id: device_id(&dev),
            _client: client,
        };
        Ok((stream, name))
    }
}

/// Захват звука приложения по имени exe (с дочерними процессами), формат сразу 48 кГц float стерео.
fn open_app(exe: &str) -> windows::core::Result<(Stream, String)> {
    use windows::Win32::Media::Multimedia::WAVE_FORMAT_IEEE_FLOAT;
    let pid = find_app_pid(exe).ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
    let client = process_loopback_client(pid)?;
    let rate = crate::mixer::SAMPLE_RATE;
    let fmt = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_IEEE_FLOAT as u16,
        nChannels: 2,
        nSamplesPerSec: rate,
        nAvgBytesPerSec: rate * 8,
        nBlockAlign: 8,
        wBitsPerSample: 32,
        cbSize: 0,
    };
    // SAFETY: COM-вызовы WASAPI с форматом на стеке.
    unsafe {
        let flags = AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM;
        client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 1_000_000, 0, &fmt, None)?;
        let capture: IAudioCaptureClient = client.GetService()?;
        client.Start()?;
        let stream = Stream {
            capture,
            format: SampleFormat::F32,
            channels: 2,
            resampler: Resampler::new(rate, rate),
            device_id: pid.to_string(),
            _client: client,
        };
        Ok((stream, app_title(exe)))
    }
}

/// Имя приложения для UI: exe без расширения.
pub fn app_title(exe: &str) -> String {
    exe.strip_suffix(".exe").or_else(|| exe.strip_suffix(".EXE")).unwrap_or(exe).to_owned()
}

#[windows_core::implement(IActivateAudioInterfaceCompletionHandler, windows::Win32::System::Com::IAgileObject)]
struct ActivateDone(std::sync::mpsc::Sender<()>);

impl IActivateAudioInterfaceCompletionHandler_Impl for ActivateDone_Impl {
    fn ActivateCompleted(
        &self,
        _op: windows_core::Ref<IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        let _ = self.0.send(());
        Ok(())
    }
}

impl windows::Win32::System::Com::IAgileObject_Impl for ActivateDone_Impl {}

/// Клиент захвата звука процесса вместе с дочерними (`PROCESS_LOOPBACK`, Windows 10 2004+).
pub fn process_loopback_client(pid: u32) -> windows::core::Result<IAudioClient> {
    use windows::Win32::Foundation::E_FAIL;
    use windows::Win32::System::Com::BLOB;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Variant::VT_BLOB;
    let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: pid,
                ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            },
        },
    };
    // PROPVARIANT ссылается на `params` на стеке: живёт до конца активации, PropVariantClear не нужен.
    let mut pv = std::mem::ManuallyDrop::new(PROPVARIANT::default());
    // SAFETY: заполнение объединения PROPVARIANT как VT_BLOB; асинхронная операция ждётся здесь же.
    unsafe {
        let inner = &mut pv.Anonymous.Anonymous;
        inner.vt = VT_BLOB;
        inner.Anonymous.blob =
            BLOB { cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32, pBlobData: (&raw mut params).cast() };
        let (tx, rx) = std::sync::mpsc::channel();
        let handler: IActivateAudioInterfaceCompletionHandler = ActivateDone(tx).into();
        let op = ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&*pv),
            &handler,
        )?;
        rx.recv_timeout(Duration::from_secs(5)).map_err(|_| windows::core::Error::from(E_FAIL))?;
        let mut hr = windows::core::HRESULT(0);
        let mut unk: Option<windows::core::IUnknown> = None;
        op.GetActivateResult(&mut hr, &mut unk)?;
        hr.ok()?;
        unk.ok_or_else(|| windows::core::Error::from(E_FAIL))?.cast()
    }
}

/// Корневой процесс приложения по имени exe: у браузеров и лаунчеров процессов с одним именем
/// много, захват идёт по дереву от того, чей родитель — другой процесс.
fn find_app_pid(exe: &str) -> Option<u32> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    let mut found: Vec<(u32, u32)> = Vec::new();
    // SAFETY: снимок процессов; дескриптор закрывается здесь же.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut e = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        while ok {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            if String::from_utf16_lossy(&e.szExeFile[..len]).eq_ignore_ascii_case(exe) {
                found.push((e.th32ProcessID, e.th32ParentProcessID));
            }
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    found.iter().find(|(_, parent)| !found.iter().any(|(p, _)| p == parent)).or(found.first()).map(|(pid, _)| *pid)
}

/// Имя exe процесса.
fn process_exe(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::*;
    // SAFETY: дескриптор процесса только для запроса имени, закрывается здесь же.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        r.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/']).next().map(str::to_owned)
    }
}

/// Процесс ещё работает.
fn process_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows::Win32::System::Threading::*;
    // SAFETY: дескриптор процесса только для кода завершения, закрывается здесь же.
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return false };
        let mut code = 0u32;
        let r = GetExitCodeProcess(h, &mut code);
        let _ = CloseHandle(h);
        r.is_ok() && code == STILL_ACTIVE.0 as u32
    }
}

/// Списки устройств и приложений со звуком.
#[derive(Debug, Clone, Default)]
pub struct DeviceSnapshot {
    pub render: Vec<DeviceInfo>,
    pub capture: Vec<DeviceInfo>,
    pub apps: Vec<AppInfo>,
    /// Первый опрос уже прошёл.
    pub ready: bool,
}

/// Фоновый опрос устройств и приложений со звуком раз в 2 с. Перечисление через COM может
/// подвисать, пока Windows обрабатывает подключение устройства, — окно его не ждёт.
pub struct DeviceWatch {
    snapshot: Arc<Mutex<DeviceSnapshot>>,
    stop: Arc<AtomicBool>,
}

impl DeviceWatch {
    pub fn start() -> Self {
        let snapshot = Arc::new(Mutex::new(DeviceSnapshot::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (snap, st) = (snapshot.clone(), stop.clone());
        let spawned = std::thread::Builder::new().name("audio-devices".into()).spawn(move || {
            com_init();
            while !st.load(Ordering::Acquire) {
                let fresh = DeviceSnapshot {
                    render: devices(Direction::Render),
                    capture: devices(Direction::Capture),
                    apps: audio_apps(),
                    ready: true,
                };
                *snap.lock() = fresh;
                for _ in 0..20 {
                    if st.load(Ordering::Acquire) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });
        if let Err(e) = spawned {
            tracing::warn!("опрос аудиоустройств не запущен: {e}");
        }
        Self { snapshot, stop }
    }

    pub fn snapshot(&self) -> DeviceSnapshot {
        self.snapshot.lock().clone()
    }
}

impl Drop for DeviceWatch {
    /// Поток не ждём: если он стоит в COM-вызове, закрытие окна не должно зависать.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

/// Приложение, которое сейчас выводит звук.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    pub exe: String,
    pub pid: u32,
}

/// Приложения со звуковыми сессиями на устройстве вывода по умолчанию (как в микшере громкости
/// Windows), без самой программы и системных звуков.
pub fn audio_apps() -> Vec<AppInfo> {
    com_init();
    let own = std::process::id();
    let mut out: Vec<AppInfo> = Vec::new();
    let Ok(en) = enumerator() else { return out };
    // SAFETY: COM-вызовы менеджера сессий.
    unsafe {
        let Ok(dev) = en.GetDefaultAudioEndpoint(eRender, eConsole) else { return out };
        let Ok(mgr) = dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { return out };
        let Ok(list) = mgr.GetSessionEnumerator() else { return out };
        for i in 0..list.GetCount().unwrap_or(0) {
            let Ok(pid) =
                list.GetSession(i).and_then(|s| s.cast::<IAudioSessionControl2>()).and_then(|s| s.GetProcessId())
            else {
                continue;
            };
            if pid == 0 || pid == own {
                continue;
            }
            if let Some(exe) = process_exe(pid)
                && !out.iter().any(|a| a.exe.eq_ignore_ascii_case(&exe))
            {
                out.push(AppInfo { exe, pid });
            }
        }
    }
    out.sort_by_key(|a| a.exe.to_lowercase());
    out
}

impl Stream {
    /// Забирает все доступные пакеты в очередь (стерео 48 кГц) с отметками времени захвата.
    fn read(&mut self, out: &mut AudioQueue) -> windows::core::Result<()> {
        let mut stereo = Vec::new();
        let mut resampled = Vec::new();
        // SAFETY: буфер пакета валиден между GetBuffer и ReleaseBuffer; размер — frames * channels * bytes.
        unsafe {
            loop {
                let n = self.capture.GetNextPacketSize()?;
                if n == 0 {
                    break;
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                let mut qpc = 0u64;
                self.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, Some(&mut qpc))?;
                // Позиция QPC первого кадра пакета в единицах 100 нс (§5.2 п.4: время захвата);
                // при ошибке метки драйвера время пакета неизвестно.
                let stamp = (qpc != 0 && flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 == 0).then_some(qpc);
                let frames_us = frames as usize;
                stereo.clear();
                stereo.reserve(frames_us * 2);
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null();
                let ch = self.channels;
                for f in 0..frames_us {
                    let (l, r) = if silent {
                        (0.0, 0.0)
                    } else {
                        let get = |c: usize| -> f32 {
                            let idx = f * ch + c.min(ch - 1);
                            match self.format {
                                SampleFormat::F32 => *(data as *const f32).add(idx),
                                SampleFormat::I16 => f32::from(*(data as *const i16).add(idx)) / 32768.0,
                                SampleFormat::I32 => *(data as *const i32).add(idx) as f32 / 2_147_483_648.0,
                            }
                        };
                        // Моно → в оба канала; многоканальный → L/R (первые два).
                        (get(0), get(1))
                    };
                    stereo.push(l);
                    stereo.push(r);
                }
                self.capture.ReleaseBuffer(frames)?;
                resampled.clear();
                self.resampler.process(&stereo, &mut resampled);
                out.push(&resampled, stamp);
            }
        }
        Ok(())
    }
}

/// Общее состояние источника захвата (для UI и микшера).
#[derive(Default)]
pub struct SourceShared {
    /// Очередь сэмплов (interleaved стерео 48 кГц) с отметками времени захвата.
    pub queue: Mutex<AudioQueue>,
    /// Имя текущего устройства (`None` — нет устройства).
    pub device_name: Mutex<Option<String>>,
    /// Время захвата (QPC, 100 нс) сэмпла сразу после последнего в очереди; 0 — неизвестно.
    pub queue_end_qpc: std::sync::atomic::AtomicU64,
    pub stop: AtomicBool,
}

/// Предел очереди источника: 3 с (смещение синхронизации канала — до +2 с, §8.2).
const MAX_QUEUE: usize = crate::mixer::SAMPLE_RATE as usize * 6; // 3 с стерео

/// Поток захвата одного источника.
pub fn spawn(
    direction: Direction,
    device: Option<String>,
    shared: Arc<SourceShared>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name(format!("audio-{direction:?}").to_lowercase()).spawn(move || {
        com_init();
        let mut stream: Option<Stream> = None;
        let mut last_check = Instant::now();
        let mut retry_at = Instant::now();
        while !shared.stop.load(Ordering::Acquire) {
            if stream.is_none() && Instant::now() >= retry_at {
                match open(direction, device.as_deref()) {
                    Ok((s, name)) => {
                        tracing::info!("аудио {direction:?}: {name}");
                        *shared.device_name.lock() = Some(name);
                        stream = Some(s);
                    }
                    Err(e) => {
                        tracing::debug!("аудио {direction:?}: нет устройства ({e})");
                        *shared.device_name.lock() = None;
                        retry_at = Instant::now() + Duration::from_secs(2);
                    }
                }
            }
            // Приложение закрылось — канал ждёт его нового запуска.
            if direction == Direction::App && last_check.elapsed() >= Duration::from_secs(2) {
                last_check = Instant::now();
                if let Some(pid) = stream.as_ref().and_then(|s| s.device_id.parse::<u32>().ok())
                    && !process_alive(pid)
                {
                    tracing::info!("аудио App: процесс {pid} завершился, ждём нового запуска");
                    stream = None;
                    *shared.device_name.lock() = None;
                    retry_at = Instant::now() + Duration::from_secs(2);
                }
            }
            // Следование за устройством по умолчанию.
            if direction != Direction::App && device.is_none() && last_check.elapsed() >= Duration::from_secs(2) {
                last_check = Instant::now();
                let current = stream.as_ref().map(|s| s.device_id.clone());
                if let Some(def) = default_device(direction)
                    && current.as_deref().is_some_and(|c| c != def.id)
                {
                    tracing::info!("аудио {direction:?}: устройство по умолчанию сменилось → {}", def.name);
                    stream = None;
                    retry_at = Instant::now();
                }
            }
            if let Some(s) = stream.as_mut() {
                let mut q = shared.queue.lock();
                let read = s.read(&mut q);
                if let Err(e) = read {
                    tracing::warn!("аудио {direction:?}: {e}; переподключение");
                    stream = None;
                    *shared.device_name.lock() = None;
                    retry_at = Instant::now() + Duration::from_millis(500);
                }
                let frames = q.frames();
                if frames > MAX_QUEUE / 2 {
                    q.discard_front(frames - MAX_QUEUE / 2);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })
}
