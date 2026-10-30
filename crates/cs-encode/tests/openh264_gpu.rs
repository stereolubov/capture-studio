//! OpenH264 от Cisco: загрузка модуля с сервера Cisco (сверка хеша) и кодирование NV12-кадров.
//! Нужны GPU и сеть, поэтому `#[ignore]` (§18.2); запуск: `cargo test -- --ignored`.
//! Модуль кладётся в `%TEMP%/cs-openh264-test`, поток — в `%TEMP%/cs-openh264-test.h264`.

#![cfg(windows)]

use cs_encode::openh264::{self, Download, OpenH264Encoder};
use cs_encode::{EncoderOptions, RateControl, VideoSettings, avc};
use cs_gpu::{ColorConverter, ColorParams, ConvertOptions, DriverKind, GpuDevice};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;

#[test]
#[ignore = "нужны GPU и доступ к серверу Cisco"]
fn openh264_downloads_and_encodes() {
    openh264::set_dir(std::env::temp_dir().join("cs-openh264-test"));
    if !openh264::installed() {
        openh264::start_download();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while matches!(openh264::download_state(), Download::Running(_)) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert_eq!(openh264::download_state(), Download::Idle, "загрузка завершилась");
    }
    assert!(openh264::installed(), "модуль на месте");

    let (w, h) = (640u32, 360u32);
    let gpu = GpuDevice::new(DriverKind::Hardware).expect("gpu");
    let s = VideoSettings {
        width: w,
        height: h,
        fps_num: 30,
        fps_den: 1,
        bitrate_kbps: 1500,
        keyframe_interval_s: 1,
        rate_control: RateControl::Cbr,
    };
    let mut enc = OpenH264Encoder::new(&gpu.device, &s, &EncoderOptions::default()).expect("OpenH264");
    let mut conv = {
        let _g = gpu.lock();
        let co = ConvertOptions { pool: 8, ..Default::default() };
        ColorConverter::new(&gpu, w, h, ColorParams { bt601: false, full_range: false }, co).expect("conv")
    };
    let mut out = Vec::new();
    for i in 0..60i64 {
        let shade = (i * 4) as u8;
        let px: Vec<u8> = (0..w * h).flat_map(|p| [shade, (p % 256) as u8, 255 - shade, 255]).collect();
        let tex = {
            let _g = gpu.lock();
            let canvas = gpu.create_texture(w, h, DXGI_FORMAT_R8G8B8A8_UNORM, Some(&px), false).expect("canvas");
            conv.convert(&gpu, &canvas.srv, (w, h))
        };
        enc.push(tex, i * s.frame_duration());
        enc.pump(&mut out).expect("pump");
    }
    enc.drain(&mut out).expect("drain");
    assert_eq!(out.len(), 60, "по пакету на кадр (пропуск кадров выключен)");
    assert!(out[0].keyframe, "первый пакет — ключевой");
    assert!(avc::find_parameter_sets(&out[0].data).is_some(), "SPS/PPS в первом ключевом кадре");
    assert_eq!(out.iter().filter(|p| p.keyframe).count(), 2, "GOP 1 с при 30 к/с");
    assert!(out.windows(2).all(|p| p[1].pts > p[0].pts && p[1].dts == p[1].pts), "без перестановки");
    let stream: Vec<u8> = out.iter().flat_map(|p| p.data.iter().copied()).collect();
    std::fs::write(std::env::temp_dir().join("cs-openh264-test.h264"), stream).expect("файл");
}
