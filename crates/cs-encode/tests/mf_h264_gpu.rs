//! Аппаратное кодирование H.264 на реальном GPU: NV12-текстуры → пакеты с SPS/PPS.
//! Требует GPU с аппаратным энкодером, поэтому `#[ignore]` (§18.2); запуск: `cargo test -- --ignored`.

#![cfg(windows)]

use cs_encode::avc;
use cs_encode::mf_h264::MfH264Encoder;
use cs_encode::{RateControl, VideoSettings};
use cs_gpu::{ColorConverter, ColorParams, DriverKind, GpuDevice};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;

#[test]
#[ignore = "нужен GPU с аппаратным H.264-энкодером"]
fn encodes_frames_on_gpu() {
    let gpu = GpuDevice::new(DriverKind::Hardware).expect("gpu");
    let s = VideoSettings {
        width: 1280,
        height: 720,
        fps_num: 60,
        fps_den: 1,
        bitrate_kbps: 4000,
        keyframe_interval_s: 1,
        rate_control: RateControl::Cbr,
    };
    let mut enc = MfH264Encoder::new(&gpu.device, &s).expect("encoder");
    let mut conv = {
        let _g = gpu.lock();
        ColorConverter::new(&gpu, 1280, 720, ColorParams { bt601: false, full_range: false }, Default::default())
            .expect("conv")
    };
    let mut out = Vec::new();
    for i in 0..120i64 {
        let shade = (i * 2) as u8;
        let px: Vec<u8> = (0..1280 * 720).flat_map(|_| [shade, 128, 255 - shade, 255]).collect();
        let tex = {
            let _g = gpu.lock();
            let canvas = gpu.create_texture(1280, 720, DXGI_FORMAT_R8G8B8A8_UNORM, Some(&px), false).expect("canvas");
            conv.convert(&gpu, &canvas.srv, (1280, 720))
        };
        enc.push(tex, i * s.frame_duration());
        enc.pump(&mut out).expect("pump");
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    enc.drain(&mut out).expect("drain");
    assert!(out.len() >= 100, "пакетов: {}", out.len());
    assert!(out[0].keyframe, "первый пакет — ключевой");
    assert!(avc::find_parameter_sets(&out[0].data).is_some(), "SPS/PPS в первом ключевом кадре");
    let keys = out.iter().filter(|p| p.keyframe).count();
    assert!(keys >= 2, "GOP 1 с → ключевых кадров за 2 с: {keys}");
    assert!(out.windows(2).all(|w| w[1].dts > w[0].dts), "DTS монотонны");
}
